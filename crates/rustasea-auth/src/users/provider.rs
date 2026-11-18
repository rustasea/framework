/// Async, write-capable user provider seam.
///
/// [`super::UserLookup`] is a synchronous, read-only credential resolver: it
/// is enough for the JWT/session guards' `login`, but it cannot create a user,
/// rotate a password hash, or flip `email_verified_at` — and it cannot await a
/// database round-trip. Registration, password reset, profile update, and
/// email verification all need to WRITE a user, and a database-backed login
/// needs to READ asynchronously. `UserProvider` is that seam.
///
/// # Relationship to `UserLookup`
///
/// `UserProvider` is deliberately **not** a supertrait of [`super::UserLookup`].
/// The two contracts serve different call sites (async DB writes vs. sync guard
/// reads), and forcing an async implementor to also satisfy the sync read
/// contract would leak an implementation detail. A type is free to implement
/// both — [`MemoryUserProvider`] does — but it is not required.
///
/// # Async convention
///
/// Every method returns a hand-rolled `Pin<Box<dyn Future<..> + Send>>` rather
/// than using `async fn` in the trait or `async_trait`, matching the crate's
/// existing [`crate::guard::Guard`] convention. This keeps the trait
/// object-safe (`&dyn UserProvider`) and `Send`.
///
/// # Fail-closed
///
/// The mutating methods have **no default implementation**: an implementor must
/// make an explicit choice for every write. [`DenyAllProvider`] is the
/// fail-closed default, mirroring [`super::StaticLookup`]'s posture.
///
/// # ORM adapter story
///
/// A database-backed implementation (e.g. over `rustasea-orm`) resolves each
/// call asynchronously inside its boxed future — `find_by_email` issues a
/// `SELECT`, `create` an `INSERT`, and so on. There is no pre-loading and no
/// feature gate: the async boundary is the trait itself, so the ORM adapter
/// returns its boxed query future directly. [`DatabaseUserProvider`] is that
/// adapter over the `rustasea-orm` connection pool and the scaffold `users`
/// table.
use std::future::Future;
use std::pin::Pin;

use crate::error::{AuthError, Result};

use super::{AuthUserRecord, UserLookup};

mod database;
mod memory;

pub use database::DatabaseUserProvider;
pub use memory::MemoryUserProvider;

/// Input for [`UserProvider::create`] (registration).
///
/// Mirrors the validated registration payload: the caller has already hashed
/// the plaintext password (`password_hash` is a PHC string) and decided the
/// initial `email_verified_at` (`None` for the typical "verify by email"
/// flow). `name` is carried for ORM-backed providers that persist a full
/// `users` row; it is not representable on [`AuthUserRecord`] (which stays
/// limited to the auth-relevant columns), so an in-memory provider retains
/// only the auth-relevant fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUserRecord {
    /// Display name.
    pub name: String,
    /// Unique login email.
    pub email: String,
    /// Argon2 PHC hash of the plaintext password.
    pub password_hash: String,
    /// `email_verified_at` to persist, or `None` for a fresh unverified user.
    pub email_verified_at: Option<String>,
    /// Preferred IANA timezone (`users.timezone`), or `None` for no preference.
    pub timezone: Option<String>,
}

impl NewUserRecord {
    /// Build an unverified registration record from an already-hashed password.
    pub fn new(
        name: impl Into<String>,
        email: impl Into<String>,
        password_hash: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            password_hash: password_hash.into(),
            email_verified_at: None,
            timezone: None,
        }
    }

    /// Mark the record as already verified (builder form).
    pub fn with_email_verified_at(mut self, at: Option<impl Into<String>>) -> Self {
        self.email_verified_at = at.map(Into::into);
        self
    }

    /// Set the preferred IANA timezone (builder form).
    pub fn with_timezone(mut self, timezone: Option<impl Into<String>>) -> Self {
        self.timezone = timezone.map(Into::into);
        self
    }
}

/// Async write+read contract for the auth features.
///
/// Sufficient for registration (`create`), password reset / change
/// (`update_password`), email verification and email-change re-verification
/// (`set_email_verified_at`), and database-backed login (`find_by_email`).
pub trait UserProvider: Send + Sync {
    /// Resolve the full user record for `email` (async DB-backed login read).
    ///
    /// Returns `Ok(None)` when no user matches; an un-wired or unavailable
    /// provider returns `Err`, never a silent `Ok(None)`.
    fn find_by_email<'a>(
        &'a self,
        email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AuthUserRecord>>> + Send + 'a>>;

    /// Resolve the full user record for `user_id` (async DB-backed read).
    ///
    /// Returns `Ok(None)` when no user matches. Used to resolve the owning
    /// account after a passkey assertion (AUTH-017). The default implementation
    /// is fail-closed: an un-wired provider returns `Err`, never a silent
    /// `Ok(None)`, so a passkey login can never resolve a phantom user. Existing
    /// implementors keep compiling and keep failing closed until they opt in.
    fn find_by_id<'a>(
        &'a self,
        _user_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AuthUserRecord>>> + Send + 'a>> {
        Box::pin(async move {
            Err(AuthError::Disabled(
                "no user provider find_by_id is wired; denying access".to_string(),
            ))
        })
    }

    /// Persist a new user, returning the stored record (with its minted id).
    ///
    /// Must fail with [`AuthError::UserExists`] when `new_user.email` collides
    /// with an existing unique email.
    fn create<'a>(
        &'a self,
        new_user: NewUserRecord,
    ) -> Pin<Box<dyn Future<Output = Result<AuthUserRecord>> + Send + 'a>>;

    /// Replace the stored password hash for `user_id`.
    ///
    /// Must fail with [`AuthError::UserNotFound`] when `user_id` matches no
    /// user.
    fn update_password<'a>(
        &'a self,
        user_id: &'a str,
        password_hash: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Set (or clear) `email_verified_at` for `user_id`.
    ///
    /// Passing `None` clears the timestamp — used by an email change to force
    /// re-verification, mirroring Laravel's `markEmailAsUnverified`. Must fail
    /// with [`AuthError::UserNotFound`] when `user_id` matches no user.
    fn set_email_verified_at<'a>(
        &'a self,
        user_id: &'a str,
        verified_at: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Replace the stored display `name` and login `email` for `user_id`.
    ///
    /// Backs the profile-update route (`PATCH /settings/profile`): the caller
    /// has already authenticated and passes the desired values.
    ///
    /// Must fail with [`AuthError::UserNotFound`] when `user_id` matches no
    /// user, and with [`AuthError::UserExists`] when `email` is already owned by
    /// a *different* user (mirroring [`Self::create`]'s uniqueness rule).
    /// Re-asserting the caller's own current email is not a collision.
    ///
    /// [`AuthUserRecord`] does not carry `name`, so an in-memory provider keeps
    /// it in a side table (the read path does not expose it yet); a
    /// database-backed provider writes the `users.name` column directly.
    fn update_profile<'a>(
        &'a self,
        user_id: &'a str,
        name: &'a str,
        email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Set (or clear) the preferred IANA timezone (`users.timezone`).
    ///
    /// Backs the timezone-preference settings route. Passing `None` clears the
    /// preference so the resolver falls through to the app default. Must fail
    /// with [`AuthError::UserNotFound`] when `user_id` matches no user.
    ///
    /// The default implementation is fail-closed: an un-wired provider returns
    /// `Err`, never a silent `Ok(())`, so a preference write can never appear to
    /// succeed against a provider that does not persist it. Existing
    /// implementors keep compiling and keep failing closed until they opt in.
    fn update_timezone<'a>(
        &'a self,
        _user_id: &'a str,
        _timezone: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            Err(AuthError::Disabled(
                "no user provider update_timezone is wired; denying write".to_string(),
            ))
        })
    }
}

/// Fail-closed provider: every operation is denied.
///
/// The default wiring posture until an application injects a real provider
/// (ADR-0007 explicit `AppState` wiring). Mirrors [`super::StaticLookup`]:
/// reads yield no credentials and writes are refused, so an un-wired app fails
/// closed rather than silently persisting or authenticating.
#[derive(Debug, Default)]
pub struct DenyAllProvider;

impl DenyAllProvider {
    /// Human-readable reason attached to every denial.
    const REASON: &'static str = "no user provider is wired; denying user access";
}

impl UserProvider for DenyAllProvider {
    fn find_by_email<'a>(
        &'a self,
        _email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AuthUserRecord>>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }

    fn create<'a>(
        &'a self,
        _new_user: NewUserRecord,
    ) -> Pin<Box<dyn Future<Output = Result<AuthUserRecord>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }

    fn update_password<'a>(
        &'a self,
        _user_id: &'a str,
        _password_hash: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }

    fn set_email_verified_at<'a>(
        &'a self,
        _user_id: &'a str,
        _verified_at: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }

    fn update_profile<'a>(
        &'a self,
        _user_id: &'a str,
        _name: &'a str,
        _email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }

    fn update_timezone<'a>(
        &'a self,
        _user_id: &'a str,
        _timezone: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Err(AuthError::Disabled(Self::REASON.to_string())) })
    }
}

impl UserLookup for DenyAllProvider {
    fn hash_for_email(&self, _email: &str) -> Option<String> {
        None
    }

    fn id_for_email(&self, _email: &str) -> Option<String> {
        None
    }

    fn email_for_id(&self, _id: &str) -> Option<String> {
        None
    }
}

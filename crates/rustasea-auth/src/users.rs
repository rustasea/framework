/// User lookup seam for the JWT guard.
///
/// The M2 ORM sprint ships the `users` table; until a typed `User` model +
/// repository is wired through `AppState`, guards resolve credentials through
/// this object-safe trait. `StaticLookup` is the fail-closed default; tests
/// seed `MemoryUserRegistry`.
use std::collections::HashMap;
use std::sync::RwLock;

pub mod provider;

pub use provider::{
    DatabaseUserProvider, DenyAllProvider, MemoryUserProvider, NewUserRecord, UserProvider,
};

/// Minimal user record the auth layer needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserRecord {
    /// User UUID.
    pub id: String,
    /// Login email.
    pub email: String,
    /// Argon2 PHC hash of the password.
    pub password_hash: String,
    /// `email_verified_at` cleared by `markEmailAsUnverified`.
    pub email_verified_at: Option<String>,
    /// Preferred IANA timezone (`users.timezone`), or `None` for no preference.
    pub timezone: Option<String>,
}

/// Credential lookup contract injected into guards.
pub trait UserLookup: Send + Sync {
    /// Resolve the stored PHC hash for an email.
    fn hash_for_email(&self, email: &str) -> Option<String>;

    /// Resolve the user UUID for an email.
    fn id_for_email(&self, email: &str) -> Option<String>;

    /// Resolve the email for a user UUID.
    fn email_for_id(&self, id: &str) -> Option<String>;

    /// Resolve `users.email_verified_at` for a user UUID.
    ///
    /// Returns `None` for an unverified account **and** for a lookup that does
    /// not track verification at all — the two are indistinguishable on
    /// purpose, so an un-wired provider fails closed (the `verified` gate stays
    /// shut) rather than falsely reporting every user as verified. The default
    /// implementation returns `None`, so existing lookups keep compiling and
    /// keep failing closed until they opt in.
    fn email_verified_at_for_id(&self, _id: &str) -> Option<String> {
        None
    }

    /// Resolve the preferred IANA timezone (`users.timezone`) for a user UUID.
    ///
    /// Returns `None` for an account with no explicit preference **and** for a
    /// lookup that does not track timezones at all — the two are
    /// indistinguishable on purpose, so an un-wired lookup fails open to the
    /// mapper's next candidate (session/header/app default) rather than pinning
    /// a wrong zone. The default implementation returns `None`, so existing
    /// lookups keep compiling.
    fn timezone_for_id(&self, _id: &str) -> Option<String> {
        None
    }
}

/// Fail-closed lookup: never yields credentials.
///
/// The real database-backed lookup replaces this via `JwtGuard::with_lookup`
/// at boot (ADR-0007 explicit AppState wiring).
#[derive(Debug, Default)]
pub struct StaticLookup;

impl UserLookup for StaticLookup {
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

/// In-memory lookup for tests and local development.
#[derive(Debug, Default)]
pub struct MemoryUserRegistry {
    by_email: RwLock<HashMap<String, AuthUserRecord>>,
}

impl MemoryUserRegistry {
    /// Seed a user record for guard tests.
    pub fn seed(&self, record: AuthUserRecord) {
        // Poisoned lock => registry is unusable; fail closed (no-op) rather
        // than panic in a framework crate.
        if let Ok(mut by_email) = self.by_email.write() {
            by_email.insert(record.email.clone(), record);
        }
    }

    /// Look up a record by email.
    pub fn by_email(&self, email: &str) -> Option<AuthUserRecord> {
        self.by_email.read().ok()?.get(email).cloned()
    }
}

impl UserLookup for MemoryUserRegistry {
    fn hash_for_email(&self, email: &str) -> Option<String> {
        self.by_email(email).map(|r| r.password_hash)
    }

    fn id_for_email(&self, email: &str) -> Option<String> {
        self.by_email(email).map(|r| r.id)
    }

    fn email_for_id(&self, id: &str) -> Option<String> {
        self.by_email
            .read()
            .ok()?
            .values()
            .find(|r| r.id == id)
            .map(|r| r.email.clone())
    }

    fn email_verified_at_for_id(&self, id: &str) -> Option<String> {
        self.by_email
            .read()
            .ok()?
            .values()
            .find(|r| r.id == id)
            .and_then(|r| r.email_verified_at.clone())
    }

    fn timezone_for_id(&self, id: &str) -> Option<String> {
        self.by_email
            .read()
            .ok()?
            .values()
            .find(|r| r.id == id)
            .and_then(|r| r.timezone.clone())
    }
}

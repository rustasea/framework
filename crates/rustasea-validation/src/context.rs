/// External context for rules that cannot be evaluated from the payload alone.
///
/// `unique` and `current_password` inherently need information outside the
/// payload: a uniqueness check against a datastore, and the authenticated
/// user's current password hash. ADR-0007 forbids global mutable singletons,
/// so the context is passed explicitly to
/// [`crate::rules::Rules::validate_with`] rather than stored in a process-wide
/// default.
///
/// Every trait method is synchronous and object-safe, and returns `Option`:
/// `None` means *cannot determine*, which makes the consuming rule fail closed
/// (see [`crate::failure::RuleError::MissingContext`]).
///
/// This crate deliberately has **no** password-hashing dependency. Password
/// verification is delegated to the caller through
/// [`ValidationContext::current_password_matches`], so the auth layer keeps
/// ownership of the hash/algorithm.
use std::collections::{HashMap, HashSet};

/// Key identifying a `(table, column, value)` uniqueness probe.
type UniqueKey = (String, String, String);

/// Context a context-dependent rule needs to evaluate.
///
/// Implementors override only the methods they support; the default methods
/// return `None` (fail-closed), so a partial implementation is safe.
pub trait ValidationContext: Send + Sync {
    /// Whether `value` is unique in `table`.`column`.
    ///
    /// * `Some(true)` — no conflicting row exists.
    /// * `Some(false)` — a conflicting row exists.
    /// * `None` — cannot determine (the `unique` rule then fails with
    ///   [`crate::failure::RuleError::MissingContext`]).
    ///
    /// `ignore_id` mirrors Laravel's `Rule::unique()->ignore($id)`
    /// self-exclusion: the row whose primary key is `ignore_id` is excluded
    /// from the conflict check, so a user can re-submit their own unchanged
    /// value (for example an unchanged email on a profile update).
    fn is_unique(
        &self,
        table: &str,
        column: &str,
        value: &str,
        ignore_id: Option<&str>,
    ) -> Option<bool> {
        let _ = (table, column, value, ignore_id);
        None
    }

    /// Whether `plaintext` matches the authenticated user's current password.
    ///
    /// * `Some(true)` — the supplied password is the user's current password.
    /// * `Some(false)` — it is not.
    /// * `None` — no authenticated user or no verifier is available (the
    ///   `current_password` rule then fails with
    ///   [`crate::failure::RuleError::MissingContext`]).
    ///
    /// The comparison itself lives in the caller so this crate never needs a
    /// password-hashing dependency.
    fn current_password_matches(&self, plaintext: &str) -> Option<bool> {
        let _ = plaintext;
        None
    }
}

/// A [`ValidationContext`] that knows nothing.
///
/// Every probe returns `None`, so any context-dependent rule fails closed with
/// a typed missing-context error. This is the context used by
/// [`crate::rules::Rules::validate`], which keeps its historical signature.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoContext;

impl ValidationContext for NoContext {}

/// An in-memory [`ValidationContext`] for tests and lightweight callers.
///
/// It is deliberately simple: a set of "already taken" `(table, column, value)`
/// tuples (optionally owned by an id, for self-exclusion) and one expected
/// current password.
///
/// ```rust
/// use rustasea_validation::{MemoryContext, Rules};
/// use rustasea_validation::serde_json::json;
///
/// let ctx = MemoryContext::new().with_unique("users", "email", "ada@example.com");
/// let rules = Rules::new().field("email", "required|email|unique:users,email");
/// assert!(rules
///     .validate_with(&json!({ "email": "grace@example.com" }), &ctx)
///     .is_ok());
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryContext {
    /// Values that exist in a datastore (conflict for every caller).
    taken: HashSet<UniqueKey>,
    /// Values that exist but belong to a specific row id (self-exclusion).
    owners: HashMap<UniqueKey, String>,
    /// Expected current password, when one is configured.
    current_password: Option<String>,
}

impl MemoryContext {
    /// Create an empty context (no conflicts, no current password).
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `value` already exists in `table`.`column`.
    pub fn with_unique(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.taken
            .insert((table.into(), column.into(), value.into()));
        self
    }

    /// Record that `value` exists in `table`.`column` and is owned by
    /// `owner_id`.
    ///
    /// `unique:table,column,owner_id` then reports no conflict (self
    /// exclusion), while any other `ignore_id` still conflicts.
    pub fn with_unique_owned(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        value: impl Into<String>,
        owner_id: impl Into<String>,
    ) -> Self {
        let key = (table.into(), column.into(), value.into());
        self.taken.insert(key.clone());
        self.owners.insert(key, owner_id.into());
        self
    }

    /// Configure the plaintext that [`ValidationContext::current_password_matches`]
    /// should accept.
    pub fn with_current_password(mut self, plaintext: impl Into<String>) -> Self {
        self.current_password = Some(plaintext.into());
        self
    }
}

impl ValidationContext for MemoryContext {
    fn is_unique(
        &self,
        table: &str,
        column: &str,
        value: &str,
        ignore_id: Option<&str>,
    ) -> Option<bool> {
        let key = (table.to_string(), column.to_string(), value.to_string());
        if let Some(owner) = self.owners.get(&key) {
            // The only conflicting row is the caller's own row.
            return Some(ignore_id == Some(owner.as_str()));
        }
        Some(!self.taken.contains(&key))
    }

    fn current_password_matches(&self, plaintext: &str) -> Option<bool> {
        self.current_password.as_deref().map(|p| p == plaintext)
    }
}

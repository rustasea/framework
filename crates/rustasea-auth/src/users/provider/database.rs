/// Database-backed [`UserProvider`] over the ORM connection pool.
///
/// Reads and writes the `users` table created by the scaffold migration
/// (`create_users_table`): `id` (UUID), `name`, `email` (unique), `password`
/// (Argon2 PHC hash), `email_verified_at`, `timezone`, `created_at`,
/// `updated_at`, `deleted_at`. This is the store-backed counterpart to the
/// in-memory [`super::MemoryUserProvider`] and the fail-closed
/// [`super::DenyAllProvider`]: install it at boot so registration, login
/// ([`crate::SessionGuard::login_with_provider`]), password reset, and profile
/// updates persist across restarts and processes.
///
/// # Semantics
///
/// - **Soft-deleted users do not exist.** Every read and write filters on
///   `deleted_at IS NULL`, so a soft-deleted account can neither log in nor be
///   mutated.
/// - **A malformed id is "no such user".** Ids are UUIDs; a string that does not
///   parse resolves to `Ok(None)` on reads and [`AuthError::UserNotFound`] on
///   writes, never a storage error.
/// - **Timestamps are RFC 3339.** `email_verified_at` is bound as a native
///   timestamp and read back as an RFC 3339 string; an unparsable value is
///   rejected with [`AuthError::Disabled`] before any write.
///
/// # Error handling
///
/// Every database failure (unreachable server, missing `users` table) surfaces
/// as [`AuthError::StoreUnavailable`] — never a panic and never a silent
/// `Ok(None)`, so an outage cannot be mistaken for "unknown user".
use std::future::Future;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use rustasea_orm::{DbPool, Value};
use uuid::Uuid;

use crate::error::{AuthError, Result};
use crate::users::AuthUserRecord;

use super::{NewUserRecord, UserProvider};

/// Default table name, matching the scaffold `create_users_table` migration.
const DEFAULT_TABLE: &str = "users";

/// Columns selected for an [`AuthUserRecord`].
const COLUMNS: &str = "id, email, password, email_verified_at, timezone";

/// A [`UserProvider`] persisting users in a relational `users` table.
#[derive(Debug, Clone)]
pub struct DatabaseUserProvider {
    pool: DbPool,
    table: String,
}

impl DatabaseUserProvider {
    /// Build a provider over `pool` against the default `users` table.
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            table: DEFAULT_TABLE.to_string(),
        }
    }

    /// Override the table name (builder form).
    ///
    /// The name is interpolated into SQL, so it is validated as a plain
    /// identifier on every query; an invalid name fails every call with
    /// [`AuthError::StoreUnavailable`].
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = table.into();
        self
    }

    /// The configured table name.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Access the underlying pool (for callers that own the schema).
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// The table name, validated as a plain SQL identifier.
    fn checked_table(&self) -> Result<&str> {
        let table = self.table.trim();
        let mut chars = table.chars();
        let valid = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if valid {
            Ok(table)
        } else {
            Err(AuthError::StoreUnavailable)
        }
    }

    /// Fetch the single live user matching `column = value`.
    async fn find_where(&self, column: &str, value: Value) -> Result<Option<AuthUserRecord>> {
        let table = self.checked_table()?;
        let sql = format!(
            "SELECT {COLUMNS} FROM {table} WHERE {column} = $1 AND deleted_at IS NULL LIMIT 1"
        );
        let rows = self.pool.fetch_json(&sql, &[value]).await.map_err(store)?;
        Ok(rows.first().and_then(record_from_row))
    }

    /// Run an `UPDATE … SET <assignments>, updated_at = now WHERE id = <user_id>`.
    ///
    /// `assignments` uses placeholders `$1..$n` for `bindings`; the timestamp and
    /// id are appended as the next two placeholders. Fails with
    /// [`AuthError::UserNotFound`] when no live row matched.
    async fn update_user(
        &self,
        user_id: &str,
        assignments: &str,
        mut bindings: Vec<Value>,
    ) -> Result<()> {
        let table = self.checked_table()?;
        let not_found = || AuthError::UserNotFound {
            id: user_id.to_string(),
        };
        let id = Uuid::parse_str(user_id).map_err(|_| not_found())?;
        let next = bindings.len() + 1;
        let sql = format!(
            "UPDATE {table} SET {assignments}, updated_at = ${next} \
             WHERE id = ${} AND deleted_at IS NULL",
            next + 1
        );
        bindings.push(Value::Timestamp(Utc::now()));
        bindings.push(Value::Uuid(id));
        let affected = self
            .pool
            .execute_bind(&sql, &bindings)
            .await
            .map_err(store)?;
        if affected == 0 {
            return Err(not_found());
        }
        Ok(())
    }
}

impl UserProvider for DatabaseUserProvider {
    fn find_by_email<'a>(
        &'a self,
        email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AuthUserRecord>>> + Send + 'a>> {
        Box::pin(async move {
            self.find_where("email", Value::Text(email.to_string()))
                .await
        })
    }

    fn find_by_id<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AuthUserRecord>>> + Send + 'a>> {
        Box::pin(async move {
            match Uuid::parse_str(user_id) {
                Ok(id) => self.find_where("id", Value::Uuid(id)).await,
                Err(_) => Ok(None),
            }
        })
    }

    fn create<'a>(
        &'a self,
        new_user: NewUserRecord,
    ) -> Pin<Box<dyn Future<Output = Result<AuthUserRecord>> + Send + 'a>> {
        Box::pin(async move {
            let table = self.checked_table()?;
            let verified_at = timestamp_value(new_user.email_verified_at.as_deref())?;
            let exists = || AuthError::UserExists {
                email: new_user.email.clone(),
            };
            if self.email_taken(&new_user.email, None).await? {
                return Err(exists());
            }
            let id = Uuid::new_v4();
            let now = Utc::now();
            let mut bindings = vec![
                Value::Uuid(id),
                Value::Text(new_user.name.clone()),
                Value::Text(new_user.email.clone()),
                Value::Text(new_user.password_hash.clone()),
                Value::Timestamp(now),
                Value::Timestamp(now),
            ];
            let verified_at = placeholder(&mut bindings, verified_at);
            let timezone = placeholder(&mut bindings, new_user.timezone.clone().map(Value::Text));
            let sql = format!(
                "INSERT INTO {table} \
                 (id, name, email, password, created_at, updated_at, email_verified_at, timezone) \
                 VALUES ($1, $2, $3, $4, $5, $6, {verified_at}, {timezone})"
            );
            let inserted = self.pool.execute_bind(&sql, &bindings).await;
            if let Err(error) = inserted {
                // A concurrent registration can win the unique-email race
                // between the check above and this insert; report it as the
                // typed collision rather than a storage failure.
                return Err(match self.email_taken(&new_user.email, None).await {
                    Ok(true) => exists(),
                    _ => store(error),
                });
            }
            // Read the row back so the returned record carries the values as
            // the database normalized them (timestamp precision and offset).
            self.find_where("id", Value::Uuid(id))
                .await?
                .ok_or(AuthError::StoreUnavailable)
        })
    }

    fn update_password<'a>(
        &'a self,
        user_id: &'a str,
        password_hash: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.update_user(
                user_id,
                "password = $1",
                vec![Value::Text(password_hash.to_string())],
            )
            .await
        })
    }

    fn set_email_verified_at<'a>(
        &'a self,
        user_id: &'a str,
        verified_at: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut bindings = Vec::new();
            let value = placeholder(&mut bindings, timestamp_value(verified_at)?);
            self.update_user(user_id, &format!("email_verified_at = {value}"), bindings)
                .await
        })
    }

    fn update_profile<'a>(
        &'a self,
        user_id: &'a str,
        name: &'a str,
        email: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let Ok(id) = Uuid::parse_str(user_id) else {
                return Err(AuthError::UserNotFound {
                    id: user_id.to_string(),
                });
            };
            if self.email_taken(email, Some(id)).await? {
                return Err(AuthError::UserExists {
                    email: email.to_string(),
                });
            }
            self.update_user(
                user_id,
                "name = $1, email = $2",
                vec![
                    Value::Text(name.to_string()),
                    Value::Text(email.to_string()),
                ],
            )
            .await
        })
    }

    fn update_timezone<'a>(
        &'a self,
        user_id: &'a str,
        timezone: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut bindings = Vec::new();
            let value = placeholder(&mut bindings, timezone.map(Value::Text));
            self.update_user(user_id, &format!("timezone = {value}"), bindings)
                .await
        })
    }
}

impl DatabaseUserProvider {
    /// Whether `email` is owned by a row other than `except`.
    ///
    /// Soft-deleted rows count: the unique index on `email` still covers them,
    /// so their address cannot be reused.
    async fn email_taken(&self, email: &str, except: Option<Uuid>) -> Result<bool> {
        let table = self.checked_table()?;
        let sql = format!("SELECT id FROM {table} WHERE email = $1");
        let rows = self
            .pool
            .fetch_json(&sql, &[Value::Text(email.to_string())])
            .await
            .map_err(store)?;
        let except = except.map(|id| id.to_string());
        Ok(rows.iter().any(|row| {
            let owner = row.get("id").and_then(|v| v.as_str());
            owner.is_none() || owner != except.as_deref()
        }))
    }
}

/// Map any database failure to the typed "store unreachable" error.
fn store(_error: rustasea_orm::OrmError) -> AuthError {
    AuthError::StoreUnavailable
}

/// Convert an optional RFC 3339 string into a bindable timestamp.
fn timestamp_value(value: Option<&str>) -> Result<Option<Value>> {
    value
        .map(|text| {
            DateTime::parse_from_rfc3339(text)
                .map(|at| Value::Timestamp(at.with_timezone(&Utc)))
                .map_err(|_| {
                    AuthError::Disabled(format!("email_verified_at {text:?} is not RFC 3339"))
                })
        })
        .transpose()
}

/// Push `value` onto `bindings` and return its `$n` placeholder, or the SQL
/// literal `NULL` when absent.
///
/// A NULL is written as a literal rather than bound: the ORM binds
/// [`Value::Null`] as a typed integer NULL, which Postgres rejects for a
/// timestamp column.
fn placeholder(bindings: &mut Vec<Value>, value: Option<Value>) -> String {
    match value {
        Some(value) => {
            bindings.push(value);
            format!("${}", bindings.len())
        }
        None => "NULL".to_string(),
    }
}

/// Build an [`AuthUserRecord`] from a JSON row, or `None` when a required
/// column is missing.
fn record_from_row(row: &serde_json::Value) -> Option<AuthUserRecord> {
    let text = |column: &str| row.get(column).and_then(|v| v.as_str()).map(str::to_string);
    Some(AuthUserRecord {
        id: text("id")?,
        email: text("email")?,
        password_hash: text("password")?,
        email_verified_at: text("email_verified_at"),
        timezone: text("timezone"),
    })
}

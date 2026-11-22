//! Raw SQL execution over a [`DbPool`] — the executable counterpart to the
//! display-only `raw` / `raw_sql` fragment helpers.
//!
//! [`DbPool::query_raw`] runs a parameterized `SELECT` and decodes every row to
//! a JSON object; [`DbPool::execute_raw`] runs a statement and returns the
//! affected row count. Both are thin, explicitly named entry points over the
//! driver dispatch in [`crate::db::exec`], so they inherit the same `$n`
//! placeholder adaptation (rewritten to `?` on MySQL) and the same typed
//! [`OrmError::Storage`] / [`OrmError::MissingTable`] mapping for driver
//! failures.
//!
//! # SQL-injection safety
//!
//! Bindings are passed to the driver as parameters — they are **never**
//! interpolated into the SQL text. A caller that needs a literal must bind it as
//! a value (e.g. `$1`) rather than splicing it into `sql`; only the placeholder
//! *shape* is rewritten, never the bound values. The `sql` string itself is
//! always trusted, developer-authored text.

use crate::connections::ConnectionPair;
use crate::db::DbPool;
use crate::error::Result;
use crate::types::Value;

impl DbPool {
    /// Run a raw, parameterized `SELECT` and decode every row into a JSON object.
    ///
    /// The statement must use `$n` positional placeholders matching `bindings`;
    /// the active driver rewrites them to its native shape (`?` on MySQL). The
    /// returned rows carry one JSON object per result row, keyed by column name.
    ///
    /// Bindings are bound as query parameters — never string-interpolated — so
    /// caller-supplied values cannot alter the statement's structure.
    ///
    /// # Errors
    ///
    /// A missing table surfaces as
    /// [`OrmError::MissingTable`](crate::error::OrmError::MissingTable); a syntax
    /// error or type mismatch surfaces as
    /// [`OrmError::Storage`](crate::error::OrmError::Storage); a pool with no
    /// compiled-in driver surfaces as
    /// [`OrmError::UnsupportedDriver`](crate::error::OrmError::UnsupportedDriver).
    pub async fn query_raw(&self, sql: &str, bindings: &[Value]) -> Result<Vec<serde_json::Value>> {
        self.fetch_json(sql, bindings).await
    }

    /// Run a raw, parameterized statement and return the number of affected rows.
    ///
    /// Intended for `INSERT` / `UPDATE` / `DELETE` (and other non-`SELECT`)
    /// statements; use [`DbPool::query_raw`] when a result set is expected. The
    /// same `$n` → driver-native placeholder adaptation and parameter binding
    /// guarantees apply.
    ///
    /// # Errors
    ///
    /// A missing table surfaces as
    /// [`OrmError::MissingTable`](crate::error::OrmError::MissingTable); a syntax
    /// error or other driver failure surfaces as
    /// [`OrmError::Storage`](crate::error::OrmError::Storage).
    pub async fn execute_raw(&self, sql: &str, bindings: &[Value]) -> Result<u64> {
        self.execute_bind(sql, bindings).await
    }
}

impl ConnectionPair {
    /// Run a raw, parameterized `SELECT` on the read pool.
    ///
    /// Read routing mirrors [`DbPool::query_raw`], targeting
    /// [`ConnectionPair::read`]; with no split the read pool *is* the write pool.
    pub async fn query_raw(&self, sql: &str, bindings: &[Value]) -> Result<Vec<serde_json::Value>> {
        self.read().query_raw(sql, bindings).await
    }

    /// Run a raw, parameterized mutation on the write pool (never a replica).
    pub async fn execute_raw(&self, sql: &str, bindings: &[Value]) -> Result<u64> {
        self.write().execute_raw(sql, bindings).await
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    /// Open an in-memory pool with a small `users` table seeded.
    async fn pool_with_users() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        pool.execute_raw(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, status TEXT NOT NULL)",
            &[],
        )
        .await
        .unwrap();
        for (id, status) in [(1, "active"), (2, "active"), (3, "banned")] {
            pool.execute_raw(
                "INSERT INTO users (id, status) VALUES ($1, $2)",
                &[Value::Int(id), Value::Text(status.into())],
            )
            .await
            .unwrap();
        }
        pool
    }

    /// Positive: a bound `SELECT COUNT(*)` returns the correct integer count.
    #[tokio::test]
    async fn query_raw_returns_bound_count() {
        let pool = pool_with_users().await;
        let rows = pool
            .query_raw(
                "SELECT COUNT(*) AS n FROM users WHERE status = $1",
                &[Value::Text("active".into())],
            )
            .await
            .unwrap();
        let count = rows
            .first()
            .and_then(|row| row.get("n"))
            .and_then(serde_json::Value::as_u64);
        assert_eq!(count, Some(2));
        pool.close().await;
    }

    /// Positive: a bound `UPDATE` reports the affected row count.
    #[tokio::test]
    async fn execute_raw_reports_affected_rows() {
        let pool = pool_with_users().await;
        let affected = pool
            .execute_raw(
                "UPDATE users SET status = $1 WHERE status = $2",
                &[Value::Text("banned".into()), Value::Text("active".into())],
            )
            .await
            .unwrap();
        assert_eq!(affected, 2);
        pool.close().await;
    }

    /// Positive: a bound value carrying SQL metacharacters is treated as data,
    /// never as statement text (injection attempt is inert).
    #[tokio::test]
    async fn query_raw_binds_injection_payload_as_data() {
        let pool = pool_with_users().await;
        let rows = pool
            .query_raw(
                "SELECT COUNT(*) AS n FROM users WHERE status = $1",
                &[Value::Text("active' OR '1'='1".into())],
            )
            .await
            .unwrap();
        let count = rows
            .first()
            .and_then(|row| row.get("n"))
            .and_then(serde_json::Value::as_u64);
        assert_eq!(count, Some(0), "payload must be bound, not executed");
        pool.close().await;
    }

    /// Negative: a syntax error surfaces as a typed [`OrmError::Storage`].
    #[tokio::test]
    async fn query_raw_syntax_error_is_storage_error() {
        let pool = pool_with_users().await;
        let error = pool.query_raw("SELECT * FRM users", &[]).await.unwrap_err();
        assert!(
            matches!(error, crate::error::OrmError::Storage(_)),
            "got {error:?}"
        );
        pool.close().await;
    }

    /// Negative: a syntax error on the write path is also a typed storage error.
    #[tokio::test]
    async fn execute_raw_syntax_error_is_storage_error() {
        let pool = pool_with_users().await;
        let error = pool
            .execute_raw("UPDTE users SET status = $1", &[Value::Text("x".into())])
            .await
            .unwrap_err();
        assert!(
            matches!(error, crate::error::OrmError::Storage(_)),
            "got {error:?}"
        );
        pool.close().await;
    }
}

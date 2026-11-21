//! Real database connection foundation backed by `sqlx`.
//!
//! [`DbPool`] wraps one live `sqlx` pool per supported driver; [`DbPool::fetch_json`]
//! reads, [`DbPool::execute_bind`] writes, and [`DbPool::query_raw`]/[`DbPool::execute_raw`] run raw SQL.
//! Driver selection and connection-URL assembly live in the [`url`] submodule.

use crate::error::{OrmError, Result};
use std::time::Duration;

mod adapt;
mod exec;
mod raw;
mod url;

pub(crate) use url::encode_userinfo;

/// Maximum connections held by a pool (SQLite in-memory is capped at one).
const DEFAULT_MAX_CONNECTIONS: u32 = 10;

/// Time to wait for a connection before `connect`/`acquire` fails.
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);

/// Tunable pool limits applied when opening a connection. [`DbPool::connect`]
/// uses [`PoolSettings::default`]; config callers pass `[database.pool]` values.
#[derive(Debug, Clone, Copy)]
pub struct PoolSettings {
    /// Minimum number of idle connections kept warm.
    pub min_connections: u32,
    /// Maximum connections the pool will open.
    pub max_connections: u32,
    /// Time to wait for a connection before `connect`/`acquire` fails.
    pub acquire_timeout: Duration,
    /// Idle lifetime after which a pooled connection is reaped.
    pub idle_timeout: Option<Duration>,
}
impl Default for PoolSettings {
    /// ORM defaults: `min = 0`, `max = 10`, 30s acquire timeout, no idle reaping.
    fn default() -> Self {
        Self {
            min_connections: 0,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            acquire_timeout: DEFAULT_ACQUIRE_TIMEOUT,
            idle_timeout: None,
        }
    }
}

/// A live connection pool for one supported database driver.
#[derive(Debug, Clone)]
pub enum DbPool {
    /// SQLite pool (file-backed or `:memory:`).
    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
    /// PostgreSQL pool.
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
    /// MySQL pool.
    #[cfg(feature = "mysql")]
    MySql(sqlx::MySqlPool),
}

impl DbPool {
    /// Connect to `url`, selecting the driver from its scheme. Unknown schemes
    /// and driver failures surface as [`OrmError::Pool`]; a recognised scheme
    /// whose driver feature is not compiled in surfaces as
    /// [`OrmError::UnsupportedDriver`].
    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_with_settings(url, PoolSettings::default()).await
    }

    /// Connect to `url` with explicit [`PoolSettings`]. SQLite in-memory URLs
    /// pin to one connection, since each connection sees a distinct database.
    pub async fn connect_with_settings(url: &str, settings: PoolSettings) -> Result<Self> {
        let scheme = url.split(':').next().unwrap_or_default();
        match scheme {
            "sqlite" => Self::connect_sqlite(url, settings).await,
            "postgres" | "postgresql" => Self::connect_postgres(url, settings).await,
            "mysql" => Self::connect_mysql(url, settings).await,
            // Name only the scheme: the URL may carry credentials.
            other => Err(OrmError::Pool(format!(
                "invalid database URL: unknown scheme `{other}`"
            ))),
        }
    }

    /// The dialect name (`sqlite` / `postgres` / `mysql`).
    pub fn dialect(&self) -> &'static str {
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(_) => "sqlite",
            #[cfg(feature = "postgres")]
            DbPool::Postgres(_) => "postgres",
            #[cfg(feature = "mysql")]
            DbPool::MySql(_) => "mysql",
            #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
            _ => "unknown",
        }
    }

    /// Execute `SELECT 1` to verify the pool can reach the database.
    pub async fn ping(&self) -> Result<()> {
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
            _ => {}
        }
        Ok(())
    }

    /// Gracefully close every connection in the pool.
    pub async fn close(&self) {
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => pool.close().await,
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => pool.close().await,
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => pool.close().await,
            #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
            _ => {}
        }
    }

    /// The inner SQLite pool, when this handle is SQLite.
    #[cfg(feature = "sqlite")]
    pub fn sqlite(&self) -> Option<&sqlx::SqlitePool> {
        match self {
            DbPool::Sqlite(pool) => Some(pool),
            #[cfg(feature = "postgres")]
            DbPool::Postgres(_) => None,
            #[cfg(feature = "mysql")]
            DbPool::MySql(_) => None,
        }
    }

    /// The inner Postgres pool, when this handle is Postgres.
    #[cfg(feature = "postgres")]
    pub fn postgres(&self) -> Option<&sqlx::PgPool> {
        match self {
            DbPool::Postgres(pool) => Some(pool),
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(_) => None,
            #[cfg(feature = "mysql")]
            DbPool::MySql(_) => None,
        }
    }

    /// The inner MySQL pool, when this handle is MySQL.
    #[cfg(feature = "mysql")]
    pub fn mysql(&self) -> Option<&sqlx::MySqlPool> {
        match self {
            DbPool::MySql(pool) => Some(pool),
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(_) => None,
            #[cfg(feature = "postgres")]
            DbPool::Postgres(_) => None,
        }
    }

    /// A process-unique identity for this pool handle: clones share one
    /// connection-options allocation; separately opened pools differ.
    pub fn identity(&self) -> usize {
        use std::sync::Arc;
        match self {
            #[cfg(feature = "sqlite")]
            DbPool::Sqlite(pool) => Arc::as_ptr(&pool.connect_options()) as *const () as usize,
            #[cfg(feature = "postgres")]
            DbPool::Postgres(pool) => Arc::as_ptr(&pool.connect_options()) as *const () as usize,
            #[cfg(feature = "mysql")]
            DbPool::MySql(pool) => Arc::as_ptr(&pool.connect_options()) as *const () as usize,
            #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
            _ => 0,
        }
    }

    /// Connect a SQLite pool; in-memory URLs are pinned to one connection.
    #[cfg(feature = "sqlite")]
    async fn connect_sqlite(url: &str, settings: PoolSettings) -> Result<Self> {
        use std::str::FromStr;

        let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)
            .map_err(|error| OrmError::Pool(error.to_string()))?
            .create_if_missing(true);

        let mut pool_options = sqlx::sqlite::SqlitePoolOptions::new()
            .min_connections(settings.min_connections)
            .max_connections(settings.max_connections)
            .acquire_timeout(settings.acquire_timeout)
            .idle_timeout(settings.idle_timeout);
        if sqlite_is_memory(url) {
            pool_options = pool_options
                .min_connections(0)
                .max_connections(1)
                .idle_timeout(None)
                .max_lifetime(None);
        }

        let pool = pool_options
            .connect_with(options)
            .await
            .map_err(|error| OrmError::Pool(error.to_string()))?;
        Ok(DbPool::Sqlite(pool))
    }

    /// Report that the SQLite driver is not compiled in.
    #[cfg(not(feature = "sqlite"))]
    async fn connect_sqlite(_url: &str, _settings: PoolSettings) -> Result<Self> {
        Err(OrmError::UnsupportedDriver("sqlite".to_string()))
    }

    /// Connect a Postgres pool.
    #[cfg(feature = "postgres")]
    async fn connect_postgres(url: &str, settings: PoolSettings) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(settings.min_connections)
            .max_connections(settings.max_connections)
            .acquire_timeout(settings.acquire_timeout)
            .idle_timeout(settings.idle_timeout)
            .connect(url)
            .await
            .map_err(|error| OrmError::Pool(error.to_string()))?;
        Ok(DbPool::Postgres(pool))
    }

    /// Report that the Postgres driver is not compiled in.
    #[cfg(not(feature = "postgres"))]
    async fn connect_postgres(_url: &str, _settings: PoolSettings) -> Result<Self> {
        Err(OrmError::UnsupportedDriver("postgres".to_string()))
    }

    /// Connect a MySQL pool.
    #[cfg(feature = "mysql")]
    async fn connect_mysql(url: &str, settings: PoolSettings) -> Result<Self> {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .min_connections(settings.min_connections)
            .max_connections(settings.max_connections)
            .acquire_timeout(settings.acquire_timeout)
            .idle_timeout(settings.idle_timeout)
            .connect(url)
            .await
            .map_err(|error| OrmError::Pool(error.to_string()))?;
        Ok(DbPool::MySql(pool))
    }

    /// Report that the MySQL driver is not compiled in.
    #[cfg(not(feature = "mysql"))]
    async fn connect_mysql(_url: &str, _settings: PoolSettings) -> Result<Self> {
        Err(OrmError::UnsupportedDriver("mysql".to_string()))
    }
}

/// Whether a SQLite URL addresses an in-memory database.
#[cfg(feature = "sqlite")]
fn sqlite_is_memory(url: &str) -> bool {
    url.contains(":memory:") || url.contains("mode=memory")
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    /// Verifies an in-memory SQLite pool connects and answers a ping.
    #[tokio::test]
    async fn connects_and_pings_sqlite_memory() {
        let pool = DbPool::connect("sqlite::memory:").await.expect("connect");
        assert_eq!(pool.dialect(), "sqlite");
        assert!(pool.sqlite().is_some());
        pool.ping().await.expect("ping");
        pool.close().await;
    }

    /// Verifies in-memory pools are pinned to a single shared connection.
    #[tokio::test]
    async fn sqlite_memory_pool_is_single_connection() {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        match &pool {
            DbPool::Sqlite(inner) => assert_eq!(inner.options().get_max_connections(), 1),
            #[cfg(feature = "postgres")]
            DbPool::Postgres(_) => {}
            #[cfg(feature = "mysql")]
            DbPool::MySql(_) => {}
        }
        pool.close().await;
    }

    /// Verifies an invalid URL maps to a typed pool error.
    #[tokio::test]
    async fn invalid_url_maps_to_pool_error() {
        let error = DbPool::connect("nonsense://localhost/none")
            .await
            .unwrap_err();
        assert!(matches!(error, OrmError::Pool(_)), "got {error:?}");
    }

    /// Verifies an unknown-scheme error names the scheme, not the credentials.
    #[tokio::test]
    async fn unknown_scheme_error_omits_the_url() {
        let error = DbPool::connect("mariadb://user:hunter2@db.internal/app")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("`mariadb`"), "got {error}");
        assert!(!error.contains("hunter2"), "leaked the password: {error}");
    }
}

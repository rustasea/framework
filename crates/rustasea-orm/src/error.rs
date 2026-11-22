/// Typed errors for the ORM layer.
use thiserror::Error;

/// Alias for results produced by ORM operations.
pub type Result<T> = std::result::Result<T, OrmError>;

/// Top-level ORM error type.
#[derive(Debug, Error)]
pub enum OrmError {
    /// No row matched the query (`firstOrFail`, `findOrFail`).
    #[error("model not found")]
    NotFound,

    /// Strict upsert was called with an empty `uniqueBy` list.
    #[error("upsert rejected: {0}")]
    Upsert(#[from] UpsertError),

    /// Invalid builder state (e.g. paginating an unpaged query twice).
    #[error("invalid query state: {0}")]
    InvalidState(String),

    /// A value could not be bound to the query.
    #[error("invalid bind value: {0}")]
    InvalidValue(String),

    /// A global scope was registered or combined illegally (duplicate id,
    /// empty table name, poisoned registry).
    #[error("invalid scope: {0}")]
    Scope(String),

    /// The configured driver is unsupported for this operation.
    #[error("unsupported driver: {0}")]
    UnsupportedDriver(String),

    /// Underlying storage/IO failure surfaced by the driver.
    #[error("storage error: {0}")]
    Storage(String),

    /// A query referenced a table that does not exist (or was not created).
    #[error("missing table: {table}")]
    MissingTable {
        /// Name of the missing table.
        table: String,
    },

    /// Connection-pool setup or acquisition failure (`connect`, `ping`).
    #[error("connection pool error: {0}")]
    Pool(String),

    /// Named-connection resolution failure (unknown name, missing field, …).
    #[error(transparent)]
    Connection(#[from] ConnectionError),

    /// Migration failure.
    #[error(transparent)]
    Migration(#[from] crate::migration::MigrationError),

    /// Transaction lifecycle failure (already committed or rolled back).
    #[error(transparent)]
    Transaction(#[from] crate::tx::TransactionError),

    /// Schema-builder failure (bad dialect, identifier, or blueprint shape).
    #[error(transparent)]
    Schema(#[from] SchemaError),

    /// Vector dimension mismatch: the column expects `expected` dimensions but
    /// the supplied embedding has `actual`.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    VectorDimensionMismatch {
        /// Expected column dimension.
        expected: usize,
        /// Actual embedding length.
        actual: usize,
    },

    /// An attribute cast could not convert a column value between its database
    /// representation and its Rust representation (corrupt or malformed data).
    #[error("cast error on `{column}`: {message}")]
    CastError {
        /// Column whose cast failed.
        column: String,
        /// Human-readable failure reason.
        message: String,
    },

    /// An activity-log recorder failed while persisting an audit row.
    ///
    /// Audit integrity wins over write availability: a failed recording is
    /// surfaced rather than silently swallowed, so a missing audit entry is
    /// never invisible to the caller.
    #[error("activity log error: {0}")]
    Activity(String),

    /// Slug generation failed (ADOPT-016).
    ///
    /// Raised before any write when the configured source columns produce no
    /// slug text — a silent empty slug would make the row unreachable by its
    /// slug — or when the deterministic uniqueness search exhausts its
    /// candidate budget.
    #[error("slug error: {0}")]
    Slug(String),
}

impl From<sqlx::Error> for OrmError {
    /// Map a raw `sqlx` failure onto the ORM's storage error, or onto
    /// [`OrmError::MissingTable`] when the driver reports an unknown table.
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(database) = &error {
            if let Some(table) = missing_table_from_error(&**database) {
                return OrmError::MissingTable { table };
            }
        }
        OrmError::Storage(error.to_string())
    }
}

/// The table a driver error reports as missing, preferring the structured
/// driver code over the message heuristic.
///
/// Postgres reports `undefined_table` as SQLSTATE `42P01`; MySQL reports
/// `ER_NO_SUCH_TABLE` (error number `1146`) as SQLSTATE `42S02`. sqlx's
/// `DatabaseError::code()` surfaces the SQLSTATE, so both are classified from the
/// code before the message is parsed. SQLite reports only the generic
/// `SQLITE_ERROR` code for a missing table, so it and any unknown driver fall
/// back to [`missing_table`].
fn missing_table_from_error(database: &dyn sqlx::error::DatabaseError) -> Option<String> {
    let structured = match database.code().as_deref() {
        Some("42P01") => table_from_quoted(database.message(), '"'),
        Some("42S02") => table_from_quoted(database.message(), '\''),
        _ => None,
    };
    structured.or_else(|| missing_table(database.message()))
}

/// The table named by the first `quote`-delimited identifier in `message`.
fn table_from_quoted(message: &str, quote: char) -> Option<String> {
    table_name(message.split(quote).nth(1)?)
}

/// The table a driver error message reports as missing, without its schema.
///
/// SQLite: `no such table: t`; Postgres: `relation "t" does not exist`; MySQL:
/// `Table 'db.t' doesn't exist`. The pattern is matched anywhere in `message`, so
/// a driver-wrapped prefix does not defeat detection.
fn missing_table(message: &str) -> Option<String> {
    if let Some(idx) = message.find("no such table: ") {
        return table_name(&message[idx + "no such table: ".len()..]);
    }
    if let Some(idx) = message.find("relation \"") {
        let rest = &message[idx + "relation \"".len()..];
        if let Some(end) = rest.find("\" does not exist") {
            return table_name(&rest[..end]);
        }
    }
    if let Some(idx) = message.find("Table '") {
        let rest = &message[idx + "Table '".len()..];
        if let Some(end) = rest.find("' doesn't exist") {
            return table_name(&rest[..end]);
        }
    }
    None
}

/// Strip a schema qualifier from `named` and reject an empty table name.
fn table_name(named: &str) -> Option<String> {
    let named = named.trim();
    let table = named.rsplit('.').next().unwrap_or(named);
    (!table.is_empty()).then(|| table.to_string())
}

/// Named-connection resolution errors raised by [`crate::connections`].
///
/// Every variant is a configuration problem detected *before* any socket is
/// opened: an unknown name, a missing required field, or a `driver`/URL scheme
/// disagreement. Driver-level failures surface separately as
/// [`OrmError::Pool`] / [`OrmError::UnsupportedDriver`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConnectionError {
    /// No connection is declared under the requested name.
    #[error("unknown database connection: {0}")]
    UnknownConnection(String),

    /// No connection map exists and no legacy `database.url` fallback is set.
    #[error("no database connection configured")]
    NotConfigured,

    /// The `[database]` config table exists but could not be deserialized.
    #[error("invalid database config: {0}")]
    InvalidConfig(String),

    /// A driver that cannot be mapped onto a supported URL scheme was declared.
    #[error("unsupported database driver `{driver}`")]
    UnsupportedDriver {
        /// The unrecognised driver (or URL scheme) string.
        driver: String,
    },

    /// A granular connection is missing a field required by its driver.
    #[error("connection is missing required field `{field}`")]
    MissingField {
        /// The missing field (e.g. `host`).
        field: String,
    },

    /// The declared `driver` disagrees with the URL scheme.
    #[error("driver `{driver}` does not match URL scheme `{scheme}`")]
    DriverMismatch {
        /// The declared driver.
        driver: String,
        /// The scheme parsed from the connection's `url`.
        scheme: String,
    },
}

/// Strict upsert errors — thrown before any round-trip.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UpsertError {
    /// `upsert(rows, unique_by: [])` — previously silent, now strict.
    #[error("uniqueBy must be non-empty")]
    EmptyUniqueBy,

    /// Upsert row count exceeded the configured batch limit.
    #[error("upsert batch too large: {0} rows")]
    BatchTooLarge(usize),
}

/// Schema-builder errors raised by [`crate::schema`].
///
/// Every variant is detected *before* any SQL is executed: the caller asked for
/// an unsupported dialect, named a table/column with an illegal identifier, or
/// produced a blueprint that cannot be rendered (no columns, duplicate column).
/// The raw DDL string is only produced once validation passes, so a successful
/// [`crate::schema::Schema::create`] always yields executable SQL.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchemaError {
    /// The requested dialect is not one of `sqlite` / `postgres` / `mysql`.
    #[error("unknown dialect: {0}")]
    UnknownDialect(String),

    /// A table name was empty (or whitespace-only).
    #[error("empty table name")]
    EmptyTableName,

    /// A column name was empty (or whitespace-only).
    #[error("empty column name")]
    EmptyColumnName,

    /// A table or column identifier contained characters outside the allow-list.
    #[error("invalid identifier `{identifier}`")]
    InvalidIdentifier {
        /// The offending identifier.
        identifier: String,
    },

    /// A table-level `index`/`unique` declaration carried no columns.
    #[error("index declaration has no columns")]
    EmptyIndexColumns,

    /// A blueprint carried no column definitions.
    #[error("table `{table}` has no columns")]
    EmptyBlueprint {
        /// The table that was left empty.
        table: String,
    },

    /// The same column name was declared twice on one blueprint.
    #[error("duplicate column `{column}` on table `{table}`")]
    DuplicateColumn {
        /// The table being built.
        table: String,
        /// The repeated column name.
        column: String,
    },
}

#[cfg(test)]
mod tests {
    use super::missing_table;

    #[test]
    fn missing_table_names_are_read_from_each_driver() {
        assert_eq!(
            missing_table("no such table: widgets").as_deref(),
            Some("widgets")
        );
        assert_eq!(
            missing_table("no such table: main.widgets").as_deref(),
            Some("widgets")
        );
        assert_eq!(
            missing_table("relation \"public.widgets\" does not exist").as_deref(),
            Some("widgets")
        );
        assert_eq!(
            missing_table("Table 'app.widgets' doesn't exist").as_deref(),
            Some("widgets")
        );
        assert_eq!(missing_table("near \"SELEC\": syntax error"), None);
        assert_eq!(missing_table("no such column: name"), None);
    }

    /// A message reporting that an object already exists is not a missing table.
    #[test]
    fn missing_table_ignores_already_exists() {
        assert_eq!(missing_table("relation \"widgets\" already exists"), None);
        assert_eq!(missing_table("Table 'app.widgets' already exists"), None);
        assert_eq!(missing_table("table widgets already exists"), None);
    }

    /// A driver-wrapped prefix does not defeat the message heuristic.
    #[test]
    fn missing_table_matches_wrapped_messages() {
        assert_eq!(
            missing_table("error returned from database: no such table: widgets").as_deref(),
            Some("widgets")
        );
        assert_eq!(
            missing_table("driver error: relation \"public.widgets\" does not exist").as_deref(),
            Some("widgets")
        );
        assert_eq!(
            missing_table("driver error: Table 'app.widgets' doesn't exist").as_deref(),
            Some("widgets")
        );
    }
}

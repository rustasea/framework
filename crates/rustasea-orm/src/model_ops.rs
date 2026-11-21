//! Async model persistence — create/save/update/delete/refresh over a [`DbPool`].
//!
//! [`ModelOps`] is blanket-implemented for every [`Model`]. Column values are
//! taken from the model's `serde` representation, so the operations stay generic
//! across derived models; `created_at`/`updated_at` are populated on insert and
//! `updated_at` is refreshed on update, each rendered for the runtime dialect.
//! A timestamp the caller already supplied is preserved (Laravel's
//! `updateTimestamps` only fills a stamp that is not already set). Only the
//! `sqlx` runtime API is used — never the compile-time `query!` macros.
//!
//! The single-key path (one `id` column) and the composite-key path (a declared
//! [`Model::primary_key_columns`]) share this trait: `create`/`save`/`update`
//! dispatch on the key shape, while `delete`/`refresh` accept either a bare
//! `Uuid` (single key) or a `&[Value]` tuple (via the `*_by_key` variants).

use crate::db::DbPool;
use crate::error::{OrmError, Result};
use crate::model::Model;
use crate::types::Value;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

mod activity_hooks;
mod cache_hooks;
mod cascade;
mod composite;
mod key;
mod restore;
mod slug_hooks;
mod timestamps;
mod write;

use key::KeyFilter;
pub use slug_hooks::SluggableFind;

/// Async persistence operations shared by every model.
#[allow(async_fn_in_trait)]
pub trait ModelOps: Model + Sized {
    /// Insert `data`, auto-assigning an id and the timestamp columns.
    ///
    /// Returns the persisted row re-read from the database.
    async fn create(pool: &DbPool, mut data: Self) -> Result<Self>
    where
        Self: Serialize + DeserializeOwned,
    {
        if !Self::has_composite_primary_key() && data.primary_key() == Uuid::nil() {
            data.assign_id();
        }
        let key = KeyFilter::of(&data);
        slug_hooks::prepare_insert(pool, &mut data).await?;
        let recorder = activity_hooks::recorder::<Self>();
        let (sql, bindings) = if Self::has_composite_primary_key() {
            composite::build_insert::<Self>(&data, pool.dialect())?
        } else {
            write::build_insert::<Self>(&data, pool.dialect())?
        };
        pool.execute_bind(&sql, &bindings).await?;
        let (persisted, snapshot) =
            activity_hooks::refresh_with_snapshot::<Self>(pool, &key, recorder.as_ref(), data)
                .await?;
        if let Some(recorder) = recorder {
            activity_hooks::record_created::<Self>(&recorder, &key.event_id(), snapshot).await?;
        }
        cache_hooks::invalidate_table(&Self::table_name());
        Ok(persisted)
    }

    /// Persist `data` with a single atomic upsert (insert-or-update).
    ///
    /// Replaces the former exists-then-branch check (a TOCTOU race): the
    /// database resolves insert-vs-update in one statement — Postgres/SQLite
    /// `INSERT … ON CONFLICT (<key>) DO UPDATE`, MySQL `INSERT … ON DUPLICATE KEY
    /// UPDATE`. The row is re-read with [`ModelOps::refresh`] afterwards.
    async fn save(pool: &DbPool, mut data: Self) -> Result<Self>
    where
        Self: Serialize + DeserializeOwned,
    {
        if !Self::has_composite_primary_key() && data.primary_key() == Uuid::nil() {
            data.assign_id();
        }
        let key = KeyFilter::of(&data);
        // The upsert is atomic, so a slug is generated with insert semantics
        // (fresh base + uniqueness). An existing row keeping its slug is
        // preserved only when the caller leaves the slug untouched and
        // `on_update` is off; use `update` for change-aware regeneration.
        slug_hooks::prepare_insert(pool, &mut data).await?;
        let recorder = activity_hooks::recorder::<Self>();
        // When auditing, read the prior row first so the event can be classified
        // as Created or Updated (the upsert itself is atomic).
        let old = match &recorder {
            Some(_) => activity_hooks::snapshot::<Self>(pool, &key).await?,
            None => None,
        };
        let (sql, bindings) = if Self::has_composite_primary_key() {
            composite::build_upsert::<Self>(&data, pool.dialect())?
        } else {
            write::build_upsert::<Self>(&data, pool.dialect())?
        };
        pool.execute_bind(&sql, &bindings).await?;
        let (persisted, snapshot) =
            activity_hooks::refresh_with_snapshot::<Self>(pool, &key, recorder.as_ref(), data)
                .await?;
        if let Some(recorder) = recorder {
            if old.is_some() {
                activity_hooks::record_updated::<Self>(
                    &recorder,
                    &key.event_id(),
                    old,
                    snapshot,
                    crate::activity::ActivityOperation::Updated,
                )
                .await?;
            } else {
                activity_hooks::record_created::<Self>(&recorder, &key.event_id(), snapshot)
                    .await?;
            }
        }
        cache_hooks::invalidate_table(&Self::table_name());
        Ok(persisted)
    }

    /// Update the row identified by the model's primary key.
    ///
    /// Bumps `updated_at` when the model tracks timestamps; returns the
    /// re-read persisted row.
    async fn update(pool: &DbPool, mut data: Self) -> Result<Self>
    where
        Self: Serialize + DeserializeOwned,
    {
        let key = KeyFilter::of(&data);
        let recorder = activity_hooks::recorder::<Self>();
        // Only pre-read the prior row when auditing is on, so a non-opted model
        // pays no extra round-trip.
        let old = match &recorder {
            Some(_) => activity_hooks::snapshot::<Self>(pool, &key).await?,
            None => None,
        };
        // Regenerate the slug only when the model opts into `on_update`; the
        // pre-read source values let the hook skip an unchanged source.
        let slug_old = match Self::sluggable() && Self::slug_options().on_update {
            true => Self::refresh_by_key(pool, &key.values)
                .await?
                .map(|row| row.slug_source_values()),
            false => None,
        };
        slug_hooks::prepare_update(pool, &mut data, slug_old.as_deref()).await?;
        let (sql, bindings) = if Self::has_composite_primary_key() {
            composite::build_update::<Self>(&data, pool.dialect())?
        } else {
            write::build_update::<Self>(&data, pool.dialect())?
        };
        let affected = pool.execute_bind(&sql, &bindings).await?;
        if affected == 0 {
            return Err(OrmError::NotFound);
        }
        let (persisted, snapshot) =
            activity_hooks::refresh_with_snapshot::<Self>(pool, &key, recorder.as_ref(), data)
                .await?;
        if let Some(recorder) = recorder {
            activity_hooks::record_updated::<Self>(
                &recorder,
                &key.event_id(),
                old,
                snapshot,
                crate::activity::ActivityOperation::Updated,
            )
            .await?;
        }
        cache_hooks::invalidate_table(&Self::table_name());
        Ok(persisted)
    }

    /// Delete the row: soft delete when the model soft-deletes, else hard delete.
    async fn delete(pool: &DbPool, id: Uuid) -> Result<bool> {
        Self::delete_by_key(pool, &[Value::Uuid(id)]).await
    }

    /// Delete the row addressed by a primary-key value tuple.
    ///
    /// Soft delete when the model soft-deletes, else hard delete. The tuple
    /// length must match [`Model::primary_key_columns`]; a composite model uses
    /// this instead of the single-`Uuid` [`ModelOps::delete`].
    async fn delete_by_key(pool: &DbPool, key: &[Value]) -> Result<bool> {
        if Self::uses_soft_deletes() {
            Self::soft_delete_by_key(pool, key).await
        } else {
            Self::force_delete_by_key(pool, key).await
        }
    }

    /// Permanently remove the row, bypassing soft deletes.
    async fn force_delete(pool: &DbPool, id: Uuid) -> Result<bool> {
        Self::force_delete_by_key(pool, &[Value::Uuid(id)]).await
    }

    /// Permanently remove the row addressed by a primary-key value tuple.
    async fn force_delete_by_key(pool: &DbPool, key: &[Value]) -> Result<bool> {
        let filter = resolve_key_filter::<Self>(key)?;
        let cascading = Self::cascade_soft_deletes();
        // Group the parent delete with its cascaded children under one batch id.
        let _batch =
            cascading.then(|| crate::activity::BatchScope::new(Uuid::now_v7().to_string()));
        let recorder = activity_hooks::recorder::<Self>();
        let old = match &recorder {
            Some(_) => activity_hooks::snapshot::<Self>(pool, &filter).await?,
            None => None,
        };
        // Capture the parent before it is removed, so the fan-out still sees the
        // foreign-key values it must match on.
        let cascade_parent = if cascading {
            cascade::parent_snapshot::<Self>(pool, &filter).await?
        } else {
            None
        };
        let (sql, bindings) = composite::build_delete::<Self>(&filter);
        let affected = pool.execute_bind(&sql, &bindings).await?;
        if let Some(recorder) = recorder {
            if affected > 0 {
                activity_hooks::record_deleted::<Self>(&recorder, &filter.event_id(), old, None)
                    .await?;
            }
        }
        if affected > 0 {
            if let Some(parent) = cascade_parent {
                let mut visited = HashSet::new();
                cascade::cascade::<Self>(
                    pool,
                    &parent,
                    cascade::CascadeMode::ForceDelete,
                    0,
                    &mut visited,
                )
                .await?;
            }
        }
        cache_hooks::invalidate_table(&Self::table_name());
        Ok(affected > 0)
    }

    /// Soft-delete the row by setting `deleted_at` to now.
    ///
    /// The timestamp expression follows the runtime pool dialect, not the
    /// compile-time feature set: SQLite binds a fixed-width RFC3339 string
    /// while Postgres/MySQL use the native `NOW()` function. This keeps soft
    /// deletes working when `postgres` is enabled but an SQLite pool is in use.
    async fn soft_delete(pool: &DbPool, id: Uuid) -> Result<bool> {
        Self::soft_delete_by_key(pool, &[Value::Uuid(id)]).await
    }

    /// Soft-delete the row addressed by a primary-key value tuple.
    async fn soft_delete_by_key(pool: &DbPool, key: &[Value]) -> Result<bool> {
        let filter = resolve_key_filter::<Self>(key)?;
        let cascading = Self::cascade_soft_deletes();
        // Group the parent soft delete with its cascaded children under one batch.
        let _batch =
            cascading.then(|| crate::activity::BatchScope::new(Uuid::now_v7().to_string()));
        let recorder = activity_hooks::recorder::<Self>();
        let old = match &recorder {
            Some(_) => activity_hooks::snapshot::<Self>(pool, &filter).await?,
            None => None,
        };
        // Capture the parent before it is stamped, so the fan-out still sees the
        // foreign-key values and the pre-delete `deleted_at`.
        let cascade_parent = if cascading {
            cascade::parent_snapshot::<Self>(pool, &filter).await?
        } else {
            None
        };
        let (sql, bindings) = composite::build_soft_delete::<Self>(&filter, pool.dialect());
        let affected = pool.execute_bind(&sql, &bindings).await?;
        if let Some(recorder) = recorder {
            if affected > 0 {
                // The row still exists with `deleted_at` set, so the event
                // carries both the pre- and post-delete snapshots.
                let new = activity_hooks::snapshot::<Self>(pool, &filter).await?;
                activity_hooks::record_deleted::<Self>(&recorder, &filter.event_id(), old, new)
                    .await?;
            }
        }
        if affected > 0 {
            if let Some(parent) = cascade_parent {
                let mut visited = HashSet::new();
                cascade::cascade::<Self>(
                    pool,
                    &parent,
                    cascade::CascadeMode::SoftDelete,
                    0,
                    &mut visited,
                )
                .await?;
            }
        }
        cache_hooks::invalidate_table(&Self::table_name());
        Ok(affected > 0)
    }

    /// Restore the row by clearing its soft-delete marker.
    async fn restore(pool: &DbPool, id: Uuid) -> Result<bool> {
        Self::restore_by_key(pool, &[Value::Uuid(id)]).await
    }

    /// Restore the row addressed by a primary-key value tuple.
    ///
    /// Clears `deleted_at`; when the model declares `#[cascade_soft_deletes]`,
    /// children soft-deleted together with the parent are restored too, while
    /// rows deleted independently (earlier stamp) stay trashed. The tuple length
    /// must match [`Model::primary_key_columns`].
    async fn restore_by_key(pool: &DbPool, key: &[Value]) -> Result<bool> {
        let filter = resolve_key_filter::<Self>(key)?;
        restore::restore_by_key::<Self>(pool, &filter).await
    }

    /// Load the row with primary key `id`, or `None` (Laravel `find`).
    ///
    /// Goes through [`Model::query`], so global scopes apply and a
    /// soft-deleted row is not found; [`ModelOps::refresh`] reads it anyway.
    async fn find(pool: &DbPool, id: Uuid) -> Result<Option<Self>>
    where
        Self: DeserializeOwned,
    {
        match Self::query().where_key(id).first(pool).await? {
            Some(value) => Ok(Some(crate::casts::hydrate::<Self>(value)?)),
            None => Ok(None),
        }
    }

    /// Re-read the row by primary key, including soft-deleted rows.
    async fn refresh(pool: &DbPool, id: Uuid) -> Result<Option<Self>>
    where
        Self: DeserializeOwned,
    {
        Self::refresh_by_key(pool, &[Value::Uuid(id)]).await
    }

    /// Re-read the row addressed by a primary-key value tuple (trashed rows
    /// included).
    async fn refresh_by_key(pool: &DbPool, key: &[Value]) -> Result<Option<Self>>
    where
        Self: DeserializeOwned,
    {
        let filter = resolve_key_filter::<Self>(key)?;
        match refresh_raw_by_key::<Self>(pool, &filter).await? {
            Some(value) => Ok(Some(crate::casts::hydrate::<Self>(value)?)),
            None => Ok(None),
        }
    }

    /// Load the row `FOR UPDATE`, returning [`OrmError::NotFound`] when absent.
    ///
    /// The pessimistic lock follows the runtime pool dialect, not the
    /// compile-time feature set: SQLite has no row locks and surfaces
    /// [`OrmError::UnsupportedDriver`] before any round-trip, even when the
    /// `postgres`/`mysql` features are compiled in.
    async fn first_for_update(pool: &DbPool, id: Uuid) -> Result<Self>
    where
        Self: DeserializeOwned,
    {
        let row = <Self as Model>::first_for_update(id, pool.dialect())?
            .first(pool)
            .await?;
        match row {
            Some(value) => crate::casts::hydrate::<Self>(value),
            None => Err(OrmError::NotFound),
        }
    }

    /// Load the row addressed by a primary-key tuple `FOR UPDATE`.
    async fn first_for_update_by_key(pool: &DbPool, key: &[Value]) -> Result<Self>
    where
        Self: DeserializeOwned,
    {
        let filter = resolve_key_filter::<Self>(key)?;
        let builder = filter
            .apply(
                crate::builder::QueryBuilder::table(Self::table_name())
                    .with_global_scopes(Self::global_scopes()),
            )
            .for_update_with_dialect(pool.dialect())?;
        match builder.first(pool).await? {
            Some(value) => crate::casts::hydrate::<Self>(value),
            None => Err(OrmError::NotFound),
        }
    }

    /// Invalidate every cached query on this model's table (ADOPT-019).
    ///
    /// Bumps the table generation (so existing keys miss) and flushes the store's
    /// table scope when one is installed. A cache-free build is a no-op.
    async fn flush_cache() -> Result<()> {
        crate::cache::flush_model_cache(&Self::table_name()).await
    }
}

/// Resolve a caller-supplied key tuple into a validated [`KeyFilter`] for `T`.
///
/// A tuple whose length disagrees with [`Model::primary_key_columns`] is a
/// typed [`OrmError::InvalidState`], so a composite row is never addressed by a
/// partial key.
fn resolve_key_filter<T: Model>(key: &[Value]) -> Result<KeyFilter> {
    let columns = T::primary_key_columns();
    if key.len() != columns.len() {
        return Err(OrmError::InvalidState(format!(
            "primary key arity mismatch: {} columns, {} values",
            columns.len(),
            key.len()
        )));
    }
    Ok(KeyFilter {
        columns: columns.to_vec(),
        values: key.to_vec(),
    })
}

impl<T: Model> ModelOps for T {}

/// Re-read a row by primary-key filter as raw JSON, including trashed rows.
///
/// The write path calls this once after a mutation so the same row can serve
/// both the hydrated return value and the activity snapshot — avoiding a second
/// SELECT when auditing is on.
async fn refresh_raw_by_key<T: Model>(
    pool: &DbPool,
    key: &KeyFilter,
) -> Result<Option<serde_json::Value>> {
    key.apply(<T as Model>::query_with_trashed())
        .first(pool)
        .await
}

#[cfg(test)]
mod tests;

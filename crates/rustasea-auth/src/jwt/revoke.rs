//! Token revocation for [`super::JwtGuard`] — logout and single-use refresh.
//!
//! Each login mints an access/refresh pair that shares one session id (`sid`).
//! Revoking a token revokes its whole pair: the `sid` is written to the
//! guard's [`Store`] until the refresh token would have expired, and every
//! `parse`/`refresh` rejects a token whose `sid` is present. Tokens issued
//! before `sid` existed are revoked individually, keyed by a hash of the
//! token, until their own `exp`.
//!
//! The default store is a per-process [`rustasea_cache::memory::MemoryStore`],
//! so a revocation is only seen by the process that recorded it. Deployments
//! with several processes pass a shared store (Redis) through
//! [`super::JwtGuard::with_revocation_store`].

use std::time::Duration;

use chrono::Utc;
use rustasea_cache::store::Store;
use sha2::{Digest, Sha256};

use super::JwtClaims;
use crate::error::{AuthError, Result};

/// Cache key prefix for revoked token pairs and tokens.
const KEY_PREFIX: &str = "jwt:revoked:";

/// The store key that marks the token (or its pair) as revoked.
fn key(claims: &JwtClaims, token: &str) -> String {
    if claims.sid.is_empty() {
        format!("{KEY_PREFIX}token:{}", hex::encode(Sha256::digest(token)))
    } else {
        format!("{KEY_PREFIX}sid:{}", claims.sid)
    }
}

/// How long the revocation must be kept: until no token it covers can still
/// verify. A pair lives until its refresh token expires (`iat` + refresh TTL).
fn ttl(claims: &JwtClaims, refresh_ttl_secs: u64) -> Duration {
    let until = if claims.sid.is_empty() {
        claims.exp as u64
    } else {
        (claims.iat as u64 + refresh_ttl_secs).max(claims.exp as u64)
    };
    let now = Utc::now().timestamp().max(0) as u64;
    Duration::from_secs(until.saturating_sub(now).max(1))
}

/// Fail with [`AuthError::InvalidToken`] when the token has been revoked.
///
/// A store failure is [`AuthError::StoreUnavailable`]: revocation state that
/// cannot be read must not let a token through.
pub(super) async fn ensure_active(
    store: &dyn Store,
    claims: &JwtClaims,
    token: &str,
) -> Result<()> {
    match store.get(&key(claims, token)).await {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(AuthError::InvalidToken),
        Err(_) => Err(AuthError::StoreUnavailable),
    }
}

/// Revoke the token (and its pair). Revoking twice is not an error.
pub(super) async fn revoke(
    store: &dyn Store,
    claims: &JwtClaims,
    token: &str,
    refresh_ttl_secs: u64,
) -> Result<()> {
    store
        .put(&key(claims, token), vec![1], ttl(claims, refresh_ttl_secs))
        .await
        .map_err(|_| AuthError::StoreUnavailable)
}

/// Revoke the token (and its pair) only if it was not already revoked.
///
/// The check and the write are one atomic `put_if_absent`, so two concurrent
/// refreshes with the same token cannot both succeed: the loser gets
/// [`AuthError::InvalidToken`].
pub(super) async fn claim_once(
    store: &dyn Store,
    claims: &JwtClaims,
    token: &str,
    refresh_ttl_secs: u64,
) -> Result<()> {
    let stored = store
        .put_if_absent(&key(claims, token), vec![1], ttl(claims, refresh_ttl_secs))
        .await
        .map_err(|_| AuthError::StoreUnavailable)?;
    if stored {
        Ok(())
    } else {
        Err(AuthError::InvalidToken)
    }
}

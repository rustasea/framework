//! Unit tests for the HS256 JWT guard.

use super::*;

/// Seconds since the UNIX epoch, used to force token expiry in tests.
fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct AlwaysVerify;

impl PasswordVerifier for AlwaysVerify {
    fn verify(&self, _hash: &str, _password: &str) -> bool {
        true
    }

    fn hash(&self, _password: &str) -> crate::error::Result<String> {
        Err(AuthError::Hash("AlwaysVerify does not hash".into()))
    }
}

fn test_guard() -> JwtGuard {
    JwtGuard::new(
        JwtConfig {
            secret: "0123456789abcdef0123456789abcdef".into(),
            allow_login_using_id: true,
            ..JwtConfig::default()
        },
        Arc::new(AlwaysVerify),
    )
}

/// A signed access token round-trips through `parse`.
#[tokio::test]
async fn login_then_parse_round_trip() {
    let guard = test_guard();
    let id = Uuid::new_v4();
    let token = guard
        .login_using_id(&id.to_string())
        .await
        .expect("login_using_id succeeds");
    assert_eq!(token.token_type, "Bearer");
    let user = guard
        .parse(&token.access_token)
        .await
        .expect("parse succeeds");
    assert_eq!(user.id, id.to_string());
    assert_eq!(user.guard, "jwt");
}

/// The shipped default secret is rejected by validation.
#[test]
fn validate_rejects_default_secret_and_short_secret() {
    let default_cfg = JwtConfig::default();
    assert_eq!(
        default_cfg.validate(),
        Err(AuthError::Disabled(
            "JWT secret is still the default 'change-me-in-production'; set a strong secret in production"
                .to_string()
        ))
    );
    let short_cfg = JwtConfig {
        secret: "short".into(),
        ..JwtConfig::default()
    };
    assert!(short_cfg.validate().is_err());
    let strong_cfg = JwtConfig {
        secret: "0123456789abcdef0123456789abcdef".into(),
        ..JwtConfig::default()
    };
    assert!(strong_cfg.validate().is_ok());
}

/// A guard built from the default config refuses every entry point with
/// `AuthError::Disabled` before any credential/token work happens.
#[tokio::test]
async fn default_config_refuses_all_entry_points() {
    let guard = JwtGuard::new(JwtConfig::default(), Arc::new(AlwaysVerify));
    let creds = Credentials {
        email: "ada@example.com".into(),
        password: "correct horse battery staple".into(),
    };
    let disabled = || {
        Err(AuthError::Disabled(
            "JWT secret is still the default 'change-me-in-production'; set a strong secret in production"
                .to_string(),
        ))
    };
    let expected_token: crate::error::Result<Token> = disabled();
    let expected_user: crate::error::Result<AuthUser> = Err(match disabled() {
        Err(e) => e,
        Ok(_) => unreachable!(),
    });
    assert_eq!(guard.login(&creds).await, expected_token);
    assert_eq!(
        guard
            .login_using_id("00000000-0000-0000-0000-000000000000")
            .await,
        expected_token
    );
    assert_eq!(guard.parse("a.b.c").await, expected_user);
    assert_eq!(guard.refresh("a.b.c").await, expected_token);
}

/// An empty HMAC secret is rejected by the key constructors before any
/// encode/decode runs, pinning the jsonwebtoken-12 `Result` mapping: signing
/// surfaces `Token`, decoding surfaces `InvalidToken`.
#[test]
fn empty_secret_maps_to_token_errors() {
    let guard = JwtGuard::new(
        JwtConfig {
            secret: String::new(),
            ..JwtConfig::default()
        },
        Arc::new(AlwaysVerify),
    );
    let claims = JwtClaims::issue(&Uuid::new_v4(), "access", 60, &guard.cfg);
    let err = guard.sign(&claims).expect_err("empty secret must not sign");
    assert!(matches!(err, AuthError::Token(_)));
    let err = guard
        .decode_claims("a.b.c")
        .expect_err("empty secret must not decode");
    assert_eq!(err, AuthError::InvalidToken);
}

/// A refresh token rotates into a new pair; access token in refresh fails.
#[tokio::test]
async fn refresh_rotates_pair() {
    let guard = test_guard();
    let id = Uuid::new_v4();
    let token = guard
        .login_using_id(&id.to_string())
        .await
        .expect("login_using_id succeeds");
    // Ensure the rotation lands on a different `iat` second so the signed
    // output differs (claims carry second-granularity timestamps).
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let rotated = guard
        .refresh(&token.refresh_token)
        .await
        .expect("refresh succeeds");
    assert_ne!(rotated.access_token, token.access_token);
    assert_ne!(rotated.refresh_token, token.refresh_token);
    let err = guard
        .refresh(&token.access_token)
        .await
        .expect_err("access token must be rejected");
    assert_eq!(err, AuthError::InvalidToken);
}

/// An expired token maps to `ExpiredToken`, garbage to `InvalidToken`.
#[tokio::test]
async fn expired_and_malformed_classify() {
    let guard = test_guard();
    let id = Uuid::new_v4();
    let claims = JwtClaims::issue(&id, "access", 1, &guard.cfg);
    let token = guard.sign(&claims).expect("signing succeeds");
    // Force expiry by re-signing with an exp in the past.
    let mut past = claims;
    past.exp = now_secs() as usize - 10;
    let expired = guard.sign(&past).expect("signing succeeds");
    let err = guard
        .parse(&expired)
        .await
        .expect_err("expired token must be rejected");
    assert_eq!(err, AuthError::ExpiredToken);
    let _ = token;
    let err = guard
        .parse("garbage.token.here")
        .await
        .expect_err("garbage token must be rejected");
    assert_eq!(err, AuthError::InvalidToken);
}

/// `parse` resolves `email_verified_at` fresh from the lookup; a guard with
/// no verification-aware lookup fails closed (`None`).
#[tokio::test]
async fn parse_resolves_email_verified_at_from_lookup() {
    let id = Uuid::new_v4();
    let registry = Arc::new(crate::users::MemoryUserRegistry::default());
    registry.seed(crate::users::AuthUserRecord {
        id: id.to_string(),
        email: "ada@example.com".into(),
        password_hash: "phc$hash".into(),
        email_verified_at: Some("2026-01-01T00:00:00Z".into()),
        timezone: None,
    });
    let guard = test_guard().with_lookup(registry);

    let token = guard
        .login_using_id(&id.to_string())
        .await
        .expect("login_using_id succeeds");
    let user = guard.parse(&token.access_token).await.expect("parse");
    assert_eq!(
        user.email_verified_at.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
    assert!(user.is_email_verified());
    // The JWT guard never carries a session-backed confirmation.
    assert_eq!(user.password_confirmed_at, None);

    // Without a lookup the field fails closed.
    let bare = test_guard();
    let bare_token = bare
        .login_using_id(&id.to_string())
        .await
        .expect("login_using_id succeeds");
    let bare_user = bare.parse(&bare_token.access_token).await.expect("parse");
    assert_eq!(bare_user.email_verified_at, None);
    assert!(!bare_user.is_email_verified());
}

/// `logout` revokes the presented token's whole pair, from either half.
#[tokio::test]
async fn logout_revokes_the_pair() {
    let guard = test_guard();
    let id = Uuid::new_v4().to_string();

    let by_access = guard.login_using_id(&id).await.expect("login");
    guard.logout(&by_access.access_token).await.expect("logout");
    assert_eq!(
        guard.parse(&by_access.access_token).await,
        Err(AuthError::InvalidToken)
    );
    assert_eq!(
        guard.refresh(&by_access.refresh_token).await,
        Err(AuthError::InvalidToken)
    );
    // Logging out twice is not an error.
    guard
        .logout(&by_access.access_token)
        .await
        .expect("second logout");

    let by_refresh = guard.login_using_id(&id).await.expect("login");
    guard
        .logout(&by_refresh.refresh_token)
        .await
        .expect("logout");
    assert_eq!(
        guard.parse(&by_refresh.access_token).await,
        Err(AuthError::InvalidToken)
    );

    // Other sessions of the same user are untouched.
    let other = guard.login_using_id(&id).await.expect("login");
    assert!(guard.parse(&other.access_token).await.is_ok());
}

/// A refresh token works once; refreshing also retires the old access token.
#[tokio::test]
async fn refresh_tokens_are_single_use() {
    let guard = test_guard();
    let token = guard
        .login_using_id(&Uuid::new_v4().to_string())
        .await
        .expect("login");

    let rotated = guard
        .refresh(&token.refresh_token)
        .await
        .expect("first refresh");
    assert_eq!(
        guard.refresh(&token.refresh_token).await,
        Err(AuthError::InvalidToken)
    );
    assert_eq!(
        guard.parse(&token.access_token).await,
        Err(AuthError::InvalidToken)
    );
    assert!(guard.parse(&rotated.access_token).await.is_ok());
}

/// `logout` of an expired token is a no-op; a malformed token is rejected.
#[tokio::test]
async fn logout_ignores_expired_and_rejects_malformed_tokens() {
    let guard = test_guard();
    let mut claims = JwtClaims::issue(&Uuid::new_v4(), "access", 60, &guard.cfg);
    claims.exp = (now_secs() - 120) as usize;
    let expired = guard.sign(&claims).expect("sign");
    assert_eq!(guard.logout(&expired).await, Ok(()));
    assert_eq!(guard.logout("a.b.c").await, Err(AuthError::InvalidToken));
}

/// A token issued before `sid` existed is revoked on its own.
#[tokio::test]
async fn tokens_without_a_session_id_are_revoked_individually() {
    let guard = test_guard();
    let user = Uuid::new_v4();
    let first = guard
        .sign(&JwtClaims::issue(&user, "access", 600, &guard.cfg))
        .expect("sign");
    let mut later = JwtClaims::issue(&user, "access", 600, &guard.cfg);
    later.iat -= 1;
    let second = guard.sign(&later).expect("sign");

    guard.logout(&first).await.expect("logout");
    assert_eq!(guard.parse(&first).await, Err(AuthError::InvalidToken));
    assert!(guard.parse(&second).await.is_ok());
}

/// Revocations live in the guard's store: a shared store carries them across
/// guards (processes); the default per-guard memory store does not.
#[tokio::test]
async fn revocations_are_shared_through_the_store() {
    let shared: Arc<dyn Store> = Arc::new(MemoryStore::new());
    let first = test_guard().with_revocation_store(Arc::clone(&shared));
    let second = test_guard().with_revocation_store(shared);
    let isolated = test_guard();

    let token = first
        .login_using_id(&Uuid::new_v4().to_string())
        .await
        .expect("login");
    first.logout(&token.access_token).await.expect("logout");
    assert_eq!(
        second.parse(&token.access_token).await,
        Err(AuthError::InvalidToken)
    );
    assert!(isolated.parse(&token.access_token).await.is_ok());
}

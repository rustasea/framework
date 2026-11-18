//! RustaSea auth and security crate — guards, CSRF, throttling.
//!
//! Security boundary for M3 (Laravel-13 hardened defaults):
//! - JWT guard (HS256 via `jsonwebtoken`) + real `tower-sessions` session guard
//!   (store-backed login/parse/refresh/logout with id rotation)
//! - `Auth` manager: named guard registration, `Auth::extend`, typed `GuardMismatch`
//! - Origin-aware `PreventRequestForgery` (`Sec-Fetch-Site` + allow-list)
//! - `RateLimiter` trait + in-memory `Throttle` (`429` + `Retry-After`)
//! - Session/cache hardening: JSON serialization, hyphenated prefixes,
//!   `serializable_classes` allow-list
//!
//! Per ADR-0007 there are no global facades: an `AuthManager`/`SessionPolicy`
//! lives inside `AppState` and flows through `axum::extract::State`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod authorize;
pub mod config;
pub mod csrf;
pub mod error;
pub mod gate;
pub mod guard;
pub mod jwt;
pub mod passkeys;
pub mod password_reset;
pub mod policy;
pub mod rbac;
pub mod session;
pub mod session_cookie;
pub mod signed_url;
pub mod throttle;
pub mod two_factor;
pub mod users;
pub mod verification;
pub mod verify;

pub use authorize::{authorizer_for, authorizer_for_abilities, AbilityGateResource, GateResource};
pub use config::{
    password_timeout_secs, AuthConfig, AuthDefaults, ConfigResult, FortifyConfig,
    FortifyFeaturesConfig, FortifyLimiterConfig, FortifyPasskeyFeatureConfig,
    FortifyPasskeysConfig, FortifyTwoFactorConfig, GuardConfig, PasswordBrokerConfig,
    ProviderConfig, ResolvedPasskeys, SessionConfig, DEFAULT_PASSWORD_TIMEOUT_SECS,
};
pub use csrf::{CsrfLayer, PreventRequestForgery, SecFetchSite};
pub use error::{AuthConfigError, AuthError, CsrfError, SerializationError, ThrottleError};
pub use gate::{AbilityCallback, AfterCallback, AuthorizationError, BeforeCallback, Gate};
pub use guard::{Auth, AuthManager, AuthUser, Credentials, Guard, GuardRegistrar, Token};
pub use jwt::{JwtClaims, JwtConfig, JwtGuard};
pub use passkeys::{
    AuthenticationResponse, ChallengeStore, DenyAllChallengeStore, DenyAllPasskeyStore,
    MemoryChallengeStore, MemoryPasskeyStore, PasskeyCredential, PasskeyService, PasskeyStore,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions, RegistrationResponse,
};
pub use password_reset::{
    generate_token, DenyAllResetStore, MemoryPasswordResetStore, PasswordResetRecord,
    PasswordResetStore, RESET_TOKEN_LEN,
};
pub use policy::{Policy, PolicyAction, PolicyRegistry};
pub use rbac::{HasRoles, PermissionResolver, RbacError, RbacRegistry, Role};
pub use session::{
    DatabaseSessionStore, DeserializationAllowList, SessionGuard, SessionPolicy, SessionUser,
};
pub use session_cookie::SessionCookieConfig;
pub use signed_url::{SignedUrlError, SignedUrlSigner};
pub use throttle::defaults::{
    kit_limiters, login_definition, normalize_username, passkeys_definition, two_factor_definition,
    LOGIN, PASSKEYS, TWO_FACTOR,
};
pub use throttle::layer::{ThrottleLayer, ThrottleService};
pub use throttle::limiter::MemoryRateLimiter;
pub use throttle::registry::{LimiterDefinition, LimiterError, LimiterInput, RateLimiterRegistry};
pub use throttle::route::{
    limiter_input, named_limiter_layer, ThrottleCredentialId, ThrottleSessionId, ThrottleUsername,
};
pub use throttle::{BucketState, KeyBy, Limit, RateLimiter, ThrottleConfig, ThrottleDecision};
pub use two_factor::{
    code_at, generate_secret, otpauth_uri, verify_code, DenyAllTwoFactorStore,
    MemoryTwoFactorStore, SecretCipher, TwoFactorEnrollment, TwoFactorRecord, TwoFactorService,
    TwoFactorStore, DEFAULT_WINDOW, DIGITS, RECOVERY_CODE_COUNT, SECRET_BYTES, STEP_SECS,
};
pub use users::provider::{
    DatabaseUserProvider, DenyAllProvider, MemoryUserProvider, NewUserRecord, UserProvider,
};
pub use verification::{EmailVerification, MemoryEmailVerification};

/// Alias kept for documentation parity (`Auth::guard(...)`).
pub type Result<T> = error::Result<T>;

/// Reserved for tests that exercise the deny-list on `logout` (S05).
pub const _RESERVED: u8 = 0;

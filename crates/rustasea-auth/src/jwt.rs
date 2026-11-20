/// JWT guard — HS256 signed claims via `jsonwebtoken`.
///
/// `login` verifies the password through the injected `PasswordVerifier`,
/// then signs an access/refresh pair sharing one session id (`sid`). `parse`
/// decodes with `exp` validation and rejects revoked tokens. Rotation
/// (`refresh`) re-signs from a still-valid refresh token's subject without
/// requiring the password again and revokes the old pair, so a refresh token
/// works once. `logout` revokes the presented token's pair.
use std::sync::Arc;

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rustasea_cache::memory::MemoryStore;
use rustasea_cache::store::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AuthError, Result};
use crate::guard::{AuthUser, Credentials, Guard, Token};
use crate::verify::PasswordVerifier;

/// Configuration for the JWT guard (HS256).
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// HMAC secret shared between signer and verifier.
    pub secret: String,
    /// Access-token lifetime in seconds.
    pub access_ttl_secs: u64,
    /// Refresh-token lifetime in seconds.
    pub refresh_ttl_secs: u64,
    /// `iss` claim emitted on every token.
    pub issuer: String,
    /// `aud` claim emitted on every token.
    pub audience: String,
    /// Gate for the internal/testing `loginUsingId` helper.
    pub allow_login_using_id: bool,
}

impl Default for JwtConfig {
    /// Defaults for local development — override `secret` in production.
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            access_ttl_secs: 3600,
            refresh_ttl_secs: 86_400,
            issuer: "rustasea".to_string(),
            audience: "rustasea-app".to_string(),
            allow_login_using_id: false,
        }
    }
}

impl JwtConfig {
    /// Placeholder secret shipped in the default config — must never sign
    /// tokens in a production environment.
    pub const DEFAULT_SECRET: &'static str = "change-me-in-production";
    /// Minimum accepted HMAC secret length (bytes).
    pub const MIN_SECRET_LEN: usize = 32;

    /// Verify the signing secret is safe to use.
    ///
    /// Rejects the literal default and any secret shorter than
    /// `MIN_SECRET_LEN` bytes. Callers guard every token-producing or
    /// -consuming entry point with this before touching the key material;
    /// the guard is permanently disabled (`AuthError::Disabled`) otherwise.
    pub fn validate(&self) -> std::result::Result<(), AuthError> {
        if self.secret == Self::DEFAULT_SECRET {
            return Err(AuthError::Disabled(
                "JWT secret is still the default 'change-me-in-production'; set a strong secret in production".into(),
            ));
        }
        if self.secret.len() < Self::MIN_SECRET_LEN {
            return Err(AuthError::Disabled(format!(
                "JWT secret must be at least {} bytes long",
                Self::MIN_SECRET_LEN
            )));
        }
        Ok(())
    }
}

/// Claims embedded in a signed JWT (contract `jwt-claims.schema.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JwtClaims {
    /// Subject — the user UUID.
    pub sub: String,
    /// Expiry (UNIX seconds).
    pub exp: usize,
    /// Issued-at (UNIX seconds).
    pub iat: usize,
    /// Issuer.
    pub iss: String,
    /// Audience.
    pub aud: String,
    /// Token purpose: `access` or `refresh`.
    pub typ: String,
    /// Session id shared by an access/refresh pair; revoking it revokes both.
    /// Empty on tokens issued before it existed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sid: String,
}

impl JwtClaims {
    /// Sign a fresh claim set for `user_id` with `ttl_secs`.
    pub fn issue(user_id: &Uuid, purpose: &str, ttl_secs: u64, cfg: &JwtConfig) -> Self {
        let now = Utc::now().timestamp() as usize;
        Self {
            sub: user_id.to_string(),
            exp: now + ttl_secs as usize,
            iat: now,
            iss: cfg.issuer.clone(),
            aud: cfg.audience.clone(),
            typ: purpose.to_string(),
            sid: String::new(),
        }
    }
}

/// HS256 JWT guard implementation.
pub struct JwtGuard {
    cfg: JwtConfig,
    verifier: Arc<dyn PasswordVerifier>,
    /// Credential lookup (fail-closed StaticLookup until DB wiring).
    lookup: Arc<dyn crate::users::UserLookup>,
    /// Revoked token pairs (per-process memory unless a shared store is set).
    revocations: Arc<dyn Store>,
}

impl JwtGuard {
    /// Create a guard from config and a password verifier.
    pub fn new(cfg: JwtConfig, verifier: Arc<dyn PasswordVerifier>) -> Self {
        Self {
            cfg,
            verifier,
            lookup: Arc::new(crate::users::StaticLookup),
            revocations: Arc::new(MemoryStore::new()),
        }
    }

    /// Record revocations in `store`, e.g. a Redis store shared by every
    /// process, so a logout in one process is honoured by all of them.
    pub fn with_revocation_store(mut self, store: Arc<dyn Store>) -> Self {
        self.revocations = store;
        self
    }

    /// Attach a credential lookup (database-backed at boot).
    pub fn with_lookup(mut self, lookup: Arc<dyn crate::users::UserLookup>) -> Self {
        self.lookup = lookup;
        self
    }

    /// Sign `claims` into a compact JWT string.
    fn sign(&self, claims: &JwtClaims) -> Result<String> {
        let key = EncodingKey::from_secret(self.cfg.secret.as_bytes())
            .map_err(|e| AuthError::Token(e.to_string()))?;
        encode(&Header::new(Algorithm::HS256), claims, &key)
            .map_err(|e| AuthError::Token(e.to_string()))
    }

    /// Decode and fully validate a token (signature, exp, iss, aud).
    fn decode_claims(&self, token: &str) -> Result<JwtClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.cfg.issuer]);
        validation.set_audience(&[&self.cfg.audience]);
        validation.leeway = 0;
        // A secret the HMAC key constructor rejects can verify no token.
        let key = DecodingKey::from_secret(self.cfg.secret.as_bytes())
            .map_err(|_| AuthError::InvalidToken)?;
        decode::<JwtClaims>(token, &key, &validation)
            .map(|d| d.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
                _ => AuthError::InvalidToken,
            })
    }

    /// Pair of signed claims for `user_id`, sharing a fresh session id.
    fn issue_pair(&self, user_id: &Uuid) -> Result<Token> {
        let sid = Uuid::new_v4().to_string();
        let access = JwtClaims {
            sid: sid.clone(),
            ..JwtClaims::issue(user_id, "access", self.cfg.access_ttl_secs, &self.cfg)
        };
        let refresh = JwtClaims {
            sid,
            ..JwtClaims::issue(user_id, "refresh", self.cfg.refresh_ttl_secs, &self.cfg)
        };
        Ok(Token::bearer(
            self.sign(&access)?,
            self.sign(&refresh)?,
            self.cfg.access_ttl_secs,
        ))
    }
}

impl Guard for JwtGuard {
    fn name(&self) -> &str {
        "jwt"
    }

    fn login<'a>(
        &'a self,
        creds: &'a Credentials,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Token>> + Send + 'a>> {
        Box::pin(async move {
            self.cfg.validate()?;
            let stored_hash = self
                .lookup
                .hash_for_email(&creds.email)
                .ok_or(AuthError::BadCredentials)?;
            // Constant-time argon2 verify; same error for unknown user + wrong
            // password so account enumeration is not possible via timing.
            if !self.verifier.verify(&stored_hash, &creds.password) {
                return Err(AuthError::BadCredentials);
            }
            let user_id = self
                .lookup
                .id_for_email(&creds.email)
                .ok_or(AuthError::BadCredentials)?;
            let parsed = Uuid::parse_str(&user_id).map_err(|_| AuthError::BadCredentials)?;
            self.issue_pair(&parsed)
        })
    }

    fn login_using_id<'a>(
        &'a self,
        user_id: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Token>> + Send + 'a>> {
        Box::pin(async move {
            self.cfg.validate()?;
            if !self.cfg.allow_login_using_id {
                return Err(AuthError::Disabled("loginUsingId is disabled".into()));
            }
            let parsed = Uuid::parse_str(user_id).map_err(|_| AuthError::InvalidToken)?;
            self.issue_pair(&parsed)
        })
    }

    fn parse<'a>(
        &'a self,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AuthUser>> + Send + 'a>> {
        Box::pin(async move {
            self.cfg.validate()?;
            let claims = self.decode_claims(token)?;
            if claims.typ != "access" {
                return Err(AuthError::InvalidToken);
            }
            revoke::ensure_active(self.revocations.as_ref(), &claims, token).await?;
            // Verification state is resolved fresh from the lookup on every
            // parse rather than baked into the signed claims: a JWT lives for
            // its whole `exp` window, so an `email_verified_at` claim would go
            // stale the moment the address is verified or `markEmailAsUnverified`
            // clears it. The lookup read keeps the `verified` gate honest at the
            // cost of one lookup per request — the same trade the `email` field
            // already makes.
            //
            // `password_confirmed_at` is deliberately left `None`: Laravel
            // stores it in the *session* (`auth.password_confirmed_at`), and a
            // stateless bearer token has no session record to carry it. A
            // password-confirmation gate over JWT therefore fails closed until
            // that state is provided out of band (a deny-list/session store,
            // tracked by the S05 milestone); see `SessionGuard::confirm_password`
            // for the stateful path.
            let email_verified_at = self.lookup.email_verified_at_for_id(&claims.sub);
            let timezone = self.lookup.timezone_for_id(&claims.sub);
            Ok(AuthUser {
                email: self.lookup.email_for_id(&claims.sub),
                id: claims.sub,
                guard: self.name().to_string(),
                email_verified_at,
                password_confirmed_at: None,
                timezone,
            })
        })
    }

    fn refresh<'a>(
        &'a self,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Token>> + Send + 'a>> {
        Box::pin(async move {
            self.cfg.validate()?;
            let claims = self.decode_claims(token)?;
            if claims.typ != "refresh" {
                return Err(AuthError::InvalidToken);
            }
            let parsed = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)?;
            // Single use: the old pair is revoked atomically before the new one
            // is issued, so a replayed refresh token is rejected.
            let refresh_ttl = self.cfg.refresh_ttl_secs;
            revoke::claim_once(self.revocations.as_ref(), &claims, token, refresh_ttl).await?;
            self.issue_pair(&parsed)
        })
    }

    fn logout<'a>(
        &'a self,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.cfg.validate()?;
            let claims = match self.decode_claims(token) {
                Ok(claims) => claims,
                // An expired token can no longer be used; nothing to revoke.
                Err(AuthError::ExpiredToken) => return Ok(()),
                Err(error) => return Err(error),
            };
            let refresh_ttl = self.cfg.refresh_ttl_secs;
            revoke::revoke(self.revocations.as_ref(), &claims, token, refresh_ttl).await
        })
    }

    fn user<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<AuthUser>>> + Send + 'a>>
    {
        Box::pin(async move {
            // The JWT guard cannot resolve a user without a bearer token; the
            // auth middleware injects `parse` results as request extensions.
            Ok(None)
        })
    }

    fn id<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<String>>> + Send + 'a>>
    {
        Box::pin(async move { Ok(None) })
    }
}

mod revoke;

#[cfg(test)]
mod tests;

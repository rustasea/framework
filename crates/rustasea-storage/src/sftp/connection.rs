//! SFTP transport: SSH handshake, host-key pinning, auth, and the SFTP
//! subsystem session.
//!
//! Built on the pure-Rust `russh` client (`client::connect` →
//! `authenticate_password`/`authenticate_publickey` →
//! `channel_open_session` → `request_subsystem("sftp")` → `SftpSession::new`).
//! No OpenSSL or system `ssh` binary is involved.

use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, AuthResult, Handle};
use russh::keys::{load_secret_key, HashAlg, PrivateKeyWithHashAlg};
use russh_sftp::client::SftpSession;

use crate::error::{Result, StorageError};
use crate::sftp::config::SftpDiskConfig;

/// SSH client handler that pins the server host key.
///
/// With an expected fingerprint, a mismatch returns `Ok(false)` so `russh`
/// aborts the handshake with [`russh::Error::UnknownKey`]; without one the key
/// is accepted (matching Laravel's default trust-on-first-use behavior).
pub struct ClientHandler {
    /// Expected `SHA256:…` fingerprint, or `None` to accept any host key.
    expected_fingerprint: Option<String>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    /// Verify the server host key against the configured pin (if any).
    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        // A host certificate is pinned by the fingerprint of the key it certifies.
        let actual = server_public_key
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
        match &self.expected_fingerprint {
            Some(expected) => Ok(actual == *expected),
            None => Ok(true),
        }
    }
}

/// A live SFTP session plus the SSH handle that keeps it alive.
pub struct SftpConnection {
    /// The SFTP protocol session used for all file operations.
    sftp: SftpSession,
    /// The owning SSH session; dropped last so the channel stays valid.
    _session: Handle<ClientHandler>,
}

impl SftpConnection {
    /// Borrow the underlying SFTP session.
    pub fn sftp(&self) -> &SftpSession {
        &self.sftp
    }
}

/// Establish an SFTP connection from `config`.
///
/// Connect, auth, host-key pinning, and subsystem startup are all bounded by
/// `config.timeout_secs()`; every failure surfaces as
/// [`StorageError::ConnectionFailed`].
///
/// # Errors
///
/// [`StorageError::Config`] when the config is invalid (blank host/username);
/// [`StorageError::ConnectionFailed`] on connect/auth/host-key/subsystem
/// failure or timeout.
pub async fn connect(config: &SftpDiskConfig) -> Result<SftpConnection> {
    config.validate()?;
    let timeout = Duration::from_secs(config.timeout_secs());

    let client_config = client::Config {
        inactivity_timeout: Some(timeout),
        ..Default::default()
    };
    let handler = ClientHandler {
        expected_fingerprint: config.host_key.clone(),
    };

    let session = tokio::time::timeout(
        timeout,
        client::connect(
            Arc::new(client_config),
            (config.host.as_str(), config.port()),
            handler,
        ),
    )
    .await
    .map_err(|_| StorageError::ConnectionFailed("connect timed out".into()))?
    .map_err(map_connect_error)?;

    authenticate(session, config).await
}

/// Authenticate the SSH session, preferring a private key over a password.
async fn authenticate(
    mut session: Handle<ClientHandler>,
    config: &SftpDiskConfig,
) -> Result<SftpConnection> {
    let result: AuthResult = match &config.private_key_path {
        Some(path) if !path.trim().is_empty() => {
            let key = load_secret_key(path, None).map_err(|e| {
                StorageError::ConnectionFailed(format!("could not load private key `{path}`: {e}"))
            })?;
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), Some(HashAlg::Sha256));
            session
                .authenticate_publickey(config.username.clone(), key)
                .await
                .map_err(|e| {
                    StorageError::ConnectionFailed(format!("public-key auth failed: {e}"))
                })?
        }
        _ => {
            let password = config.password.clone().unwrap_or_default();
            session
                .authenticate_password(config.username.clone(), password)
                .await
                .map_err(|e| StorageError::ConnectionFailed(format!("password auth failed: {e}")))?
        }
    };

    if !result.success() {
        return Err(StorageError::ConnectionFailed(
            "authentication rejected by server".into(),
        ));
    }

    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| StorageError::ConnectionFailed(format!("could not open channel: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| StorageError::ConnectionFailed(format!("sftp subsystem rejected: {e}")))?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| StorageError::ConnectionFailed(format!("sftp init failed: {e}")))?;
    sftp.set_timeout(config.timeout_secs());

    Ok(SftpConnection {
        sftp,
        _session: session,
    })
}

/// Map a `russh` connect failure, distinguishing a host-key rejection.
fn map_connect_error(error: russh::Error) -> StorageError {
    match error {
        russh::Error::UnknownKey => {
            StorageError::ConnectionFailed("host key does not match pinned fingerprint".into())
        }
        other => StorageError::ConnectionFailed(format!("ssh handshake failed: {other}")),
    }
}

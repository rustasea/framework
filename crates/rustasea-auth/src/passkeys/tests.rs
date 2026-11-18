//! `PasskeyService` ceremony tests (AUTH-017).
//!
//! Split out of [`super`] for the 500-line cap. These sign real P-256
//! assertions and build real CBOR attestation objects, so the verifier is
//! exercised end to end rather than mocked.

use super::*;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ciborium::value::Value as CborValue;
use p256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// Relying-party id used by the ceremony fixtures.
const RP_ID: &str = "localhost";
/// Origin used by the ceremony fixtures.
const ORIGIN: &str = "http://localhost";
/// Credential id used by the ceremony fixtures.
const CRED_ID: &str = "cred-1";

/// A fixed P-256 signing key: the ceremonies need a valid key, not a random one.
fn signing_key() -> SigningKey {
    SigningKey::from_slice(&[7u8; 32]).expect("valid P-256 scalar")
}

/// Resolved relying-party settings for the fixtures.
fn resolved() -> ResolvedPasskeys {
    ResolvedPasskeys {
        relying_party_id: RP_ID.to_string(),
        allowed_origins: vec![ORIGIN.to_string()],
        user_handle_secret: "test-secret".to_string(),
        timeout: 60_000,
    }
}

/// Build a service over fresh in-memory stores.
fn service() -> (
    PasskeyService,
    Arc<MemoryPasskeyStore>,
    Arc<MemoryChallengeStore>,
) {
    let store = Arc::new(MemoryPasskeyStore::default());
    let challenges = Arc::new(MemoryChallengeStore::default());
    let service = PasskeyService::new(store.clone(), challenges.clone(), resolved(), "RustaSea");
    (service, store, challenges)
}

/// SHA-256 digest of `bytes`.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Base64url-encode `bytes`.
fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Encode a CBOR value to bytes.
fn cbor(value: &CborValue) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).expect("encode CBOR");
    out
}

/// Build a COSE EC2/P-256 public key from a SEC1 point.
fn cose_key(sec1: &[u8]) -> CborValue {
    let int = |value: i128| CborValue::Integer(value.try_into().expect("cbor integer"));
    CborValue::Map(vec![
        (int(1), int(2)),
        (int(3), int(-7)),
        (int(-1), int(1)),
        (int(-2), CborValue::Bytes(sec1[1..33].to_vec())),
        (int(-3), CborValue::Bytes(sec1[33..65].to_vec())),
    ])
}

/// Build a `none`-attestation registration response for `challenge`.
fn registration_response(
    signing: &SigningKey,
    challenge: &str,
    counter: u32,
) -> RegistrationResponse {
    let sec1 = VerifyingKey::from(signing)
        .to_sec1_point(false)
        .as_bytes()
        .to_vec();
    let client_data =
        format!(r#"{{"type":"webauthn.create","challenge":"{challenge}","origin":"{ORIGIN}"}}"#);
    let mut auth_data = Vec::new();
    auth_data.extend_from_slice(&sha256(RP_ID.as_bytes()));
    auth_data.push(0x01 | 0x40);
    auth_data.extend_from_slice(&counter.to_be_bytes());
    auth_data.extend_from_slice(&[0u8; 16]);
    auth_data.extend_from_slice(&(CRED_ID.len() as u16).to_be_bytes());
    auth_data.extend_from_slice(CRED_ID.as_bytes());
    auth_data.extend_from_slice(&cbor(&cose_key(&sec1)));
    let attestation = CborValue::Map(vec![
        (
            CborValue::Text("fmt".into()),
            CborValue::Text("none".into()),
        ),
        (CborValue::Text("attStmt".into()), CborValue::Map(vec![])),
        (
            CborValue::Text("authData".into()),
            CborValue::Bytes(auth_data),
        ),
    ]);
    RegistrationResponse {
        id: encode(CRED_ID.as_bytes()),
        raw_id: None,
        name: Some("Test key".to_string()),
        response: webauthn::RegistrationResponseInner {
            client_data_json: encode(client_data.as_bytes()),
            attestation_object: encode(&cbor(&attestation)),
        },
    }
}

/// Build an assertion response for `challenge`.
fn authentication_response(
    signing: &SigningKey,
    challenge: &str,
    counter: u32,
    user_handle: Option<String>,
) -> AuthenticationResponse {
    let client_data =
        format!(r#"{{"type":"webauthn.get","challenge":"{challenge}","origin":"{ORIGIN}"}}"#);
    let mut auth_data = Vec::new();
    auth_data.extend_from_slice(&sha256(RP_ID.as_bytes()));
    auth_data.push(0x01);
    auth_data.extend_from_slice(&counter.to_be_bytes());
    let mut message = auth_data.clone();
    message.extend_from_slice(&sha256(client_data.as_bytes()));
    let signature: Signature = signing.sign(&message);
    AuthenticationResponse {
        id: encode(CRED_ID.as_bytes()),
        raw_id: None,
        response: webauthn::AuthenticationResponseInner {
            client_data_json: encode(client_data.as_bytes()),
            authenticator_data: encode(&auth_data),
            signature: encode(signature.to_der().as_bytes()),
            user_handle,
        },
    }
}

/// Register `user-1` with a fresh key, returning the key + credential.
async fn register(service: &PasskeyService, signing: &SigningKey) -> PasskeyCredential {
    let options = service
        .begin_registration("user-1", "ada@example.com", "Ada", "register")
        .await
        .expect("begin");
    let response = registration_response(signing, &options.challenge, 0);
    service
        .finish_registration("user-1", "register", &response, "Laptop")
        .await
        .expect("finish")
}

/// A full registration then assertion round-trip succeeds.
#[tokio::test]
async fn registration_then_assertion_round_trip() {
    let (service, store, _challenges) = service();
    let signing = signing_key();
    let credential = register(&service, &signing).await;
    assert_eq!(store.list("user-1").await.expect("list").len(), 1);

    let options = service
        .begin_authentication("login", std::slice::from_ref(&credential))
        .await
        .expect("begin auth");
    let handle = user_handle("test-secret", "user-1").expect("handle");
    let response = authentication_response(&signing, &options.challenge, 1, Some(handle));
    let authenticated = service
        .finish_authentication("login", &response)
        .await
        .expect("finish auth");
    assert_eq!(authenticated.user_id, "user-1");
    assert_eq!(
        store.get_sync(&authenticated.id).expect("stored").counter,
        1
    );
}

/// A challenge is consumed by the first finish and rejected on replay.
#[tokio::test]
async fn challenge_is_single_use() {
    let (service, _store, _challenges) = service();
    let signing = signing_key();
    let options = service
        .begin_registration("user-1", "ada@example.com", "Ada", "register")
        .await
        .expect("begin");
    let response = registration_response(&signing, &options.challenge, 0);
    service
        .finish_registration("user-1", "register", &response, "Laptop")
        .await
        .expect("first finish");
    assert!(service
        .finish_registration("user-1", "register", &response, "Laptop")
        .await
        .is_err());
}

/// A signature-counter regression is rejected; an increase is accepted.
#[tokio::test]
async fn counter_regression_is_rejected() {
    let (service, _store, _challenges) = service();
    let signing = signing_key();
    let credential = register(&service, &signing).await;

    let options = service
        .begin_authentication("a", std::slice::from_ref(&credential))
        .await
        .expect("begin");
    let advance = authentication_response(&signing, &options.challenge, 5, None);
    service
        .finish_authentication("a", &advance)
        .await
        .expect("advance");

    let options = service
        .begin_authentication("b", std::slice::from_ref(&credential))
        .await
        .expect("begin");
    let replay = authentication_response(&signing, &options.challenge, 3, None);
    assert_eq!(
        service.finish_authentication("b", &replay).await,
        Err(AuthError::InvalidPasskeyAssertion)
    );
}

/// An unknown credential is rejected with `PasskeyRequired`.
#[tokio::test]
async fn unknown_credential_is_rejected() {
    let (service, _store, _challenges) = service();
    let signing = signing_key();
    let _ = register(&service, &signing).await;
    let options = service.begin_authentication("a", &[]).await.expect("begin");
    let response = authentication_response(&signing, &options.challenge, 1, None);
    // Rewrite the credential id to one that was never registered.
    let mut unknown = response;
    unknown.id = encode(b"other");
    assert_eq!(
        service.finish_authentication("a", &unknown).await,
        Err(AuthError::PasskeyRequired)
    );
}

/// A mismatched origin is rejected during verification.
#[tokio::test]
async fn wrong_origin_is_rejected() {
    let (service, _store, _challenges) = service();
    let signing = signing_key();
    let options = service
        .begin_registration("user-1", "ada@example.com", "Ada", "register")
        .await
        .expect("begin");
    let mut response = registration_response(&signing, &options.challenge, 0);
    // Swap the origin inside the (base64url) clientDataJSON.
    let client_data = format!(
        r#"{{"type":"webauthn.create","challenge":"{}","origin":"https://evil.example"}}"#,
        options.challenge
    );
    response.response.client_data_json = encode(client_data.as_bytes());
    assert!(service
        .finish_registration("user-1", "register", &response, "Laptop")
        .await
        .is_err());
}

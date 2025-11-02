use std::{
    collections::{HashMap, HashSet},
    sync::RwLock,
};

use anyhow::{self, Context};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use openguild_crypto::{
    generate_signing_key, sign_message, signing_key_from_base64, verifying_key_from, SigningKey,
};
use openguild_storage::{MlsKeyPackageStore, NewMlsKeyPackage};
use rand::{rng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{session, AppState};
use axum::{
    extract::{MatchedPath, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use tracing::error;

#[derive(Debug, Error)]
pub enum MlsError {
    #[error("identity '{0}' is not managed by this server")]
    UnknownIdentity(String),
    #[error("failed to persist MLS key package: {0}")]
    Persistence(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyPackage {
    pub identity: String,
    pub ciphersuite: String,
    pub signature_key: String,
    pub hpke_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeTestVector {
    pub identity: String,
    pub ciphersuite: String,
    pub message: String,
    pub signature: String,
    pub verifying_key: String,
}

struct StoredKeyPackage {
    identity: String,
    ciphersuite: String,
    signing: SigningKey,
    hpke_public: [u8; 32],
}

impl StoredKeyPackage {
    fn generate(identity: &str, ciphersuite: &str) -> Self {
        let signing = generate_signing_key();
        let mut hpke_public = [0u8; 32];
        rng().fill_bytes(&mut hpke_public);
        Self {
            identity: identity.to_owned(),
            ciphersuite: ciphersuite.to_owned(),
            signing,
            hpke_public,
        }
    }

    fn from_persisted(record: &openguild_storage::MlsKeyPackageRecord) -> anyhow::Result<Self> {
        let signing = signing_key_from_base64(&record.signing_key)
            .map_err(|err| anyhow::anyhow!(err))
            .context("decoding persisted MLS signing key")?;
        let decoded = URL_SAFE_NO_PAD
            .decode(record.hpke_public_key.as_bytes())
            .context("decoding persisted MLS HPKE public key")?;
        let hpke_public: [u8; 32] = decoded
            .try_into()
            .map_err(|_| anyhow::anyhow!("hpke public key must be 32 bytes"))?;

        Ok(Self {
            identity: record.identity.clone(),
            ciphersuite: record.ciphersuite.clone(),
            signing,
            hpke_public,
        })
    }

    fn package(&self) -> PublicKeyPackage {
        let verifying = verifying_key_from(&self.signing);
        PublicKeyPackage {
            identity: self.identity.clone(),
            ciphersuite: self.ciphersuite.clone(),
            signature_key: URL_SAFE_NO_PAD.encode(verifying.to_bytes()),
            hpke_public_key: URL_SAFE_NO_PAD.encode(self.hpke_public),
        }
    }

    fn into_new_package(&self) -> NewMlsKeyPackage {
        let signing_key = URL_SAFE_NO_PAD.encode(self.signing.to_bytes());
        let verifying_key = URL_SAFE_NO_PAD.encode(verifying_key_from(&self.signing).to_bytes());
        let hpke_public_key = URL_SAFE_NO_PAD.encode(self.hpke_public);
        NewMlsKeyPackage {
            identity: self.identity.clone(),
            ciphersuite: self.ciphersuite.clone(),
            signing_key,
            signature_key: verifying_key,
            hpke_public_key,
        }
    }
}

pub struct MlsKeyStore {
    ciphersuite: String,
    packages: RwLock<HashMap<String, StoredKeyPackage>>,
    persistence: Option<MlsKeyPackageStore>,
}

impl MlsKeyStore {
    pub fn new(ciphersuite: impl Into<String>, identities: Vec<String>) -> Self {
        let ciphersuite = ciphersuite.into();
        let packages = identities
            .into_iter()
            .map(|identity| {
                (
                    identity.clone(),
                    StoredKeyPackage::generate(&identity, &ciphersuite),
                )
            })
            .collect();
        Self::from_parts(ciphersuite, packages, None)
    }

    pub async fn with_persistence(
        ciphersuite: impl Into<String>,
        identities: Vec<String>,
        store: MlsKeyPackageStore,
    ) -> anyhow::Result<Self> {
        let ciphersuite = ciphersuite.into();
        let mut packages = HashMap::new();
        let mut remaining: HashSet<String> = identities.iter().cloned().collect();

        let persisted = store.latest_packages().await?;
        for record in persisted {
            if !remaining.contains(&record.identity) {
                continue;
            }
            let stored = StoredKeyPackage::from_persisted(&record)
                .with_context(|| format!("reconstructing MLS package for '{}'", record.identity))?;
            remaining.remove(&record.identity);
            packages.insert(record.identity.clone(), stored);
        }

        for identity in remaining {
            let package = StoredKeyPackage::generate(&identity, &ciphersuite);
            store
                .record_package(&package.into_new_package())
                .await
                .with_context(|| format!("persisting MLS package for '{identity}'"))?;
            packages.insert(identity.clone(), package);
        }

        Ok(Self::from_parts(ciphersuite, packages, Some(store)))
    }

    fn from_parts(
        ciphersuite: String,
        packages: HashMap<String, StoredKeyPackage>,
        persistence: Option<MlsKeyPackageStore>,
    ) -> Self {
        Self {
            ciphersuite,
            packages: RwLock::new(packages),
            persistence,
        }
    }

    pub fn list_packages(&self) -> Vec<PublicKeyPackage> {
        let packages = self.packages.read().expect("packages lock poisoned");
        let mut result: Vec<_> = packages.values().map(|pkg| pkg.package()).collect();
        result.sort_by(|a, b| a.identity.cmp(&b.identity));
        result
    }

    pub fn handshake_vectors(&self) -> Vec<HandshakeTestVector> {
        const MESSAGE: &str = "OpenGuild MLS handshake test vector v1";
        let packages = self.packages.read().expect("packages lock poisoned");
        let mut vectors: Vec<_> = packages
            .values()
            .map(|pkg| {
                let signature = sign_message(&pkg.signing, MESSAGE.as_bytes());
                HandshakeTestVector {
                    identity: pkg.identity.clone(),
                    ciphersuite: pkg.ciphersuite.clone(),
                    message: MESSAGE.to_string(),
                    signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
                    verifying_key: URL_SAFE_NO_PAD
                        .encode(verifying_key_from(&pkg.signing).to_bytes()),
                }
            })
            .collect();
        vectors.sort_by(|a, b| a.identity.cmp(&b.identity));
        vectors
    }

    pub async fn rotate(&self, identity: &str) -> Result<PublicKeyPackage, MlsError> {
        if !self
            .packages
            .read()
            .expect("packages lock poisoned")
            .contains_key(identity)
        {
            return Err(MlsError::UnknownIdentity(identity.to_owned()));
        }

        let new_package = StoredKeyPackage::generate(identity, &self.ciphersuite);
        if let Some(store) = &self.persistence {
            store
                .record_package(&new_package.into_new_package())
                .await
                .map_err(|err| MlsError::Persistence(err.to_string()))?;
        }

        let public = new_package.package();
        let mut packages = self.packages.write().expect("packages lock poisoned");
        packages.insert(identity.to_owned(), new_package);
        Ok(public)
    }
}

pub async fn list_key_packages(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PublicKeyPackage>>, StatusCode> {
    let Some(mls) = state.mls() else {
        let status = StatusCode::NOT_IMPLEMENTED;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let packages = mls.list_packages();
    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());

    Ok(Json(packages))
}

pub async fn rotate_key_package(
    matched_path: MatchedPath,
    Path(identity): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PublicKeyPackage>, StatusCode> {
    let Some(mls) = state.mls() else {
        let status = StatusCode::NOT_IMPLEMENTED;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    match mls.rotate(&identity).await {
        Ok(package) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());
            Ok(Json(package))
        }
        Err(MlsError::UnknownIdentity(_)) => {
            #[cfg(feature = "metrics")]
            state.record_http_request(matched_path.as_str(), StatusCode::NOT_FOUND.as_u16());
            Err(StatusCode::NOT_FOUND)
        }
        Err(MlsError::Persistence(err)) => {
            error!(identity, %err, "failed to persist MLS key package rotation");
            #[cfg(feature = "metrics")]
            state.record_http_request(
                matched_path.as_str(),
                StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn handshake_test_vectors(
    matched_path: MatchedPath,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<HandshakeTestVector>>, StatusCode> {
    let Some(mls) = state.mls() else {
        let status = StatusCode::NOT_IMPLEMENTED;
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    };

    if let Err(status) = session::authenticate_bearer(&state, &headers) {
        #[cfg(feature = "metrics")]
        state.record_http_request(matched_path.as_str(), status.as_u16());
        return Err(status);
    }

    #[cfg(not(feature = "metrics"))]
    let _ = matched_path;

    let vectors = mls.handshake_vectors();
    #[cfg(feature = "metrics")]
    state.record_http_request(matched_path.as_str(), StatusCode::OK.as_u16());

    Ok(Json(vectors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openguild_crypto::{verifying_key_from_base64, Signature};
    use std::convert::TryInto;

    #[tokio::test]
    async fn generates_and_rotates_packages() {
        let store = MlsKeyStore::new(
            "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519",
            vec!["alice".into(), "bob".into()],
        );

        let packages = store.list_packages();
        assert_eq!(packages.len(), 2);

        let alice_before = packages
            .iter()
            .find(|pkg| pkg.identity == "alice")
            .unwrap()
            .signature_key
            .clone();

        let rotated = store.rotate("alice").await.expect("rotation succeeds");
        assert_eq!(rotated.identity, "alice");
        assert_ne!(rotated.signature_key, alice_before);
        assert!(store.rotate("carol").await.is_err());
    }

    #[test]
    fn handshake_vectors_sign_constant_message() {
        let store = MlsKeyStore::new(
            "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519",
            vec!["alice".into()],
        );

        let vectors = store.handshake_vectors();
        assert_eq!(vectors.len(), 1);
        let vector = &vectors[0];
        assert_eq!(vector.identity, "alice");
        assert_eq!(
            vector.message,
            "OpenGuild MLS handshake test vector v1".to_string()
        );

        let verifying =
            verifying_key_from_base64(&vector.verifying_key).expect("verifying key decodes");
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(vector.signature.as_bytes())
            .expect("signature decodes");
        let signature_array: [u8; 64] = signature_bytes.try_into().expect("signature size");
        let signature = Signature::from_bytes(&signature_array);

        verifying
            .verify_strict(vector.message.as_bytes(), &signature)
            .expect("signature verifies");
    }
}

use std::{collections::HashMap, sync::RwLock};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use openguild_crypto::{generate_signing_key, verifying_key_from, SigningKey};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{session, AppState};
use axum::{
    extract::{MatchedPath, State},
    http::{HeaderMap, StatusCode},
    Json,
};

#[derive(Debug, Error)]
pub enum MlsError {
    #[error("identity '{0}' is not managed by this server")]
    UnknownIdentity(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyPackage {
    pub identity: String,
    pub ciphersuite: String,
    pub signature_key: String,
    pub hpke_public_key: String,
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
        OsRng.fill_bytes(&mut hpke_public);
        Self {
            identity: identity.to_owned(),
            ciphersuite: ciphersuite.to_owned(),
            signing,
            hpke_public,
        }
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
}

pub struct MlsKeyStore {
    ciphersuite: String,
    packages: RwLock<HashMap<String, StoredKeyPackage>>,
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
        Self {
            ciphersuite,
            packages: RwLock::new(packages),
        }
    }

    pub fn list_packages(&self) -> Vec<PublicKeyPackage> {
        let packages = self.packages.read().expect("packages lock poisoned");
        let mut result: Vec<_> = packages.values().map(|pkg| pkg.package()).collect();
        result.sort_by(|a, b| a.identity.cmp(&b.identity));
        result
    }

    pub fn rotate(&self, identity: &str) -> Result<PublicKeyPackage, MlsError> {
        let mut packages = self.packages.write().expect("packages lock poisoned");
        match packages.get_mut(identity) {
            Some(entry) => {
                *entry = StoredKeyPackage::generate(identity, &self.ciphersuite);
                Ok(entry.package())
            }
            None => Err(MlsError::UnknownIdentity(identity.to_owned())),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_and_rotates_packages() {
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

        let rotated = store.rotate("alice").expect("rotation succeeds");
        assert_eq!(rotated.identity, "alice");
        assert_ne!(rotated.signature_key, alice_before);
        assert!(store.rotate("carol").is_err());
    }
}

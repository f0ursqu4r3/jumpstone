//! Media storage adapters.

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct MediaConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

pub fn validate_config(cfg: &MediaConfig) -> Result<()> {
    if cfg.endpoint.is_empty() || cfg.bucket.is_empty() {
        anyhow::bail!("media configuration is incomplete");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_config_accepts_complete_settings() {
        let cfg = MediaConfig {
            endpoint: "https://minio.local".into(),
            bucket: "uploads".into(),
            access_key: "minio".into(),
            secret_key: "secret".into(),
        };

        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn validate_config_rejects_missing_fields() {
        let cfg = MediaConfig {
            endpoint: "".into(),
            bucket: "".into(),
            access_key: "minio".into(),
            secret_key: "secret".into(),
        };

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(
            err.to_string().contains("incomplete"),
            "expected error to mention incomplete configuration, got: {err:?}"
        );
    }
}

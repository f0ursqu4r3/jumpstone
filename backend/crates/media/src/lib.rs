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

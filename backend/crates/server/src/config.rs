use std::{net::SocketAddr, str::FromStr};

use openguild_crypto::{signing_key_from_base64, verifying_key_from_base64};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to build configuration: {0}")]
    Build(#[from] config::ConfigError),
    #[error("invalid bind address: {0}")]
    InvalidBindAddr(String),
    #[error("invalid session key material: {0}")]
    InvalidSessionKey(String),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum LogFormat {
    Compact,
    Json,
}

impl Default for LogFormat {
    fn default() -> Self {
        LogFormat::Compact
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub bind_addr: Option<String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_addr: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct SessionConfig {
    /// Base64-encoded ed25519 signing key (32 bytes) used for session token signing.
    #[serde(alias = "signing_key", rename = "signing_key")]
    pub active_signing_key: Option<String>,
    /// Optional fallback verifying keys (base64) that remain valid during key rotation.
    pub fallback_verifying_keys: Vec<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            active_signing_key: None,
            fallback_verifying_keys: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub bind_addr: Option<String>,
    pub host: String,
    pub server_name: String,
    pub port: u16,
    pub log_format: LogFormat,
    pub metrics: MetricsConfig,
    pub database_url: Option<String>,
    pub session: SessionConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: None,
            host: "0.0.0.0".to_string(),
            server_name: "localhost".to_string(),
            port: 8080,
            log_format: LogFormat::Compact,
            metrics: MetricsConfig::default(),
            database_url: None,
            session: SessionConfig::default(),
        }
    }
}

impl ServerConfig {
    const ENV_PREFIX: &'static str = "OPENGUILD_SERVER";

    pub fn load() -> Result<Self, ConfigError> {
        let defaults = ServerConfig::default();

        let builder = config::Config::builder()
            .add_source(config::File::with_name("config/server").required(false))
            .add_source(config::File::with_name("config/server.local").required(false))
            .add_source(
                config::Environment::with_prefix(Self::ENV_PREFIX)
                    .separator("__")
                    .try_parsing(true),
            )
            .set_default("host", defaults.host.clone())?
            .set_default("server_name", defaults.server_name.clone())?
            .set_default("port", defaults.port as i64)?
            .set_default("log_format", defaults.log_format.as_str())?
            .set_default("metrics.enabled", defaults.metrics.enabled)?;

        let settings: ServerConfig = builder.build()?.try_deserialize()?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn listener_addr(&self) -> Result<SocketAddr, ConfigError> {
        if let Some(addr) = &self.bind_addr {
            return addr
                .parse()
                .map_err(|_| ConfigError::InvalidBindAddr(addr.clone()));
        }

        let addr = format!("{}:{}", self.host, self.port);
        addr.parse().map_err(|_| ConfigError::InvalidBindAddr(addr))
    }

    pub fn log_format(&self) -> LogFormat {
        self.log_format.clone()
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::InvalidBindAddr("port cannot be zero".into()));
        }
        if let Some(addr) = &self.metrics.bind_addr {
            addr.parse::<SocketAddr>()
                .map_err(|_| ConfigError::InvalidBindAddr(addr.clone()))?;
        }
        if let Some(active) = &self.session.active_signing_key {
            signing_key_from_base64(active)
                .map_err(|err| ConfigError::InvalidSessionKey(err.to_string()))?;
        }
        for key in &self.session.fallback_verifying_keys {
            verifying_key_from_base64(key)
                .map_err(|err| ConfigError::InvalidSessionKey(err.to_string()))?;
        }
        Ok(())
    }

    pub fn apply_overrides(&mut self, overrides: &CliOverrides) -> Result<(), ConfigError> {
        if let Some(bind_addr) = &overrides.bind_addr {
            self.bind_addr = Some(bind_addr.clone());
        }

        if let Some(host) = &overrides.host {
            self.host = host.clone();
        }

        if let Some(server_name) = &overrides.server_name {
            self.server_name = server_name.clone();
        }

        if let Some(port) = overrides.port {
            self.port = port;
        }

        if let Some(log_format) = overrides.log_format {
            self.log_format = log_format;
        }

        if let Some(metrics_enabled) = overrides.metrics_enabled {
            self.metrics.enabled = metrics_enabled;
        }

        if let Some(metrics_bind_addr) = &overrides.metrics_bind_addr {
            self.metrics.bind_addr = Some(metrics_bind_addr.clone());
        }

        if let Some(database_url) = &overrides.database_url {
            self.database_url = Some(database_url.clone());
        }

        if let Some(session_signing_key) = &overrides.session_signing_key {
            self.session.active_signing_key = Some(session_signing_key.clone());
        }

        if let Some(fallbacks) = &overrides.session_fallback_verifying_keys {
            self.session.fallback_verifying_keys = fallbacks.clone();
        }

        self.validate()
    }
}

#[derive(Debug, Default, Clone)]
pub struct CliOverrides {
    pub bind_addr: Option<String>,
    pub host: Option<String>,
    pub server_name: Option<String>,
    pub port: Option<u16>,
    pub log_format: Option<LogFormat>,
    pub metrics_enabled: Option<bool>,
    pub metrics_bind_addr: Option<String>,
    pub database_url: Option<String>,
    pub session_signing_key: Option<String>,
    pub session_fallback_verifying_keys: Option<Vec<String>>,
}

impl LogFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogFormat::Compact => "compact",
            LogFormat::Json => "json",
        }
    }
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Ok(LogFormat::Json),
            "compact" => Ok(LogFormat::Compact),
            other => Err(format!("unsupported log format '{other}'")),
        }
    }
}

impl<'de> Deserialize<'de> for LogFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        LogFormat::from_str(&value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    use openguild_crypto::{generate_signing_key, verifying_key_from};

    #[test]
    fn defaults_match_expectations() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.log_format, LogFormat::Compact);
        assert!(!config.metrics.enabled);
        assert!(config.database_url.is_none());
        assert!(config.session.active_signing_key.is_none());
        assert!(config.session.fallback_verifying_keys.is_empty());
    }

    #[test]
    #[serial]
    fn environment_overrides_take_effect() {
        env::set_var("OPENGUILD_SERVER__HOST", "127.0.0.1");
        env::set_var("OPENGUILD_SERVER__PORT", "9090");
        env::set_var("OPENGUILD_SERVER__LOG_FORMAT", "json");

        let config = ServerConfig::load().expect("config loads");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);
        assert_eq!(config.log_format, LogFormat::Json);

        env::remove_var("OPENGUILD_SERVER__HOST");
        env::remove_var("OPENGUILD_SERVER__PORT");
        env::remove_var("OPENGUILD_SERVER__LOG_FORMAT");
    }

    #[test]
    #[serial]
    fn listener_addr_prefers_bind_addr() {
        env::set_var("OPENGUILD_SERVER__BIND_ADDR", "192.168.1.20:5555");

        let config = ServerConfig::load().expect("config loads");
        let addr = config.listener_addr().expect("valid addr");
        assert_eq!(addr.to_string(), "192.168.1.20:5555");

        env::remove_var("OPENGUILD_SERVER__BIND_ADDR");
    }

    #[test]
    fn listener_addr_composes_host_and_port() {
        let config = ServerConfig {
            host: "10.0.0.2".into(),
            port: 7000,
            ..ServerConfig::default()
        };

        let addr = config.listener_addr().expect("valid addr");
        assert_eq!(addr.to_string(), "10.0.0.2:7000");
    }

    #[test]
    #[serial]
    fn invalid_bind_addr_returns_error() {
        env::set_var("OPENGUILD_SERVER__BIND_ADDR", "::invalid::");

        let config = ServerConfig::load().expect("config loads");
        let err = config.listener_addr().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidBindAddr(_)));

        env::remove_var("OPENGUILD_SERVER__BIND_ADDR");
    }

    #[test]
    fn apply_overrides_updates_fields() {
        let mut cfg = ServerConfig::default();
        let signing_key = generate_signing_key();
        let signing_base64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signing_key.to_bytes());
        let verifying_key = verifying_key_from(&signing_key);
        let verifying_base64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifying_key.to_bytes());
        let overrides = CliOverrides {
            bind_addr: Some("127.0.0.1:9999".into()),
            host: Some("127.0.0.1".into()),
            server_name: Some("api.openguild.local".into()),
            port: Some(9999),
            log_format: Some(LogFormat::Json),
            metrics_enabled: Some(true),
            metrics_bind_addr: Some("127.0.0.1:9100".into()),
            database_url: Some("postgres://app:secret@localhost/db".into()),
            session_signing_key: Some(signing_base64.clone()),
            session_fallback_verifying_keys: Some(vec![verifying_base64.clone()]),
        };

        cfg.apply_overrides(&overrides).expect("overrides apply");
        assert_eq!(cfg.bind_addr.as_deref(), Some("127.0.0.1:9999"));
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.server_name, "api.openguild.local");
        assert_eq!(cfg.port, 9999);
        assert_eq!(cfg.log_format, LogFormat::Json);
        assert!(cfg.metrics.enabled);
        assert_eq!(cfg.metrics.bind_addr.as_deref(), Some("127.0.0.1:9100"));
        assert_eq!(
            cfg.database_url.as_deref(),
            Some("postgres://app:secret@localhost/db")
        );
        assert_eq!(
            cfg.session.active_signing_key.as_deref(),
            Some(signing_base64.as_str())
        );
        assert_eq!(cfg.session.fallback_verifying_keys, vec![verifying_base64]);
    }

    #[test]
    fn override_invalid_metrics_addr_errors() {
        let mut cfg = ServerConfig::default();
        let overrides = CliOverrides {
            metrics_bind_addr: Some("::bad::".into()),
            ..CliOverrides::default()
        };

        let err = cfg.apply_overrides(&overrides).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidBindAddr(_)));
    }
}

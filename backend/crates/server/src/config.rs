use std::{net::SocketAddr, str::FromStr};

#[cfg(test)]
use base64::Engine;
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
    #[error("invalid messaging configuration: {0}")]
    InvalidMessagingConfig(String),
    #[error("invalid federation configuration: {0}")]
    InvalidFederationConfig(String),
    #[error("invalid MLS configuration: {0}")]
    InvalidMlsConfig(String),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
pub enum LogFormat {
    #[default]
    Compact,
    Json,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub bind_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct MessagingConfig {
    pub max_messages_per_user_per_window: usize,
    pub max_messages_per_ip_per_window: usize,
    pub rate_limit_window_secs: u64,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            max_messages_per_user_per_window: 60,
            max_messages_per_ip_per_window: 200,
            rate_limit_window_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct FederationConfig {
    pub trusted_servers: Vec<FederatedServerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FederatedServerConfig {
    pub server_name: String,
    pub key_id: String,
    pub verifying_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct SessionConfig {
    /// Base64-encoded ed25519 signing key (32 bytes) used for session token signing.
    #[serde(alias = "signing_key", rename = "signing_key")]
    pub active_signing_key: Option<String>,
    /// Optional fallback verifying keys (base64) that remain valid during key rotation.
    pub fallback_verifying_keys: Vec<String>,
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
    pub messaging: MessagingConfig,
    pub federation: FederationConfig,
    pub mls: MlsConfig,
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
            messaging: MessagingConfig::default(),
            federation: FederationConfig::default(),
            mls: MlsConfig::default(),
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
            .set_default("metrics.enabled", defaults.metrics.enabled)?
            .set_default(
                "messaging.max_messages_per_user_per_window",
                defaults.messaging.max_messages_per_user_per_window as i64,
            )?
            .set_default(
                "messaging.max_messages_per_ip_per_window",
                defaults.messaging.max_messages_per_ip_per_window as i64,
            )?
            .set_default(
                "messaging.rate_limit_window_secs",
                defaults.messaging.rate_limit_window_secs as i64,
            )?
            .set_default("mls.enabled", defaults.mls.enabled)?
            .set_default("mls.ciphersuite", defaults.mls.ciphersuite.clone())?;

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
        self.log_format
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
        if self.messaging.max_messages_per_user_per_window == 0 {
            return Err(ConfigError::InvalidMessagingConfig(
                "messaging.max_messages_per_user_per_window must be greater than zero".into(),
            ));
        }
        if self.messaging.max_messages_per_ip_per_window == 0 {
            return Err(ConfigError::InvalidMessagingConfig(
                "messaging.max_messages_per_ip_per_window must be greater than zero".into(),
            ));
        }
        if self.messaging.rate_limit_window_secs == 0 {
            return Err(ConfigError::InvalidMessagingConfig(
                "messaging.rate_limit_window_secs must be greater than zero".into(),
            ));
        }
        for key in &self.session.fallback_verifying_keys {
            verifying_key_from_base64(key)
                .map_err(|err| ConfigError::InvalidSessionKey(err.to_string()))?;
        }
        if self.mls.enabled {
            if self.mls.ciphersuite.trim().is_empty() {
                return Err(ConfigError::InvalidMlsConfig(
                    "mls.ciphersuite must be provided when MLS is enabled".into(),
                ));
            }
            if self.mls.identities.is_empty() {
                return Err(ConfigError::InvalidMlsConfig(
                    "mls.identities must include at least one identity when MLS is enabled".into(),
                ));
            }
        }
        for peer in &self.federation.trusted_servers {
            if peer.server_name.trim().is_empty() {
                return Err(ConfigError::InvalidFederationConfig(
                    "trusted server entries require a server_name".into(),
                ));
            }
            if peer.key_id.trim().is_empty() {
                return Err(ConfigError::InvalidFederationConfig(
                    "trusted server entries require a key_id".into(),
                ));
            }
            verifying_key_from_base64(&peer.verifying_key).map_err(|err| {
                ConfigError::InvalidFederationConfig(format!(
                    "invalid verifying key for '{}': {}",
                    peer.server_name, err
                ))
            })?;
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

        if let Some(limit) = overrides.max_messages_per_user_per_window {
            self.messaging.max_messages_per_user_per_window = limit;
        }

        if let Some(limit) = overrides.max_messages_per_ip_per_window {
            self.messaging.max_messages_per_ip_per_window = limit;
        }

        if let Some(window) = overrides.rate_limit_window_secs {
            self.messaging.rate_limit_window_secs = window;
        }

        self.validate()
    }

    pub fn environment_override_keys() -> Vec<String> {
        let mut keys: Vec<String> = std::env::vars()
            .filter_map(|(key, _)| {
                if key.starts_with(Self::ENV_PREFIX) {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();
        keys.sort();
        keys
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
    pub max_messages_per_user_per_window: Option<usize>,
    pub max_messages_per_ip_per_window: Option<usize>,
    pub rate_limit_window_secs: Option<u64>,
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
        assert_eq!(config.messaging.max_messages_per_user_per_window, 60);
        assert_eq!(config.messaging.max_messages_per_ip_per_window, 200);
        assert_eq!(config.messaging.rate_limit_window_secs, 60);
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
        env::set_var(
            "OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_USER_PER_WINDOW",
            "120",
        );
        env::set_var(
            "OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_IP_PER_WINDOW",
            "400",
        );
        env::set_var("OPENGUILD_SERVER__MESSAGING__RATE_LIMIT_WINDOW_SECS", "120");

        let config = ServerConfig::load().expect("config loads");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);
        assert_eq!(config.log_format, LogFormat::Json);
        assert_eq!(config.messaging.max_messages_per_user_per_window, 120);
        assert_eq!(config.messaging.max_messages_per_ip_per_window, 400);
        assert_eq!(config.messaging.rate_limit_window_secs, 120);

        env::remove_var("OPENGUILD_SERVER__HOST");
        env::remove_var("OPENGUILD_SERVER__PORT");
        env::remove_var("OPENGUILD_SERVER__LOG_FORMAT");
        env::remove_var("OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_USER_PER_WINDOW");
        env::remove_var("OPENGUILD_SERVER__MESSAGING__MAX_MESSAGES_PER_IP_PER_WINDOW");
        env::remove_var("OPENGUILD_SERVER__MESSAGING__RATE_LIMIT_WINDOW_SECS");
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
            max_messages_per_user_per_window: Some(42),
            max_messages_per_ip_per_window: Some(84),
            rate_limit_window_secs: Some(30),
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
        assert_eq!(cfg.messaging.max_messages_per_user_per_window, 42);
        assert_eq!(cfg.messaging.max_messages_per_ip_per_window, 84);
        assert_eq!(cfg.messaging.rate_limit_window_secs, 30);
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

    #[test]
    fn federation_entries_require_valid_data() {
        let mut cfg = ServerConfig::default();
        cfg.federation.trusted_servers.push(FederatedServerConfig {
            server_name: "".into(),
            key_id: "".into(),
            verifying_key: "invalid".into(),
        });
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFederationConfig(_)));
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct MlsConfig {
    pub enabled: bool,
    pub ciphersuite: String,
    pub identities: Vec<String>,
}

impl Default for MlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".into(),
            identities: Vec::new(),
        }
    }
}

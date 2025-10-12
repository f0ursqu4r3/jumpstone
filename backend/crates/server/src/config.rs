use std::{net::SocketAddr, str::FromStr};

use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to build configuration: {0}")]
    Build(#[from] config::ConfigError),
    #[error("invalid bind address: {0}")]
    InvalidBindAddr(String),
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
pub struct ServerConfig {
    pub bind_addr: Option<String>,
    pub host: String,
    pub port: u16,
    pub log_format: LogFormat,
    pub metrics: MetricsConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: None,
            host: "0.0.0.0".to_string(),
            port: 8080,
            log_format: LogFormat::Compact,
            metrics: MetricsConfig::default(),
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

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::InvalidBindAddr("port cannot be zero".into()));
        }
        if let Some(addr) = &self.metrics.bind_addr {
            addr.parse::<SocketAddr>()
                .map_err(|_| ConfigError::InvalidBindAddr(addr.clone()))?;
        }
        Ok(())
    }
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

    #[test]
    fn defaults_match_expectations() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.log_format, LogFormat::Compact);
        assert!(!config.metrics.enabled);
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
}

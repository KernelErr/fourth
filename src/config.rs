use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error as IOError, Read};
use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub base: ParsedConfig,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ParsedConfig {
    pub version: i32,
    pub log: Option<String>,
    pub servers: HashMap<String, ServerConfig>,
    pub upstream: HashMap<String, Upstream>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct BaseConfig {
    pub version: i32,
    pub log: Option<String>,
    pub servers: HashMap<String, ServerConfig>,
    pub upstream: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ServerConfig {
    pub listen: Vec<String>,
    pub protocol: Option<String>,
    pub tls: Option<bool>,
    pub sni: Option<HashMap<String, String>>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Upstream {
    Ban,
    Echo,
    Custom(CustomUpstream),
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomUpstream {
    pub name: String,
    pub addr: String,
    pub protocol: String,
}

#[derive(Debug)]
pub enum ConfigError {
    IO(IOError),
    Yaml(serde_yaml::Error),
    Custom(String),
}

impl Config {
    pub fn new(path: &str) -> Result<Config, ConfigError> {
        let base = (load_config(path))?;

        Ok(Config { base })
    }
}

fn load_config(path: &str) -> Result<ParsedConfig, ConfigError> {
    let mut contents = String::new();
    let mut file = (File::open(path))?;
    (file.read_to_string(&mut contents))?;

    let base: BaseConfig = serde_yaml::from_str(&contents)?;

    if base.version != 1 {
        return Err(ConfigError::Custom(
            "Unsupported config version".to_string(),
        ));
    }

    let log_level = base.log.clone().unwrap_or_else(|| "info".to_string());
    if !log_level.eq("disable") {
        std::env::set_var("FOURTH_LOG", log_level.clone());
        pretty_env_logger::init_custom_env("FOURTH_LOG");
        debug!("Set log level to {}", log_level);
    }

    debug!("Config version {}", base.version);

    let mut parsed_upstream: HashMap<String, Upstream> = HashMap::new();

    for (name, upstream) in base.upstream.iter() {
        let upstream_url = match Url::parse(upstream) {
            Ok(url) => url,
            Err(_) => {
                return Err(ConfigError::Custom(format!(
                    "Invalid upstream url \"{}\"",
                    upstream
                )))
            }
        };

        let upstream_host = match upstream_url.host_str() {
            Some(host) => host,
            None => {
                return Err(ConfigError::Custom(format!(
                    "Invalid upstream url \"{}\"",
                    upstream
                )))
            }
        };

        let upsteam_port = match upstream_url.port_or_known_default() {
            Some(port) => port,
            None => {
                return Err(ConfigError::Custom(format!(
                    "Invalid upstream url \"{}\"",
                    upstream
                )))
            }
        };

        parsed_upstream.insert(
            name.to_string(),
            Upstream::Custom(CustomUpstream {
                name: name.to_string(),
                addr: format!("{}:{}", upstream_host, upsteam_port),
                protocol: upstream_url.scheme().to_string(),
            }),
        );
    }

    parsed_upstream.insert(
        "ban".to_string(),
        Upstream::Ban,
    );

    parsed_upstream.insert(
        "echo".to_string(),
        Upstream::Echo,
    );

    let parsed = ParsedConfig {
        version: base.version,
        log: base.log,
        servers: base.servers,
        upstream: parsed_upstream,
    };

    // ToDo: validate config

    Ok(parsed)
}

impl From<IOError> for ConfigError {
    fn from(err: IOError) -> ConfigError {
        ConfigError::IO(err)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(err: serde_yaml::Error) -> ConfigError {
        ConfigError::Yaml(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let config = Config::new("tests/config.yaml").unwrap();
        assert_eq!(config.base.version, 1);
        assert_eq!(config.base.log.unwrap(), "disable");
        assert_eq!(config.base.servers.len(), 5);
        assert_eq!(config.base.upstream.len(), 3 + 2); // Add ban and echo upstreams
    }
}

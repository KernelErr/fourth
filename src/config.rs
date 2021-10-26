use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error as IOError, Read};

#[derive(Debug, Clone)]
pub struct Config {
    pub base: BaseConfig,
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

fn load_config(path: &str) -> Result<BaseConfig, ConfigError> {
    let mut contents = String::new();
    let mut file = (File::open(path))?;
    (file.read_to_string(&mut contents))?;

    let parsed: BaseConfig = serde_yaml::from_str(&contents)?;

    if parsed.version != 1 {
        return Err(ConfigError::Custom(
            "Unsupported config version".to_string(),
        ));
    }

    let log_level = parsed.log.clone().unwrap_or_else(|| "info".to_string());
    if !log_level.eq("disable") {
        std::env::set_var("FOURTH_LOG", log_level.clone());
        pretty_env_logger::init_custom_env("FOURTH_LOG");
        debug!("Set log level to {}", log_level);
    }

    debug!("Config version {}", parsed.version);

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
        assert_eq!(config.base.servers.len(), 4);
        assert_eq!(config.base.upstream.len(), 3);
    }
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub web_port: u16,
    pub proxy_http_port: u16,
    pub proxy_https_port: u16,
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            web_port: 19688,
            proxy_http_port: 80,
            proxy_https_port: 443,
            data_dir: PathBuf::from("./data"),
            static_dir: PathBuf::from("./static"),
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("UNVER_CONFIG")
            .unwrap_or_else(|_| "./data/config.toml".to_string());

        if std::path::Path::new(&config_path).exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("unver.db")
    }
}

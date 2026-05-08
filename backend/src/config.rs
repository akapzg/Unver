use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

        let mut config = if std::path::Path::new(&config_path).exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            Self::default()
        };

        config.resolve_paths();
        Ok(config)
    }

    fn resolve_paths(&mut self) {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        // Static files ship alongside the binary — resolve relative to it.
        if self.static_dir.is_relative() {
            self.static_dir = exe_dir.join(&self.static_dir);
        }
        // data_dir intentionally NOT resolved — it's controlled by
        // DATABASE_URL / UNVER_CONFIG env vars and may live anywhere.
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("unver.db")
    }
}

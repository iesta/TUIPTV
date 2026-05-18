use anyhow::{Context, Result};
use std::path::Path;

#[derive(Clone)]
pub struct Config {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Config {
    pub fn api_base(&self) -> String {
        format!(
            "{}/player_api.php?username={}&password={}",
            self.url.trim_end_matches('/'),
            self.username,
            self.password
        )
    }
}

pub fn load_env() -> Result<Option<Config>> {
    let env_path = Path::new(".env");
    if !env_path.exists() {
        return Ok(None);
    }
    dotenvy::from_path(env_path).context("failed to load .env")?;
    let url = std::env::var("XTREAM_URL").ok();
    let username = std::env::var("XTREAM_USER").ok();
    let password = std::env::var("XTREAM_PASS").ok();
    match (url, username, password) {
        (Some(url), Some(username), Some(password)) => Ok(Some(Config {
            url,
            username,
            password,
        })),
        _ => Ok(None),
    }
}

pub fn write_env(url: &str, username: &str, password: &str) -> Result<()> {
    let content = format!(
        "XTREAM_URL={}\nXTREAM_USER={}\nXTREAM_PASS={}\n",
        url, username, password
    );
    std::fs::write(".env", content).context("failed to write .env")?;
    Ok(())
}

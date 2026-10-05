use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const APP_NAME: &str = "ttui";

pub use crate::themes::Theme;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub pinned: std::collections::HashSet<String>,
    pub drafts: std::collections::HashMap<String, String>,
    pub read_at: std::collections::HashMap<String, i64>,
    #[serde(skip)]
    pub storage_path: Option<PathBuf>,
    // Authentication (persisted so restarts stay logged in)
    pub auth_token: Option<String>,
    pub device_id: Option<String>,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub user_name: Option<String>,

    // Behavior
    #[serde(default = "default_poll_ms")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_true")]
    pub image_enabled: bool,
    #[serde(default = "default_match_count")]
    pub match_count: u32,
    #[serde(default = "default_photo_w")]
    pub photo_width_cells: u16,
    #[serde(default = "default_photo_h")]
    pub photo_height_cells: u16,
    #[serde(default = "default_avatar_cells")]
    pub avatar_cells: u16,
    #[serde(default)]
    pub theme: Theme,
    /// Use the built-in offline mock API instead of the real Tinder API.
    #[serde(default)]
    pub mock: bool,
    /// Open full-resolution photos with an external viewer when available.
    #[serde(default)]
    pub external_photo_viewer: bool,
}

fn default_poll_ms() -> u64 {
    5000
}
fn default_true() -> bool {
    true
}
fn default_match_count() -> u32 {
    60
}
fn default_photo_w() -> u16 {
    46
}
fn default_photo_h() -> u16 {
    26
}
fn default_avatar_cells() -> u16 {
    10
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_path: None,
            pinned: Default::default(),
            drafts: Default::default(),
            read_at: Default::default(),
            auth_token: None,
            device_id: None,
            refresh_token: None,
            user_id: None,
            user_name: None,
            poll_interval_ms: default_poll_ms(),
            image_enabled: default_true(),
            match_count: default_match_count(),
            photo_width_cells: default_photo_w(),
            photo_height_cells: default_photo_h(),
            avatar_cells: default_avatar_cells(),
            theme: Theme::default(),
            mock: false,
            external_photo_viewer: false,
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(APP_NAME)
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn cache_dir() -> PathBuf {
        let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
        base.join(APP_NAME)
    }

    pub fn photo_cache_dir() -> PathBuf {
        Self::cache_dir().join("photos")
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth_token
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn load(mock: bool) -> std::io::Result<Config> {
        let path = Self::config_dir().join(if mock { "demo.json" } else { "config.json" });
        let mut config: Config = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Could not read {}: {e}. Your file has been preserved.",
                        path.display()
                    ),
                )
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e),
        };
        config.storage_path = Some(path);
        config.mock = mock;
        config.normalize();
        Ok(config)
    }

    pub fn normalize(&mut self) {
        self.poll_interval_ms = self.poll_interval_ms.clamp(1000, 60000);
        self.match_count = self.match_count.clamp(20, 100);
        self.avatar_cells = self.avatar_cells.clamp(4, 14);
        self.photo_width_cells = self.photo_width_cells.clamp(20, 80);
        self.photo_height_cells = self.photo_height_cells.clamp(10, 40);
    }

    /// Atomic replacement: secrets are owner-readable from the instant of creation.
    pub fn save(&self) -> std::io::Result<()> {
        let path = self.storage_path.clone().unwrap_or_else(Self::config_path);
        let dir = path
            .parent()
            .ok_or_else(|| std::io::Error::other("invalid config path"))?;
        fs::create_dir_all(dir)?;
        let temp = dir.join(format!(".config-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
            file.write_all(&json)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let c = Config {
            auth_token: Some("abc".into()),
            ..Default::default()
        };
        let s = serde_json::to_string(&c).unwrap();
        let d: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(d.auth_token.as_deref(), Some("abc"));
        assert_eq!(d.poll_interval_ms, 5000);
    }

    #[test]
    fn missing_fields_default() {
        let d: Config = serde_json::from_str("{}").unwrap();
        assert!(d.image_enabled);
        assert_eq!(d.photo_width_cells, 46);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn first_save_and_replacement_are_private_and_atomic() {
        let dir = std::env::temp_dir().join(format!("ttui-config-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("config.json");
        let mut config = Config {
            storage_path: Some(path.clone()),
            auth_token: Some("test-secret".into()),
            ..Default::default()
        };
        config.save().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        config.theme = Theme::Light;
        config.save().unwrap();
        let loaded: Config = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.auth_token.as_deref(), Some("test-secret"));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_dimensions_and_polling_are_bounded() {
        let mut c = Config {
            poll_interval_ms: 0,
            photo_width_cells: u16::MAX,
            photo_height_cells: 0,
            match_count: 0,
            ..Default::default()
        };
        c.normalize();
        assert_eq!(c.poll_interval_ms, 1000);
        assert_eq!(c.photo_width_cells, 80);
        assert_eq!(c.photo_height_cells, 10);
        assert_eq!(c.match_count, 20);
    }
}

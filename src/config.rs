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
        if let Some(root) = std::env::var_os("TTUI_DATA_DIR").filter(|s| !s.is_empty()) {
            return PathBuf::from(root).join("config");
        }
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(APP_NAME)
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn cache_dir() -> PathBuf {
        if let Some(root) = std::env::var_os("TTUI_DATA_DIR").filter(|s| !s.is_empty()) {
            return PathBuf::from(root).join("cache");
        }
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
        Self::load_path(path, mock)
    }

    fn load_path(path: PathBuf, mock: bool) -> std::io::Result<Config> {
        let mut config: Config = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_value(crate::secrets::decode(contents.as_bytes())?)
                .map_err(|e| {
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

    pub fn set_identity(&mut self, id: String, name: String) {
        if self.user_id.as_ref().is_some_and(|old| old != &id) {
            self.drafts.clear();
            self.pinned.clear();
            self.read_at.clear();
        }
        self.user_id = Some(id);
        self.user_name = Some(name);
    }

    pub fn clear_session(&mut self) {
        self.auth_token = None;
        self.refresh_token = None;
        self.device_id = None;
        self.user_id = None;
        self.user_name = None;
        self.drafts.clear();
        self.read_at.clear();
        self.pinned.clear();
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
            let value = serde_json::to_value(self).map_err(std::io::Error::other)?;
            let json = crate::secrets::encode(value, self.mock)?;
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

    fn account_config() -> Config {
        Config {
            user_id: Some("old-user".into()),
            user_name: Some("Old name".into()),
            auth_token: Some("test-auth".into()),
            refresh_token: Some("test-refresh".into()),
            device_id: Some("test-device".into()),
            drafts: [("conversation".into(), "Private draft".into())].into(),
            pinned: ["conversation".into()].into(),
            read_at: [("conversation".into(), 123)].into(),
            theme: Theme::Light,
            ..Default::default()
        }
    }

    #[test]
    fn changed_identity_clears_account_data_and_preserves_preferences() {
        let mut config = account_config();
        config.set_identity("new-user".into(), "New name".into());
        assert_eq!(config.user_id.as_deref(), Some("new-user"));
        assert_eq!(config.user_name.as_deref(), Some("New name"));
        assert!(config.drafts.is_empty());
        assert!(config.pinned.is_empty());
        assert!(config.read_at.is_empty());
        assert_eq!(config.theme, Theme::Light);
    }

    #[test]
    fn same_identity_rename_retains_account_data() {
        let mut config = account_config();
        config.set_identity("old-user".into(), "Updated name".into());
        assert_eq!(config.user_id.as_deref(), Some("old-user"));
        assert_eq!(config.user_name.as_deref(), Some("Updated name"));
        assert_eq!(config.drafts["conversation"], "Private draft");
        assert_eq!(config.pinned, ["conversation".into()].into());
        assert_eq!(config.read_at["conversation"], 123);
    }

    #[test]
    fn clear_session_removes_every_secret_and_account_field() {
        let mut config = account_config();
        config.clear_session();
        assert!(config.auth_token.is_none());
        assert!(config.refresh_token.is_none());
        assert!(config.device_id.is_none());
        assert!(config.user_id.is_none());
        assert!(config.user_name.is_none());
        assert!(config.drafts.is_empty());
        assert!(config.pinned.is_empty());
        assert!(config.read_at.is_empty());
        assert_eq!(config.theme, Theme::Light);
    }

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
        let loaded = Config::load_path(path.clone(), false).unwrap();
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

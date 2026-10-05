use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::api::client::{ApiError, TinderApi};
use crate::api::types::UserProfile;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrowserCredentials {
    pub auth_token: Option<String>,
    pub refresh_token: Option<String>,
    pub device_id: Option<String>,
    pub timestamp: u64,
}

impl BrowserCredentials {
    pub fn is_usable(&self) -> bool {
        self.auth_token
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
            || self
                .refresh_token
                .as_ref()
                .is_some_and(|t| !t.trim().is_empty())
    }
}

/// Discovers candidate Chromium-based browser profile directories on the current system.
pub fn candidate_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(unix)]
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return dirs,
    };

    #[cfg(target_os = "linux")]
    {
        let config = home.join(".config");
        let browser_bases = [
            config.join("google-chrome"),
            config.join("chromium"),
            config.join("BraveSoftware/Brave-Browser"),
            config.join("microsoft-edge"),
        ];
        for base in &browser_bases {
            if base.is_dir() {
                dirs.push(base.join("Default"));
                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir()
                            && entry.file_name().to_string_lossy().starts_with("Profile ")
                        {
                            dirs.push(path);
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let app_support = home.join("Library/Application Support");
        let browser_bases = [
            app_support.join("Google/Chrome"),
            app_support.join("Chromium"),
            app_support.join("BraveSoftware/Brave-Browser"),
            app_support.join("Microsoft Edge"),
        ];
        for base in &browser_bases {
            if base.is_dir() {
                dirs.push(base.join("Default"));
                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir()
                            && entry.file_name().to_string_lossy().starts_with("Profile ")
                        {
                            dirs.push(path);
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = dirs::data_local_dir() {
            let browser_bases = [
                local_app_data.join("Google/Chrome/User Data"),
                local_app_data.join("Chromium/User Data"),
                local_app_data.join("BraveSoftware/Brave-Browser/User Data"),
                local_app_data.join("Microsoft/Edge/User Data"),
            ];
            for base in &browser_bases {
                if base.is_dir() {
                    dirs.push(base.join("Default"));
                    if let Ok(entries) = std::fs::read_dir(base) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir()
                                && entry.file_name().to_string_lossy().starts_with("Profile ")
                            {
                                dirs.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    dirs
}

/// Extract candidate Tinder credentials from a single Chromium profile directory.
pub fn extract_from_profile(profile_dir: &Path) -> Option<BrowserCredentials> {
    let idb_dir = profile_dir.join("IndexedDB/https_tinder.com_0.indexeddb.leveldb");
    let ls_dir = profile_dir.join("Local Storage/leveldb");

    let (auth_token, refresh_token, timestamp) = scan_indexeddb(&idb_dir)?;
    let device_id = scan_local_storage(&ls_dir);

    Some(BrowserCredentials {
        auth_token,
        refresh_token,
        device_id,
        timestamp,
    })
}

/// Experimental extraction. Never silently select between browser profiles.
pub fn extract_freshest_credentials() -> Option<BrowserCredentials> {
    selected_profile()
        .ok()
        .and_then(|path| extract_from_profile(&path))
}

fn select_profile(paths: Vec<PathBuf>) -> Result<PathBuf, String> {
    let candidates: Vec<_> = paths
        .into_iter()
        .filter(|path| {
            path.join("IndexedDB/https_tinder.com_0.indexeddb.leveldb")
                .is_dir()
        })
        .collect();
    match candidates.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err("No supported Chromium Tinder profile found. Use session import, or set TTUI_BROWSER_PROFILE to the intended profile directory.".into()),
        _ => Err("Multiple Tinder browser profiles found. Set TTUI_BROWSER_PROFILE to the intended profile directory, or use session import.".into()),
    }
}

fn selected_profile() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("TTUI_BROWSER_PROFILE") {
        let path = PathBuf::from(path);
        if path.is_absolute() && path.is_dir() {
            return Ok(path);
        }
        return Err("TTUI_BROWSER_PROFILE must be an existing absolute profile directory.".into());
    }
    select_profile(candidate_profile_dirs())
}

fn scan_indexeddb(dir: &Path) -> Option<(Option<String>, Option<String>, u64)> {
    if !dir.is_dir() {
        return None;
    }

    let mut best_auth: Option<String> = None;
    let mut best_refresh: Option<String> = None;
    let mut best_ts: u64 = 0;

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.ends_with(".log") && !name.ends_with(".ldb") {
            continue;
        }

        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };

        let mut idx = 0;
        let needle = b"\"authToken\":";
        while idx < bytes.len() {
            let Some(found) = find_subslice(&bytes[idx..], needle) else {
                break;
            };
            let pos = idx + found;
            idx = pos + needle.len();

            // Find start of JSON object containing this field
            let scan_back = pos.saturating_sub(60);
            let start = match bytes[scan_back..pos].iter().rposition(|&b| b == b'{') {
                Some(p) => scan_back + p,
                None => continue,
            };

            // Scan forward with brace balancing to locate the end of the JSON object
            let mut depth = 0usize;
            let mut end = None;
            let max_scan = (start + 4096).min(bytes.len());
            for (i, &b) in bytes[start..max_scan].iter().enumerate() {
                if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = Some(start + i + 1);
                        break;
                    }
                }
            }

            let Some(end) = end else {
                continue;
            };

            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes[start..end]) {
                let tok = val
                    .get("authToken")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned);
                let ref_tok = val
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned);
                let ts = val
                    .pointer("/__PERSIST__/timestamp")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if (tok.is_some() || ref_tok.is_some())
                    && (ts >= best_ts || (best_auth.is_none() && best_refresh.is_none()))
                {
                    best_ts = ts;
                    best_auth = tok;
                    best_refresh = ref_tok;
                }
            }
        }
    }

    if best_auth.is_some() || best_refresh.is_some() {
        Some((best_auth, best_refresh, best_ts))
    } else {
        None
    }
}

fn scan_local_storage(dir: &Path) -> Option<String> {
    if !dir.is_dir() {
        return None;
    }

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.ends_with(".log") && !name.ends_with(".ldb") {
            continue;
        }

        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };

        let mut idx = 0;
        let needle = b"Web/uuid";
        while idx < bytes.len() {
            let Some(found) = find_subslice(&bytes[idx..], needle) else {
                break;
            };
            let pos = idx + found;
            idx = pos + needle.len();

            let end = (pos + 120).min(bytes.len());
            if let Some(uuid) = find_uuid(&bytes[pos..end]) {
                return Some(uuid);
            }
        }
    }

    None
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find_uuid(bytes: &[u8]) -> Option<String> {
    // Looks for 8-4-4-4-12 hex format
    for window in bytes.windows(36) {
        let is_uuid = window[8] == b'-'
            && window[13] == b'-'
            && window[18] == b'-'
            && window[23] == b'-'
            && window[..8].iter().all(u8::is_ascii_hexdigit)
            && window[9..13].iter().all(u8::is_ascii_hexdigit)
            && window[14..18].iter().all(u8::is_ascii_hexdigit)
            && window[19..23].iter().all(u8::is_ascii_hexdigit)
            && window[24..36].iter().all(u8::is_ascii_hexdigit);

        if is_uuid {
            return String::from_utf8(window.to_vec()).ok();
        }
    }
    None
}

/// Opens the system browser to https://tinder.com
pub fn open_browser_to_tinder() -> std::io::Result<()> {
    crate::platform::open("https://tinder.com")
}

/// Helper to verify or renew credentials with TinderApi.
pub async fn verify_or_renew_credentials(
    api: &dyn TinderApi,
    mut creds: BrowserCredentials,
) -> Result<(String, String, Option<String>, UserProfile), ApiError> {
    let device_id = creds
        .device_id
        .take()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // 1. If auth_token is present, try get_own_user
    if let Some(tok) = &creds.auth_token {
        api.apply_auth(tok.clone(), device_id.clone(), creds.refresh_token.clone());
        match api.get_own_user().await {
            Ok(own) => return Ok((tok.clone(), device_id, creds.refresh_token, own)),
            Err(e) if e.is_auth() => {
                // Auth token expired or invalid; try refresh token below
            }
            Err(e) => return Err(e),
        }
    }

    // 2. If refresh_token is present, attempt refresh
    if let Some(refresh) = &creds.refresh_token {
        api.apply_auth(String::new(), device_id.clone(), Some(refresh.clone()));
        let renewed = api.refresh_token(refresh).await?;
        let new_auth = renewed.auth_token;
        let new_refresh = renewed.refresh_token.or(Some(refresh.clone()));
        api.apply_auth(new_auth.clone(), device_id.clone(), new_refresh.clone());
        let own = api.get_own_user().await?;
        return Ok((new_auth, device_id, new_refresh, own));
    }

    Err(ApiError::Auth)
}

/// Polls browser profiles periodically until fresh credentials appear and are verified.
pub async fn poll_browser_login(
    api: Arc<dyn TinderApi>,
    baseline_ts: u64,
    timeout: Duration,
) -> Result<(String, String, Option<String>, UserProfile), String> {
    let profile = selected_profile()?;
    let start = std::time::Instant::now();
    let mut check_interval = tokio::time::interval(Duration::from_millis(1000));
    check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut tried = None;

    loop {
        check_interval.tick().await;

        if start.elapsed() > timeout {
            return Err("Browser login timed out after 3 minutes. Please try again.".into());
        }

        if let Some(creds) = extract_from_profile(&profile)
            && tried.as_ref() != Some(&creds)
            && (tried.is_none() || creds.timestamp > baseline_ts)
        {
            tried = Some(creds.clone());
            match verify_or_renew_credentials(api.as_ref(), creds).await {
                Ok(res) => return Ok(res),
                Err(ApiError::Network(_)) => {
                    // Network error during verification, keep polling
                    continue;
                }
                Err(_) => {
                    // If initial attempt failed, loop will wait for creds with timestamp > baseline_ts
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uuid_from_raw_bytes() {
        let sample =
            b"\x01Web/uuid\x13\x15\x00\x01\x01\xd8\x01b66005f2-de64-4194-88d3-186d09f48217\x15\x14";
        assert_eq!(
            find_uuid(sample),
            Some("b66005f2-de64-4194-88d3-186d09f48217".to_string())
        );
    }

    #[test]
    fn parses_indexeddb_mock_blob() {
        let dir = std::env::temp_dir().join(format!("ttui-idb-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_file = dir.join("000001.log");
        let content = b"junk data {\"authToken\":\"test-auth-123\",\"refreshToken\":\"test-refresh-456\",\"__PERSIST__\":{\"timestamp\":1234567890}} trailing";
        std::fs::write(&log_file, content).unwrap();

        let res = scan_indexeddb(&dir);
        assert!(res.is_some());
        let (auth, refresh, ts) = res.unwrap();
        assert_eq!(auth.as_deref(), Some("test-auth-123"));
        assert_eq!(refresh.as_deref(), Some("test-refresh-456"));
        assert_eq!(ts, 1234567890);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn newest_record_never_inherits_another_records_credentials() {
        let dir = std::env::temp_dir().join(format!("ttui-idb-coherence-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // These synthetic records test parser isolation, not Chromium compatibility.
        for (auth, refresh) in [(Some("new-auth"), None), (None, Some("new-refresh"))] {
            let old = r#"{"authToken":"old-auth","refreshToken":"old-refresh","__PERSIST__":{"timestamp":100}}"#;
            let newest = format!(
                r#"{{"authToken":{},"refreshToken":{},"__PERSIST__":{{"timestamp":300}}}}"#,
                serde_json::to_string(&auth).unwrap(),
                serde_json::to_string(&refresh).unwrap()
            );
            // The old record occurs last to prove selection uses its timestamp.
            std::fs::write(dir.join("000001.log"), format!("{newest}\0{old}")).unwrap();
            std::fs::write(
                dir.join("ignored.txt"),
                r#"{"authToken":"ignored","__PERSIST__":{"timestamp":900}}"#,
            )
            .unwrap();
            let (actual_auth, actual_refresh, timestamp) = scan_indexeddb(&dir).unwrap();
            assert_eq!(actual_auth.as_deref(), auth);
            assert_eq!(actual_refresh.as_deref(), refresh);
            assert_eq!(timestamp, 300);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn profile_selection_requires_exactly_one_tinder_profile() {
        let root = std::env::temp_dir().join(format!("ttui-profiles-{}", uuid::Uuid::new_v4()));
        let first = root.join("Default");
        let second = root.join("Profile 1");
        let unrelated = root.join("Unrelated");
        std::fs::create_dir_all(&unrelated).unwrap();
        assert!(
            select_profile(vec![unrelated.clone()])
                .unwrap_err()
                .contains("No supported")
        );
        for profile in [&first, &second] {
            std::fs::create_dir_all(profile.join("IndexedDB/https_tinder.com_0.indexeddb.leveldb"))
                .unwrap();
        }
        assert_eq!(
            select_profile(vec![unrelated.clone(), first.clone()]).unwrap(),
            first
        );
        let error = select_profile(vec![first, unrelated, second]).unwrap_err();
        assert!(error.contains("Multiple Tinder browser profiles"));
        assert!(error.contains("TTUI_BROWSER_PROFILE"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "explicit opt-in integration check; reads personal browser storage"]
    fn extracts_real_credentials_if_available() {
        if let Some(creds) = extract_freshest_credentials() {
            assert!(creds.is_usable());
            println!(
                "Real creds discovered: auth={:?} refresh={:?} device={:?} ts={}",
                creds.auth_token.is_some(),
                creds.refresh_token.is_some(),
                creds.device_id.is_some(),
                creds.timestamp
            );
        }
    }
}

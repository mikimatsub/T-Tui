//! Login screen state and key handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Config;

#[derive(Debug)]
pub struct LoginView {
    pub focus: usize,
    pub auth_token: String,
    pub device_id: String,
    pub refresh_token: String,
    pub busy: bool,
    pub browser_waiting: bool,
    pub error: Option<String>,
}

impl LoginView {
    pub fn new(config: &Config) -> Self {
        Self {
            focus: 0,
            auth_token: config.auth_token.clone().unwrap_or_default(),
            device_id: config.device_id.clone().unwrap_or_default(),
            refresh_token: config.refresh_token.clone().unwrap_or_default(),
            busy: false,
            browser_waiting: false,
            error: None,
        }
    }

    fn field_mut(&mut self, i: usize) -> &mut String {
        match i {
            0 => &mut self.auth_token,
            1 => &mut self.device_id,
            2 => &mut self.refresh_token,
            _ => &mut self.auth_token,
        }
    }

    /// Handle a key event. Returns `true` when a submit was requested.
    pub fn handle_key(&mut self, ev: KeyEvent) -> bool {
        if self.busy {
            return false;
        }
        if ev.modifiers.contains(KeyModifiers::CONTROL) {
            if ev.code == KeyCode::Char('u') {
                self.field_mut(self.focus).clear();
            }
            return false;
        }
        match ev.code {
            KeyCode::Tab => self.focus = (self.focus + 1) % 3,
            KeyCode::BackTab => self.focus = (self.focus + 2) % 3,
            KeyCode::Enter => return true,
            KeyCode::Backspace => {
                self.field_mut(self.focus).pop();
            }
            KeyCode::Char(c) => {
                self.field_mut(self.focus).push(c);
            }
            _ => {}
        }
        false
    }

    pub fn paste(&mut self, text: &str) {
        if self.busy {
            return;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
            let field = |keys: &[&str]| {
                keys.iter()
                    .find_map(|k| v.get(k).and_then(|v| v.as_str()))
                    .map(str::to_owned)
            };
            if let (Some(token), Some(device)) = (
                field(&["auth_token", "authToken"]),
                field(&["device_id", "deviceId", "persistent-device-id"]),
            ) {
                self.auth_token = token;
                self.device_id = device;
                if let Some(refresh) = field(&["refresh_token", "refreshToken"]) {
                    self.refresh_token = refresh;
                }
                self.error = None;
                return;
            }
        }
        let header = |name: &str| -> Option<String> {
            let lower = text.to_ascii_lowercase();
            let start = lower.find(&format!("{name}:"))? + name.len() + 1;
            let value: String = text[start..]
                .trim_start()
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, '\'' | '"'))
                .collect();
            (!value.is_empty()).then_some(value)
        };
        if text.trim_start().starts_with("curl ") || text.contains("x-auth-token:") {
            if let (Some(token), Some(device)) =
                (header("x-auth-token"), header("persistent-device-id"))
            {
                self.auth_token = token;
                self.device_id = device;
                self.error = None;
            } else {
                self.error = Some("This request is missing session headers. Copy a request to api.gotinder.com as cURL.".into());
            }
            return;
        }
        self.field_mut(self.focus)
            .extend(text.chars().filter(|c| !c.is_control()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn tabs_cycle_fields() {
        let mut v = LoginView::new(&Config::default());
        assert_eq!(v.focus, 0);
        v.handle_key(key(KeyCode::Tab));
        assert_eq!(v.focus, 1);
        v.handle_key(key(KeyCode::BackTab));
        assert_eq!(v.focus, 0);
        v.handle_key(key(KeyCode::BackTab));
        assert_eq!(v.focus, 2);
    }

    #[test]
    fn typing_and_backspace() {
        let mut v = LoginView::new(&Config::default());
        for c in "abc".chars() {
            v.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(v.auth_token, "abc");
        v.handle_key(key(KeyCode::Backspace));
        assert_eq!(v.auth_token, "ab");
    }

    #[test]
    fn enter_submits_when_ready() {
        let mut v = LoginView::new(&Config::default());
        v.auth_token.push_str("tok");
        v.device_id.push_str("dev");
        assert!(v.handle_key(key(KeyCode::Enter)));
        v.busy = true;
        assert!(!v.handle_key(key(KeyCode::Enter)));
        v.busy = false;
        assert!(v.handle_key(key(KeyCode::Enter)));
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    #[test]
    fn imports_only_session_fields_from_browser_curl() {
        let mut login = LoginView::new(&Config::default());
        login.paste("curl 'https://api.gotinder.com/v2/matches' \\\n-H 'X-Auth-Token: tok' \\\n-H 'persistent-device-id: device' \\\n-H 'cookie: unrelated-private-value'");
        assert_eq!(login.auth_token, "tok");
        assert_eq!(login.device_id, "device");
        assert!(login.refresh_token.is_empty());
        assert!(login.error.is_none());
    }
    #[test]
    fn malformed_curl_never_becomes_the_token() {
        let mut login = LoginView::new(&Config::default());
        login.paste("curl 'https://example.com' -H 'accept: */*'");
        assert!(login.auth_token.is_empty());
        assert!(login.error.is_some());
    }
}

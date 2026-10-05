//! Hit targets are recorded by the renderer, so mouse and visual layout stay in sync.
use super::{App, Screen, toast};
use crate::config::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxFilter {
    All,
    Unread,
    New,
    Drafts,
    Pinned,
}
impl InboxFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Unread,
        Self::New,
        Self::Drafts,
        Self::Pinned,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Unread => "Unread",
            Self::New => "New",
            Self::Drafts => "Drafts",
            Self::Pinned => "Pinned",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxSort {
    Recent,
    Name,
    Newest,
}
impl InboxSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "Recent",
            Self::Name => "Name",
            Self::Newest => "Newest",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollTarget {
    Inbox,
    Messages,
    Details,
    Settings,
    Help,
    Themes,
    Account,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AskSuperLike,
    AskBoost,
    ConfirmPremium,
    CancelPremium,
    Web,
    ReportHelp,
    Navigate(Screen),
    Key(KeyCode),
    Help,
    Themes,
    Theme(usize),
    ApplyTheme,
    CancelTheme,
    Chat(String),
    SelectedChat,
    Profile,
    Search,
    ClearSearch,
    Filter,
    Sort,
    Pin,
    MarkAllRead,
    Settings(usize),
    LoginField(usize),
    Connect,
    BrowserLogin,
    ClearLogin,
    Scroll(ScrollTarget),
    Photo(bool),
    Editor { rect: Rect, start: usize },
    AccountField(usize),
    AccountCursor { rect: Rect, start: usize },
    SaveAccount,
    ReloadAccount,
    DiscardAccount,
    OwnProfile,
    Unmatch,
    ConfirmUnmatch,
    CancelUnmatch,
    ConfirmSignout,
    CancelSignout,
    Send,
    Older,
    Latest,
    RestoreDraft,
    ExternalPhoto,
}
#[derive(Debug, Clone)]
pub enum PremiumAction {
    SuperLike(Box<crate::api::types::Recommendation>),
    Boost,
}

#[derive(Debug, Clone)]
pub struct HitRegion {
    pub rect: Rect,
    pub action: Action,
}

impl App {
    pub fn hit(&self, rect: Rect, action: Action) {
        if rect.width > 0 && rect.height > 0 {
            self.hits.borrow_mut().push(HitRegion { rect, action });
        }
    }
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        let point = Position::new(ev.column, ev.row);
        match ev.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let target = self
                    .hits
                    .borrow()
                    .iter()
                    .rev()
                    .find(|h| h.rect.contains(point) && matches!(h.action, Action::Scroll(_)))
                    .map(|h| h.action.clone());
                if let Some(Action::Scroll(target)) = target {
                    self.scroll(
                        target,
                        if ev.kind == MouseEventKind::ScrollUp {
                            -1
                        } else {
                            1
                        },
                    );
                }
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                let hit = self
                    .hits
                    .borrow()
                    .iter()
                    .rev()
                    .find(|h| h.rect.contains(point) && !matches!(h.action, Action::Scroll(_)))
                    .map(|h| h.action.clone());
                if let Some(action) = hit {
                    match action {
                        Action::Editor { rect, start } => {
                            self.cursor = super::account::cursor_at(
                                &self.input,
                                rect.width.max(1) as usize,
                                (ev.column - rect.x) as usize,
                                start + (ev.row - rect.y) as usize,
                            );
                        }
                        Action::AccountCursor { rect, start }
                            if !self.account.saving && !self.account.loading =>
                        {
                            self.account.select(0);
                            if let Some(bio) = self.account.fields.first() {
                                self.account.cursor = super::account::cursor_at(
                                    bio,
                                    rect.width.max(1) as usize,
                                    (ev.column - rect.x) as usize,
                                    start + (ev.row - rect.y) as usize,
                                );
                            }
                        }
                        _ if matches!(ev.kind, MouseEventKind::Down(_)) => self.activate(action),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        self.request_visible_photos();
    }
    fn scroll(&mut self, target: ScrollTarget, delta: isize) {
        match target {
            ScrollTarget::Inbox => self.move_selection(delta),
            ScrollTarget::Messages => {
                self.msg_scroll = self.msg_scroll.saturating_add_signed(-delta * 3);
                if delta < 0 {
                    self.load_older();
                }
            }
            ScrollTarget::Details => {
                if self.screen == Screen::Discover {
                    self.discovery_scroll = self
                        .discovery_scroll
                        .saturating_add_signed(delta * 3)
                        .min(1000);
                } else if let Some(p) = &mut self.profile {
                    p.info_scroll = p.info_scroll.saturating_add_signed(delta * 3).min(1000);
                }
            }
            ScrollTarget::Settings => {
                self.settings.sel = self
                    .settings
                    .sel
                    .saturating_add_signed(delta)
                    .min(super::settings::ROW_COUNT - 1)
            }
            ScrollTarget::Help => {
                self.help_scroll = self.help_scroll.saturating_add_signed(delta * 3).min(100)
            }
            ScrollTarget::Themes => self.preview_theme(
                self.theme_sel
                    .saturating_add_signed(delta)
                    .min(Theme::ALL.len() - 1),
            ),
            ScrollTarget::Account => self.account.select(
                self.account
                    .focus
                    .saturating_add_signed(delta)
                    .min(super::account::LABELS.len() - 1),
            ),
        }
    }
    pub fn activate(&mut self, action: Action) {
        match action {
            Action::AskSuperLike => {
                if !self.swiping && !self.discovery_loading && !self.refreshing
                    && let Some(rec) = self.discovery.front() {
                    self.premium = Some(PremiumAction::SuperLike(Box::new(rec.clone())));
                }
            }
            Action::AskBoost => { if !self.premium_busy && !self.refreshing { self.premium = Some(PremiumAction::Boost); } }
            Action::ConfirmPremium => self.submit_premium(),
            Action::CancelPremium => { if !self.premium_busy { self.premium = None; } }
            Action::Web => self.open_web("https://tinder.com/app/recs"),
            Action::ReportHelp => self.open_web("https://www.help.tinder.com/hc/en-us/articles/115003822043-Reporting-profiles-and-content"),

            Action::Navigate(screen) => {
                if self.screen == Screen::Chat {
                    self.stash_chat();
                    self.save_config();
                }
                self.searching = false;
                match screen {
                    Screen::Discover => self.open_discovery(),
                    Screen::Settings => {
                        if self.screen != Screen::Settings {
                            self.open_settings();
                        }
                    }
                    Screen::Account => self.open_account(),
                    _ => self.screen = screen,
                }
            }
            Action::Key(code) => {
                self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
            }
            Action::Help => {
                self.help = !self.help;
                self.help_scroll = 0;
            }
            Action::Themes => self.open_themes(),
            Action::Theme(i) => self.preview_theme(i),
            Action::ApplyTheme => {
                self.theme_original = None;
                self.save_config();
            }
            Action::CancelTheme => {
                if let Some(theme) = self.theme_original.take() {
                    self.config.theme = theme;
                }
            }
            Action::Chat(id) => self.open_chat(&id),
            Action::SelectedChat => {
                if let Some(id) = self.active_match_id() {
                    self.searching = false;
                    self.open_chat(&id);
                }
            }
            Action::Profile => self.selected_profile(),
            Action::Search => {
                if self.screen == Screen::Chat { self.stash_chat(); self.save_config(); }
                self.screen = Screen::Matches;
                self.searching = true;
            }
            Action::ClearSearch => {
                self.search.clear();
                self.searching = false;
                self.select_first_result();
            }
            Action::Filter => {
                let i = InboxFilter::ALL
                    .iter()
                    .position(|f| *f == self.inbox_filter)
                    .unwrap_or(0);
                self.inbox_filter = InboxFilter::ALL[(i + 1) % InboxFilter::ALL.len()];
                self.select_first_result();
            }
            Action::Sort => {
                self.inbox_sort = match self.inbox_sort {
                    InboxSort::Recent => InboxSort::Name,
                    InboxSort::Name => InboxSort::Newest,
                    InboxSort::Newest => InboxSort::Recent,
                };
                self.select_first_result();
            }
            Action::Pin => {
                if let Some(id) = self.active_match_id() {
                    if !self.config.pinned.remove(&id) {
                        self.config.pinned.insert(id);
                    }
                    self.save_config();
                    if !self.filtered_matches().contains(&self.match_sel) {
                        self.select_first_result();
                    }
                }
            }
            Action::MarkAllRead => {
                let ids: Vec<_> = self.matches.iter().map(|m| m.id.clone()).collect();
                for id in ids {
                    self.mark_read(&id);
                }
                self.save_config();
            }
            Action::Settings(i) => {
                self.settings.sel = i;
                self.apply_settings_row();
            }
            Action::LoginField(i) => {
                if !self.login.busy {
                    self.login.focus = i;
                }
            }
            Action::Connect => self.submit_login(),
            Action::BrowserLogin => self.start_browser_login(),
            Action::ClearLogin => {
                self.login
                    .handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
            }
            Action::Photo(forward) => {
                if self.screen == Screen::Discover {
                    self.cycle_discovery_photo(forward);
                } else if self.screen == Screen::Profile {
                    self.handle_key(KeyEvent::new(
                        if forward {
                            KeyCode::Right
                        } else {
                            KeyCode::Left
                        },
                        KeyModifiers::NONE,
                    ));
                }
            }
            Action::AccountField(i) => {
                self.account.select(i);
                if i > 3 {
                    self.account.change_choice();
                }
            }
            Action::SaveAccount => self.save_account(),
            Action::ReloadAccount => self.reload_account(),
            Action::DiscardAccount => self.account.discard(),
            Action::OwnProfile => {
                if let Some(own) = self.own.clone() {
                    self.profile_back = self.screen;
                    self.profile = Some(super::ProfileState {
                        user_id: own.id.clone(),
                        from_match_id: None,
                        loading: false,
                        profile: Some(own),
                        photo_idx: 0,
                        info_scroll: 0,
                        error: None,
                    });
                    self.screen = Screen::Profile;
                }
            }
            Action::Unmatch => {
                if let Some(id) = self.active_match_id()
                    && let Some(m) = self.matches.iter().find(|m| m.id == id)
                {
                    self.confirm_unmatch = Some((
                        id,
                        m.person
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| "this person".into()),
                    ));
                }
            }
            Action::ConfirmUnmatch => self.submit_unmatch(),
            Action::CancelUnmatch => {
                if self.unmatching.is_none() {
                    self.confirm_unmatch = None;
                }
            }
            Action::ConfirmSignout => self.sign_out(),
            Action::CancelSignout => self.confirm_signout = false,
            Action::Send => self.send(),
            Action::Older => self.load_older(),
            Action::Latest => self.msg_scroll = 0,
            Action::RestoreDraft => self.retry_failed(),
            Action::ExternalPhoto => self.open_external_viewer(),
            Action::Scroll(_) | Action::Editor { .. } | Action::AccountCursor { .. } => {}
        }
        self.request_visible_photos();
    }
    pub fn open_themes(&mut self) {
        self.theme_original = Some(self.config.theme);
        self.theme_sel = Theme::ALL
            .iter()
            .position(|t| *t == self.config.theme)
            .unwrap_or(0);
    }
    fn preview_theme(&mut self, i: usize) {
        self.theme_sel = i.min(Theme::ALL.len() - 1);
        self.config.theme = Theme::ALL[self.theme_sel];
    }
    pub fn active_match_id(&self) -> Option<String> {
        match self.screen {
            Screen::Chat => self.chat_match_id.clone(),
            Screen::Profile => self.profile.as_ref().and_then(|p| p.from_match_id.clone()),
            _ => self
                .matches
                .get(self.match_sel)
                .filter(|_| self.filtered_matches().contains(&self.match_sel))
                .map(|m| m.id.clone()),
        }
    }
    pub fn open_account(&mut self) {
        self.screen = Screen::Account;
        if self.account.fields.is_empty() && !self.account.loading {
            self.reload_account();
        }
    }
    pub fn reload_account(&mut self) {
        if self.account.saving || self.account.loading {
            return;
        }
        if self.account.dirty() {
            self.account.error = Some("Save or discard your edits before reloading.".into());
            return;
        }
        self.account.loading = true;
        self.account.error = None;
        let api = self.api.clone();
        self.spawn(async move { super::AppEvent::AccountLoaded(api.get_own_user().await) });
    }
    pub fn save_account(&mut self) {
        if self.account.saving || self.account.loading {
            return;
        }
        if !self.account.dirty() {
            self.toasts.push(toast("No account changes to save."));
            return;
        }
        let update = match self.account.update() {
            Ok(u) => u,
            Err(e) => {
                self.account.error = Some(e);
                return;
            }
        };
        self.account.saving = true;
        self.account.error = None;
        let api = self.api.clone();
        self.spawn(async move {
            let result = api.update_profile(&update).await;
            super::AppEvent::AccountSaved { update, result }
        });
    }
    pub fn submit_unmatch(&mut self) {
        if self.unmatching.is_some() {
            return;
        }
        let Some((id, _)) = self.confirm_unmatch.clone() else {
            return;
        };
        self.unmatching = Some(id.clone());
        let api = self.api.clone();
        self.spawn(async move {
            let result = api.unmatch(&id).await;
            super::AppEvent::Unmatched { id, result }
        });
    }
}

impl App {
    pub fn modal_open(&self) -> bool {
        self.help
            || self.confirm_signout
            || self.confirm_unmatch.is_some()
            || self.theme_original.is_some()
            || self.premium.is_some()
    }
    pub fn submit_premium(&mut self) {
        if self.premium_busy || self.refreshing {
            return;
        }
        let Some(action) = self.premium.clone() else {
            return;
        };
        let api = self.api.clone();
        match action {
            PremiumAction::SuperLike(rec) => {
                if self.swiping
                    || self
                        .discovery
                        .front()
                        .is_none_or(|r| r.user.id != rec.user.id)
                {
                    return;
                }
                if self.super_likes_remaining == Some(0) {
                    self.premium = None;
                    self.toasts.push(toast("No Super Likes remaining."));
                    return;
                }
                self.premium_busy = true;
                self.swiping = true;
                self.spawn(async move {
                    let result = api.super_like(&rec).await;
                    super::AppEvent::SuperLiked {
                        id: rec.user.id,
                        result,
                    }
                });
            }
            PremiumAction::Boost => {
                if self.boosts_remaining == Some(0) {
                    self.premium = None;
                    self.toasts.push(toast("No Boosts remaining."));
                    return;
                }
                self.premium_busy = true;
                self.spawn(async move { super::AppEvent::Boosted(api.boost().await) });
            }
        }
    }
    fn open_web(&mut self, url: &'static str) {
        self.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                std::process::Command::new(if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                })
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            })
            .await;
            super::AppEvent::Notice(match result {
                Ok(Ok(status)) if status.success() => "Opened Tinder in your browser.".into(),
                _ => format!("Could not open a browser. Visit {url}"),
            })
        });
    }
}

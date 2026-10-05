//! UI state and session-scoped background work. Network I/O never blocks drawing.
pub mod account;
pub mod interaction;
pub mod login;
pub mod settings;
pub mod ui;

use crate::api::client::{ApiError, TinderApi};
use crate::api::types::*;
use crate::config::{Config, Theme};
use crate::photos::{PhotoPipeline, PhotoReady, SizeClass};
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use interaction::{Action, HitRegion, InboxFilter, InboxSort, PremiumAction};
use login::LoginView;
use settings::{ROW_COUNT, SettingsView};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Matches,
    Chat,
    Profile,
    Discover,
    Settings,
    Account,
}

#[derive(Debug)]
pub enum AppEvent {
    Session(u64, Box<AppEvent>),
    Startup(Result<(UserProfile, Vec<Match>), ApiError>),
    MatchesLoaded(Result<Vec<Match>, ApiError>),
    MessagesLoaded {
        match_id: String,
        initial: bool,
        result: Result<MessagesData, ApiError>,
    },
    MessageSent {
        match_id: String,
        local_id: String,
        result: Result<Message, ApiError>,
    },
    ProfileLoaded {
        user_id: String,
        result: Result<UserProfile, ApiError>,
    },
    Updates(Result<UpdateResponse, ApiError>),
    RefreshDone(Result<RefreshResult, ApiError>),
    LoginDone(Result<UserProfile, ApiError>),
    DiscoveryLoaded(Result<Vec<Recommendation>, ApiError>),
    SwipeDone {
        user_id: String,
        like: bool,
        result: Result<SwipeResult, ApiError>,
    },
    SuperLiked {
        id: String,
        result: Result<(SwipeResult, Option<u64>), ApiError>,
    },
    Boosted(Result<Option<u64>, ApiError>),
    DiscoveryRevision {
        revision: u64,
        result: Result<Vec<Recommendation>, ApiError>,
    },
    AccountLoaded(Result<UserProfile, ApiError>),
    AccountSaved {
        update: ProfileUpdate,
        result: Result<(), ApiError>,
    },
    Unmatched {
        id: String,
        result: Result<(), ApiError>,
    },
    BrowserLoginDone(Result<(String, String, Option<String>, UserProfile), String>),
    Photo(PhotoReady),
    Notice(String),
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub until: Instant,
}
#[derive(Debug, Clone)]
pub struct ChatMsg {
    pub m: Message,
    pub pending: bool,
    pub failed: bool,
}
#[derive(Debug, Clone)]
pub struct ProfileState {
    pub user_id: String,
    pub from_match_id: Option<String>,
    pub loading: bool,
    pub profile: Option<UserProfile>,
    pub photo_idx: usize,
    pub info_scroll: usize,
    pub error: Option<String>,
}
pub fn toast(text: &str) -> Toast {
    Toast {
        text: text.into(),
        until: Instant::now() + Duration::from_secs(5),
    }
}
fn cycle<T: Copy + PartialEq>(cur: &mut T, values: &[T]) {
    *cur = values[values
        .iter()
        .position(|x| x == cur)
        .map(|i| (i + 1) % values.len())
        .unwrap_or(0)];
}

pub struct App {
    pub premium: Option<PremiumAction>,
    pub premium_busy: bool,
    pub super_likes_remaining: Option<u64>,
    pub boosts_remaining: Option<u64>,
    discovery_revision: u64,
    pub hits: RefCell<Vec<HitRegion>>,
    pub theme_original: Option<Theme>,
    pub theme_sel: usize,
    pub inbox_filter: InboxFilter,
    pub inbox_sort: InboxSort,
    pub account: account::AccountEditor,
    pub confirm_unmatch: Option<(String, String)>,
    pub unmatching: Option<String>,
    removed_matches: HashSet<String>,
    pub config: Config,
    pub api: Arc<dyn TinderApi>,
    pub photos: PhotoPipeline,
    pub tx: mpsc::UnboundedSender<AppEvent>,
    pub screen: Screen,
    pub own: Option<UserProfile>,
    pub matches: Vec<Match>,
    pub match_sel: usize,
    pub unread: HashMap<String, usize>,
    pub last_msg: HashMap<String, String>,
    pub matches_loading: bool,
    pub inbox_limit: u32,
    draft_changed: Option<Instant>,
    pub status: Option<String>,
    pub toasts: Vec<Toast>,
    pub last_sync: Option<Instant>,
    pub search: String,
    pub searching: bool,
    pub chat_match_id: Option<String>,
    pub messages: Vec<ChatMsg>,
    pub msg_scroll: usize,
    pub has_more: bool,
    pub chat_loading: bool,
    pub loading_more: bool,
    pub input: String,
    pub cursor: usize,
    pub profile: Option<ProfileState>,
    pub profile_back: Screen,
    pub settings_back: Screen,
    pub login: LoginView,
    pub settings: SettingsView,
    pub help: bool,
    pub help_scroll: usize,
    pub confirm_signout: bool,
    pub signout_error: Option<String>,
    pub refreshing: bool,
    pub discovery: VecDeque<Recommendation>,
    pub discovery_loading: bool,
    pub discovery_error: Option<String>,
    pub discovery_photo: usize,
    pub discovery_scroll: usize,
    pub swiping: bool,
    pub likes_remaining: Option<u64>,
    page_token: Option<String>,
    threads: HashMap<String, Vec<ChatMsg>>,
    seen: HashSet<String>,
    swiped: HashSet<String>,
    poller: Option<AbortHandle>,
    tasks: Vec<AbortHandle>,
    generation: u64,
    login_refreshed: bool,
    since: Arc<Mutex<DateTime<Utc>>>,
}

impl App {
    pub fn new(
        mut config: Config,
        api: Arc<dyn TinderApi>,
        photos: PhotoPipeline,
        tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Self {
        config.normalize();
        photos.set_sizes(
            config.avatar_cells,
            config.photo_width_cells,
            config.photo_height_cells,
        );
        let login = LoginView::new(&config);
        let screen = if config.mock || config.is_authenticated() || config.refresh_token.is_some() {
            Screen::Matches
        } else {
            Screen::Login
        };
        Self {
            premium: None,
            premium_busy: false,
            super_likes_remaining: None,
            boosts_remaining: None,
            discovery_revision: 0,
            hits: RefCell::new(Vec::new()),
            theme_original: None,
            theme_sel: 0,
            inbox_filter: InboxFilter::All,
            inbox_sort: InboxSort::Recent,
            account: account::AccountEditor::default(),
            confirm_unmatch: None,
            unmatching: None,
            removed_matches: HashSet::new(),
            inbox_limit: config.match_count,
            draft_changed: None,
            config,
            api,
            photos,
            tx,
            screen,
            own: None,
            matches: vec![],
            match_sel: 0,
            unread: HashMap::new(),
            last_msg: HashMap::new(),
            matches_loading: false,
            status: None,
            toasts: vec![],
            last_sync: None,
            search: String::new(),
            searching: false,
            chat_match_id: None,
            messages: vec![],
            msg_scroll: 0,
            has_more: false,
            chat_loading: false,
            loading_more: false,
            input: String::new(),
            cursor: 0,
            profile: None,
            profile_back: Screen::Matches,
            settings_back: Screen::Matches,
            login,
            settings: SettingsView::default(),
            help: false,
            help_scroll: 0,
            confirm_signout: false,
            signout_error: None,
            refreshing: false,
            discovery: VecDeque::new(),
            discovery_loading: false,
            discovery_error: None,
            discovery_photo: 0,
            discovery_scroll: 0,
            swiping: false,
            likes_remaining: None,
            page_token: None,
            threads: HashMap::new(),
            seen: HashSet::new(),
            swiped: HashSet::new(),
            poller: None,
            tasks: vec![],
            generation: 0,
            login_refreshed: false,
            since: Arc::new(Mutex::new(Utc::now())),
        }
    }

    fn spawn(&mut self, future: impl Future<Output = AppEvent> + Send + 'static) {
        let tx = self.tx.clone();
        let generation = self.generation;
        self.tasks.retain(|h| !h.is_finished());
        self.tasks.push(
            tokio::spawn(async move {
                let ev = future.await;
                let _ = tx.send(AppEvent::Session(generation, Box::new(ev)));
            })
            .abort_handle(),
        );
    }
    pub fn own_id(&self) -> Option<String> {
        self.own.as_ref().map(|p| p.id.clone())
    }
    pub fn start(&mut self) {
        if self.matches_loading {
            return;
        }
        if self.screen == Screen::Login {
            return;
        }
        if !self.config.mock && !self.config.is_authenticated() {
            self.start_refresh();
            return;
        }
        self.matches_loading = true;
        let api = self.api.clone();
        let count = self.config.match_count;
        self.spawn(async move {
            AppEvent::Startup(
                async {
                    let own = api.get_own_user().await?;
                    let matches = api.get_matches(count, false).await?;
                    Ok((own, matches))
                }
                .await,
            )
        });
    }
    pub fn start_poller(&mut self) {
        if self.poller.is_some() || self.own.is_none() {
            return;
        }
        let api = self.api.clone();
        let tx = self.tx.clone();
        let since = self.since.clone();
        let generation = self.generation;
        let base = Duration::from_millis(self.config.poll_interval_ms);
        self.poller = Some(
            tokio::spawn(async move {
                let mut backoff = base;
                loop {
                    tokio::time::sleep(backoff).await;
                    let time = *since.lock().unwrap();
                    let result = api.get_updates(time).await;
                    let auth = result.as_ref().is_err_and(|e| e.is_auth());
                    backoff = if result.is_ok() {
                        base
                    } else {
                        (backoff * 2).min(Duration::from_secs(60))
                    };
                    if tx
                        .send(AppEvent::Session(
                            generation,
                            Box::new(AppEvent::Updates(result)),
                        ))
                        .is_err()
                        || auth
                    {
                        break;
                    }
                }
            })
            .abort_handle(),
        );
    }
    pub fn stop_poller(&mut self) {
        if let Some(h) = self.poller.take() {
            h.abort();
        }
    }
    fn save_config(&mut self) {
        let mut saved = self.config.clone();
        if let Some(theme) = self.theme_original {
            saved.theme = theme;
        }
        if let Err(e) = saved.save() {
            self.toasts
                .push(toast(&format!("Could not save settings: {e}")));
            if self.draft_changed.is_some() {
                self.draft_changed = Some(Instant::now() + Duration::from_secs(5));
            }
        } else {
            self.draft_changed = None;
        }
    }
    fn stash_chat(&mut self) {
        if let Some(id) = &self.chat_match_id {
            self.config.drafts.insert(id.clone(), self.input.clone());
            self.threads.insert(id.clone(), self.messages.clone());
        }
    }
    pub fn shutdown(&mut self) {
        if let Some(theme) = self.theme_original.take() {
            self.config.theme = theme;
        }
        self.stash_chat();
        // An unopened login form should not create or overwrite credentials.
        if self.own.is_some() || self.config.mock {
            self.save_config();
        }
        self.stop_poller();
        for h in self.tasks.drain(..) {
            h.abort();
        }
    }
    fn error(&mut self, e: ApiError) {
        self.status = Some(e.to_string());
        if e.is_auth() {
            self.stop_poller();
            if self
                .config
                .refresh_token
                .as_ref()
                .is_some_and(|s| !s.is_empty())
            {
                self.start_refresh();
            } else {
                self.go_login(
                    "Session expired. Press Ctrl-B or click [Browser Login] to reconnect.",
                );
            }
        }
    }
    pub fn filtered_matches(&self) -> Vec<usize> {
        let query = self.search.to_lowercase();
        let mut indices: Vec<_> = self
            .matches
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                let text_match = query.is_empty()
                    || m.person
                        .as_ref()
                        .is_some_and(|p| p.name.to_lowercase().contains(&query))
                    || self
                        .last_msg
                        .get(&m.id)
                        .is_some_and(|s| s.to_lowercase().contains(&query));
                text_match
                    && match self.inbox_filter {
                        InboxFilter::All => true,
                        InboxFilter::Unread => self.unread.get(&m.id).is_some_and(|n| *n > 0),
                        InboxFilter::New => {
                            m.message_count == 0 && !self.last_msg.contains_key(&m.id)
                        }
                        InboxFilter::Drafts => {
                            self.config.drafts.get(&m.id).is_some_and(|s| !s.is_empty())
                        }
                        InboxFilter::Pinned => self.config.pinned.contains(&m.id),
                    }
            })
            .map(|(i, _)| i)
            .collect();
        indices.sort_by(|a, b| {
            let (a, b) = (&self.matches[*a], &self.matches[*b]);
            self.config
                .pinned
                .contains(&b.id)
                .cmp(&self.config.pinned.contains(&a.id))
                .then_with(|| match self.inbox_sort {
                    InboxSort::Recent => b.last_activity().cmp(&a.last_activity()),
                    InboxSort::Newest => b.created_date.cmp(&a.created_date),
                    InboxSort::Name => a
                        .person
                        .as_ref()
                        .map(|p| p.name.to_lowercase())
                        .cmp(&b.person.as_ref().map(|p| p.name.to_lowercase())),
                })
        });
        indices
    }
    fn move_selection(&mut self, delta: isize) {
        let indices = self.filtered_matches();
        if indices.is_empty() {
            return;
        }
        let pos = indices
            .iter()
            .position(|i| *i == self.match_sel)
            .unwrap_or(0);
        self.match_sel =
            indices[(pos as isize + delta).clamp(0, indices.len() as isize - 1) as usize];
    }
    fn select_first_result(&mut self) {
        self.match_sel = self.filtered_matches().first().copied().unwrap_or(0);
    }
    pub fn handle_key(&mut self, ev: KeyEvent) -> bool {
        let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(ev.code, KeyCode::Char('q' | 'c')) {
            return true;
        }
        if self.premium.is_some() {
            match ev.code {
                KeyCode::Enter => self.submit_premium(),
                KeyCode::Esc => self.activate(Action::CancelPremium),
                _ => {}
            }
            return false;
        }
        if self.confirm_unmatch.is_some() {
            match ev.code {
                KeyCode::Enter => self.submit_unmatch(),
                KeyCode::Esc => self.activate(Action::CancelUnmatch),
                _ => {}
            }
            return false;
        }
        if self.theme_original.is_some() {
            match ev.code {
                KeyCode::Esc | KeyCode::F(2) => self.activate(Action::CancelTheme),
                KeyCode::Enter => self.activate(Action::ApplyTheme),
                KeyCode::Down | KeyCode::Char('j') => self.activate(Action::Theme(
                    (self.theme_sel + 1).min(Theme::ALL.len() - 1),
                )),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.activate(Action::Theme(self.theme_sel.saturating_sub(1)))
                }
                KeyCode::Home => self.activate(Action::Theme(0)),
                KeyCode::End => self.activate(Action::Theme(Theme::ALL.len() - 1)),
                KeyCode::PageDown => self.activate(Action::Theme(
                    (self.theme_sel + 8).min(Theme::ALL.len() - 1),
                )),
                KeyCode::PageUp => self.activate(Action::Theme(self.theme_sel.saturating_sub(8))),
                _ => {}
            }
            return false;
        }
        if self.confirm_signout {
            match ev.code {
                KeyCode::Enter => self.sign_out(),
                KeyCode::Esc => self.confirm_signout = false,
                _ => {}
            }
            return false;
        }
        if self.help {
            match ev.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => self.help = false,
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = (self.help_scroll + 1).min(100)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => self.help_scroll = (self.help_scroll + 10).min(100),
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
                _ => {}
            }
            return false;
        }
        if ev.code == KeyCode::F(2) {
            self.open_themes();
            return false;
        }
        if ctrl && ev.code == KeyCode::Char('o') && self.screen != Screen::Login {
            self.activate(Action::Navigate(Screen::Account));
            return false;
        }
        if ev.code == KeyCode::F(1)
            || (ev.code == KeyCode::Char('?')
                && !matches!(self.screen, Screen::Chat | Screen::Login | Screen::Account)
                && !self.searching)
        {
            self.help = true;
            self.help_scroll = 0;
            return false;
        }
        match self.screen {
            Screen::Login => {
                if ctrl && ev.code == KeyCode::Char('b') {
                    self.start_browser_login();
                } else if self.login.busy && ev.code == KeyCode::Esc {
                    self.cancel_browser_login();
                } else if self.login.handle_key(ev) {
                    self.submit_login();
                }
            }
            Screen::Matches => {
                if self.searching {
                    match ev.code {
                        KeyCode::Esc => {
                            self.search.clear();
                            self.searching = false;
                            self.select_first_result();
                        }
                        KeyCode::Enter => self.searching = false,
                        KeyCode::Backspace => {
                            self.search.pop();
                            self.select_first_result();
                        }
                        KeyCode::Char(c) if !ctrl => {
                            self.search.push(c);
                            self.select_first_result();
                        }
                        KeyCode::Down => self.move_selection(1),
                        KeyCode::Up => self.move_selection(-1),
                        _ => {}
                    }
                    return false;
                }
                match ev.code {
                    KeyCode::Char('q') => return true,
                    KeyCode::Char('/') => self.searching = true,
                    KeyCode::Char('L') => self.load_more_matches(),
                    KeyCode::Esc => {
                        self.search.clear();
                        self.select_first_result();
                    }
                    KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
                    KeyCode::PageDown => self.move_selection(8),
                    KeyCode::PageUp => self.move_selection(-8),
                    KeyCode::Home => self.move_selection(-10000),
                    KeyCode::End => self.move_selection(10000),
                    KeyCode::Enter if !self.filtered_matches().is_empty() => {
                        if let Some(m) = self.matches.get(self.match_sel) {
                            self.open_chat(&m.id.clone());
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Right if !self.filtered_matches().is_empty() => {
                        self.selected_profile()
                    }
                    KeyCode::Char('f') => self.activate(Action::Filter),
                    KeyCode::Char('o') => self.activate(Action::Sort),
                    KeyCode::Char('p') => self.activate(Action::Pin),
                    KeyCode::Char('a') => self.activate(Action::MarkAllRead),
                    KeyCode::Char('s') => self.open_settings(),
                    KeyCode::Char('d') | KeyCode::Tab => self.open_discovery(),
                    KeyCode::Char('r') => {
                        if self.own.is_some() {
                            self.load_matches();
                        } else {
                            self.start();
                        }
                    }
                    _ => {}
                }
            }
            Screen::Account => {
                if ctrl && ev.code == KeyCode::Char('s') {
                    self.save_account();
                } else if ev.code == KeyCode::Esc {
                    self.screen = Screen::Matches;
                } else {
                    self.account.key(ev);
                }
            }
            Screen::Chat => self.chat_key(ev),
            Screen::Profile => match ev.code {
                KeyCode::Esc => {
                    self.profile = None;
                    self.screen = self.profile_back;
                }
                KeyCode::Enter => {
                    let mid = self.profile.as_ref().and_then(|p| p.from_match_id.clone());
                    self.profile = None;
                    self.screen = self.profile_back;
                    if let Some(mid) = mid {
                        if self.chat_match_id.as_deref() == Some(&mid) {
                            self.screen = Screen::Chat;
                        } else {
                            self.open_chat(&mid);
                        }
                    }
                }
                KeyCode::Char('u') => self.activate(Action::Unmatch),
                KeyCode::Char('w') => self.activate(Action::Web),
                KeyCode::Char('v') => self.open_external_viewer(),
                KeyCode::Char('r') => {
                    if let Some(p) = self.profile.clone() {
                        let back = self.profile_back;
                        self.open_profile(&p.user_id, p.from_match_id);
                        self.profile_back = back;
                    }
                }
                _ => {
                    if let Some(p) = &mut self.profile {
                        let n = p.profile.as_ref().map(|u| u.photos.len()).unwrap_or(0);
                        match ev.code {
                            KeyCode::Left | KeyCode::Char('h') if n > 0 => {
                                p.photo_idx = (p.photo_idx + n - 1) % n
                            }
                            KeyCode::Right | KeyCode::Char('l') if n > 0 => {
                                p.photo_idx = (p.photo_idx + 1) % n
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                p.info_scroll = p.info_scroll.saturating_sub(1)
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                p.info_scroll = (p.info_scroll + 1).min(1000)
                            }
                            _ => {}
                        }
                    }
                }
            },
            Screen::Discover => match ev.code {
                KeyCode::Char('m') | KeyCode::Tab | KeyCode::Esc => self.screen = Screen::Matches,
                KeyCode::Char('s') => self.open_settings(),
                KeyCode::Char('r') => self.load_discovery(),
                KeyCode::Char('y') => self.swipe(true),
                KeyCode::Char('u') => self.activate(Action::AskSuperLike),
                KeyCode::Char('b') => self.activate(Action::AskBoost),
                KeyCode::Char('w') => self.activate(Action::Web),
                KeyCode::Char('n') => self.swipe(false),
                KeyCode::Left | KeyCode::Char('h') => self.cycle_discovery_photo(false),
                KeyCode::Right | KeyCode::Char('l') => self.cycle_discovery_photo(true),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.discovery_scroll = self.discovery_scroll.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.discovery_scroll = (self.discovery_scroll + 1).min(1000)
                }
                _ => {}
            },
            Screen::Settings => match ev.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.settings.sel = (self.settings.sel + 1) % ROW_COUNT
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.settings.sel = (self.settings.sel + ROW_COUNT - 1) % ROW_COUNT
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => self.apply_settings_row(),
                KeyCode::Esc => self.screen = self.settings_back,
                _ => {}
            },
        }
        self.request_visible_photos();
        false
    }
    pub fn handle_paste(&mut self, s: &str) {
        if self.help
            || self.confirm_signout
            || self.theme_original.is_some()
            || self.confirm_unmatch.is_some()
            || self.premium.is_some()
        {
            return;
        }
        if self.screen == Screen::Login {
            self.login.paste(s);
        } else if self.screen == Screen::Account {
            self.account.insert(s);
        } else if self.screen == Screen::Chat {
            let clean: String = s
                .chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .map(|c| if c == '\t' { ' ' } else { c })
                .collect();
            self.insert_text(&clean);
        } else if self.searching {
            self.search.extend(s.chars().filter(|c| !c.is_control()));
            self.select_first_result();
        }
    }
    fn insert_text(&mut self, s: &str) {
        // Limit draft size without cutting through UTF-8.
        let room = 5000usize.saturating_sub(self.input.chars().count());
        let s: String = s.chars().take(room).collect();
        self.cursor = self.cursor.min(self.input.len());
        self.input.insert_str(self.cursor, &s);
        self.cursor += s.len();
    }
    fn chat_key(&mut self, ev: KeyEvent) {
        let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match ev.code {
                KeyCode::Char('u') => {
                    self.input.clear();
                    self.cursor = 0;
                }
                KeyCode::Char('a') => self.cursor = 0,
                KeyCode::Char('e') => self.cursor = self.input.len(),
                KeyCode::Char('p') => self.selected_profile(),
                KeyCode::Char('r') => self.retry_failed(),
                KeyCode::Char('x') => self.activate(Action::Unmatch),
                KeyCode::Char('s') => self.open_settings(),
                _ => {}
            }
            return;
        }
        match ev.code {
            KeyCode::Esc => {
                self.stash_chat();
                self.save_config();
                self.screen = Screen::Matches;
            }
            KeyCode::Enter
                if ev.modifiers.contains(KeyModifiers::SHIFT)
                    || ev.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_text("\n")
            }
            KeyCode::Enter => self.send(),
            KeyCode::Char(c) if !ev.modifiers.contains(KeyModifiers::ALT) => {
                self.insert_text(&c.to_string())
            }
            KeyCode::Left => self.cursor = previous_boundary(&self.input, self.cursor),
            KeyCode::Right => self.cursor = next_boundary(&self.input, self.cursor),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                let prev = previous_boundary(&self.input, self.cursor);
                self.input.drain(prev..self.cursor);
                self.cursor = prev;
            }
            KeyCode::Delete if self.cursor < self.input.len() => {
                self.input
                    .drain(self.cursor..next_boundary(&self.input, self.cursor));
            }
            KeyCode::Up => self.msg_scroll = self.msg_scroll.saturating_add(1),
            KeyCode::Down => self.msg_scroll = self.msg_scroll.saturating_sub(1),
            KeyCode::PageUp => {
                self.msg_scroll = self.msg_scroll.saturating_add(10);
                self.load_older();
            }
            KeyCode::PageDown => self.msg_scroll = self.msg_scroll.saturating_sub(10),
            _ => {}
        }
    }
    fn selected_profile(&mut self) {
        if self.screen == Screen::Matches && !self.filtered_matches().contains(&self.match_sel) {
            return;
        }
        let m = if self.screen == Screen::Chat {
            self.matches
                .iter()
                .find(|m| Some(&m.id) == self.chat_match_id.as_ref())
        } else {
            self.matches.get(self.match_sel)
        };
        if let Some(m) = m
            && let Some(p) = &m.person
        {
            self.open_profile(&p.id.clone(), Some(m.id.clone()));
        }
    }
    pub fn open_settings(&mut self) {
        if self.screen == Screen::Settings {
            return;
        }
        if self.screen == Screen::Chat {
            self.stash_chat();
            self.save_config();
        }
        self.settings_back = self.screen;
        self.screen = Screen::Settings;
    }
    pub fn open_chat(&mut self, id: &str) {
        if self.screen == Screen::Chat && self.chat_match_id.as_deref() == Some(id) {
            return;
        }
        self.stash_chat();
        self.chat_match_id = Some(id.into());
        self.match_sel = self.matches.iter().position(|m| m.id == id).unwrap_or(0);
        self.messages = self.threads.get(id).cloned().unwrap_or_default();
        self.input = self.config.drafts.get(id).cloned().unwrap_or_default();
        self.cursor = self.input.len();
        self.msg_scroll = 0;
        self.chat_loading = true;
        self.loading_more = false;
        self.has_more = false;
        self.page_token = None;
        self.mark_read(id);
        self.screen = Screen::Chat;
        self.status = None;
        let api = self.api.clone();
        let mid = id.to_owned();
        self.spawn(async move {
            AppEvent::MessagesLoaded {
                match_id: mid.clone(),
                initial: true,
                result: api.get_messages(&mid, 50, None).await,
            }
        });
    }
    fn mark_read(&mut self, id: &str) {
        self.unread.remove(id);
        self.config
            .read_at
            .insert(id.into(), Utc::now().timestamp_millis());
        if let Some(m) = self.matches.iter_mut().find(|m| m.id == id) {
            m.is_new_match = false;
        }
    }
    pub fn open_profile(&mut self, id: &str, mid: Option<String>) {
        self.profile_back = self.screen;
        self.profile = Some(ProfileState {
            user_id: id.into(),
            from_match_id: mid,
            loading: true,
            profile: None,
            photo_idx: 0,
            info_scroll: 0,
            error: None,
        });
        self.screen = Screen::Profile;
        let api = self.api.clone();
        let uid = id.to_owned();
        self.spawn(async move {
            AppEvent::ProfileLoaded {
                user_id: uid.clone(),
                result: api.get_user(&uid).await,
            }
        });
    }
    pub fn send(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.chat_loading || self.refreshing {
            return;
        }
        let Some(mid) = self.chat_match_id.clone() else {
            return;
        };
        let Some(own) = self.own_id() else {
            self.toasts.push(toast("Reconnect before sending."));
            return;
        };
        let Some(other) = self
            .matches
            .iter()
            .find(|m| m.id == mid)
            .and_then(|m| m.other_id(&own))
        else {
            return;
        };
        let local_id = format!("local-{}", uuid::Uuid::new_v4());
        self.messages.push(ChatMsg {
            m: Message {
                id: local_id.clone(),
                message: text.clone(),
                from: Some(own.clone()),
                to: Some(other.clone()),
                timestamp: Utc::now().timestamp_millis(),
                match_id: Some(mid.clone()),
                sent_date: None,
            },
            pending: true,
            failed: false,
        });
        self.input.clear();
        self.cursor = 0;
        self.config.drafts.remove(&mid);
        self.msg_scroll = 0;
        let api = self.api.clone();
        self.spawn(async move {
            AppEvent::MessageSent {
                match_id: mid.clone(),
                local_id,
                result: api.send_message(&own, &other, &mid, &text).await,
            }
        });
    }
    fn retry_failed(&mut self) {
        if !self.input.is_empty() {
            self.toasts.push(toast(
                "Keep or send your draft before restoring a failed message.",
            ));
            return;
        }
        if let Some(c) = self.messages.iter().rev().find(|c| c.failed) {
            self.input = c.m.message.clone();
            self.cursor = self.input.len();
            self.toasts.push(toast(
                "Restored draft. Check history before resending; delivery may be uncertain.",
            ));
        }
    }
    pub fn load_older(&mut self) {
        if self.loading_more || self.chat_loading || !self.has_more {
            return;
        }
        let Some(mid) = self.chat_match_id.clone() else {
            return;
        };
        let token = self.page_token.clone();
        let api = self.api.clone();
        self.loading_more = true;
        self.spawn(async move {
            AppEvent::MessagesLoaded {
                match_id: mid.clone(),
                initial: false,
                result: api.get_messages(&mid, 50, token).await,
            }
        });
    }
    pub fn submit_login(&mut self) {
        if self.login.busy || self.refreshing {
            return;
        }
        let token = self.login.auth_token.trim().to_owned();
        let device = self.login.device_id.trim().to_owned();
        let refresh = self.login.refresh_token.trim().to_owned();
        if device.is_empty() || (token.is_empty() && refresh.is_empty()) {
            self.login.error = Some("Enter a device ID and an auth token or refresh token.".into());
            return;
        }
        self.login_refreshed = false;
        self.login.busy = true;
        self.login.error = None;
        self.status = None;
        self.config.device_id = Some(device.clone());
        self.config.auth_token = (!token.is_empty()).then_some(token.clone());
        self.config.refresh_token = (!refresh.is_empty()).then_some(refresh);
        self.api
            .apply_auth(token.clone(), device, self.config.refresh_token.clone());
        if token.is_empty() {
            self.start_refresh();
            return;
        }
        let api = self.api.clone();
        self.spawn(async move { AppEvent::LoginDone(api.get_own_user().await) });
    }
    pub fn start_browser_login(&mut self) {
        if self.config.mock {
            self.login.error = Some("Browser login is unavailable in offline demo mode.".into());
            return;
        }
        if self.login.busy {
            return;
        }
        self.login.busy = true;
        self.login.browser_waiting = true;
        self.login.error = None;
        self.status = Some("Opening browser to https://tinder.com… Log in in your browser".into());

        let baseline_ts = crate::browser::extract_freshest_credentials()
            .map(|c| c.timestamp)
            .unwrap_or(0);

        let _ = crate::browser::open_browser_to_tinder();

        let api = self.api.clone();
        self.spawn(async move {
            let res =
                crate::browser::poll_browser_login(api, baseline_ts, Duration::from_secs(180))
                    .await;
            AppEvent::BrowserLoginDone(res)
        });
    }
    pub fn cancel_browser_login(&mut self) {
        self.generation += 1;
        for task in self.tasks.drain(..) {
            task.abort();
        }
        self.login.busy = false;
        self.login.browser_waiting = false;
        self.status = None;
    }
    fn finish_login(&mut self, own: UserProfile) {
        self.adopt_identity(&own);
        self.own = Some(own);
        self.screen = Screen::Matches;
        self.status = None;
        self.login.error = None;
        self.login.busy = false;
        self.login.browser_waiting = false;
        self.save_config();
        self.load_matches();
        self.start_poller();
    }

    fn adopt_identity(&mut self, own: &UserProfile) {
        if self.own.as_ref().is_some_and(|old| old.id != own.id)
            || self
                .config
                .user_id
                .as_ref()
                .is_some_and(|old| old != &own.id)
        {
            self.inbox_limit = self.config.match_count;
            self.draft_changed = None;
            self.account = account::AccountEditor::default();
            self.matches.clear();
            self.match_sel = 0;
            self.profile = None;
            self.seen.clear();
            self.photos.clear();
            self.search.clear();
            self.searching = false;
            self.inbox_filter = InboxFilter::All;
            self.super_likes_remaining = None;
            self.boosts_remaining = None;
            *self.since.lock().unwrap() = Utc::now();
            self.config.drafts.clear();
            self.config.read_at.clear();
            self.config.pinned.clear();
            self.messages.clear();
            self.threads.clear();
            self.unread.clear();
            self.last_msg.clear();
            self.chat_match_id = None;
            self.input.clear();
            self.cursor = 0;
            self.discovery.clear();
            self.swiped.clear();
            self.removed_matches.clear();
        }

        self.config.set_identity(own.id.clone(), own.name.clone());
    }
    pub fn start_refresh(&mut self) {
        if self.refreshing {
            return;
        }
        let Some(token) = self.config.refresh_token.clone().filter(|t| !t.is_empty()) else {
            self.go_login("Please sign in again.");
            return;
        };
        self.stop_poller();
        self.login_refreshed = true;
        self.refreshing = true;
        let api = self.api.clone();
        self.spawn(async move { AppEvent::RefreshDone(api.refresh_token(&token).await) });
    }
    fn go_login(&mut self, message: &str) {
        self.generation += 1;
        for task in self.tasks.drain(..) {
            task.abort();
        }
        self.stop_poller();
        self.premium = None;
        self.premium_busy = false;
        self.confirm_unmatch = None;
        self.unmatching = None;
        self.swiping = false;
        self.discovery_loading = false;
        self.matches_loading = false;
        self.chat_loading = false;
        self.loading_more = false;
        self.account.saving = false;
        self.account.loading = false;
        for msg in self
            .messages
            .iter_mut()
            .chain(self.threads.values_mut().flatten())
        {
            if msg.pending {
                msg.pending = false;
                msg.failed = true;
            }
        }
        self.stash_chat();
        self.screen = Screen::Login;
        self.login = LoginView::new(&self.config);
        self.login.error = Some(message.into());
        self.login.busy = false;
    }
    pub fn sign_out(&mut self) {
        let mut cleared = self.config.clone();
        if let Some(theme) = self.theme_original {
            cleared.theme = theme;
        }
        cleared.clear_session();
        if let Err(error) = cleared.save() {
            self.signout_error = Some(format!(
                "Sign-out failed: {error}. The saved session remains. Fix file access and retry, or cancel."
            ));
            self.confirm_signout = true;
            return;
        }
        self.config = cleared;
        self.inbox_limit = self.config.match_count;
        self.draft_changed = None;
        self.signout_error = None;
        self.generation += 1;
        self.stop_poller();
        for h in self.tasks.drain(..) {
            h.abort();
        }
        self.api.apply_auth(String::new(), String::new(), None);
        self.config.auth_token = None;
        self.config.refresh_token = None;
        self.config.device_id = None;
        self.config.user_id = None;
        self.config.user_name = None;
        self.config.pinned.clear();
        self.premium = None;
        self.premium_busy = false;
        self.super_likes_remaining = None;
        self.boosts_remaining = None;
        self.discovery_revision += 1;
        self.account = account::AccountEditor::default();
        self.confirm_unmatch = None;
        self.unmatching = None;
        self.removed_matches.clear();
        self.inbox_filter = InboxFilter::All;
        self.theme_original = None;
        self.config.drafts.clear();
        self.config.read_at.clear();
        self.own = None;
        self.matches.clear();
        self.messages.clear();
        self.threads.clear();
        self.seen.clear();
        self.unread.clear();
        self.last_msg.clear();
        self.chat_match_id = None;
        self.input.clear();
        self.cursor = 0;
        self.discovery.clear();
        self.swiped.clear();
        self.swiping = false;
        self.discovery_loading = false;
        self.discovery_error = None;
        self.profile = None;
        self.photos.clear();
        self.refreshing = false;
        self.matches_loading = false;
        self.chat_loading = false;
        self.search.clear();
        self.searching = false;
        self.confirm_signout = false;
        self.status = None;
        self.last_sync = None;
        self.screen = Screen::Login;
        self.login = LoginView::new(&self.config);
        self.toasts.clear();
        *self.since.lock().unwrap() = Utc::now();
    }
    pub fn apply_settings_row(&mut self) {
        match self.settings.sel {
            0 => {
                self.open_themes();
                return;
            }
            1 => {
                self.toasts.push(toast(
                    "Launch with --mock for demo, or --live for your account.",
                ));
                return;
            }
            2 => self.config.image_enabled = !self.config.image_enabled,
            3 => self.config.external_photo_viewer = !self.config.external_photo_viewer,
            4 => {
                cycle(
                    &mut self.config.poll_interval_ms,
                    &[1000, 2000, 5000, 10000, 30000],
                );
                self.stop_poller();
                self.start_poller();
            }
            5 => {
                cycle(&mut self.config.match_count, &[20, 40, 60, 100]);
                self.inbox_limit = self.config.match_count;
                self.load_matches();
            }
            6 => cycle(&mut self.config.photo_width_cells, &[30, 46, 60]),
            7 => cycle(&mut self.config.photo_height_cells, &[18, 26, 34]),
            8 => cycle(&mut self.config.avatar_cells, &[6, 8, 10, 14]),
            9 => {
                self.load_matches();
                return;
            }
            10 => {
                self.confirm_signout = true;
                return;
            }
            _ => return,
        }
        self.photos.set_sizes(
            self.config.avatar_cells,
            self.config.photo_width_cells,
            self.config.photo_height_cells,
        );
        self.save_config();
        self.request_visible_photos();
    }
    pub fn load_matches(&mut self) {
        if self.matches_loading {
            return;
        }
        self.matches_loading = true;
        let api = self.api.clone();
        let count = self.inbox_limit;
        self.spawn(async move { AppEvent::MatchesLoaded(api.get_matches(count, false).await) });
    }
    pub fn load_more_matches(&mut self) {
        if self.matches_loading {
            return;
        }
        if self.inbox_limit >= 1000 {
            self.toasts
                .push(toast("The session limit is 1,000 conversations."));
            return;
        }
        self.inbox_limit = (self.inbox_limit + 100).min(1000);
        self.load_matches();
    }
    pub fn open_discovery(&mut self) {
        self.screen = Screen::Discover;
        if self.discovery.is_empty() {
            self.load_discovery();
        }
    }
    pub fn load_discovery(&mut self) {
        if self.discovery_loading || self.swiping {
            return;
        }
        self.discovery_loading = true;
        self.discovery_error = None;
        let api = self.api.clone();
        let revision = self.discovery_revision;
        self.spawn(async move {
            AppEvent::DiscoveryRevision {
                revision,
                result: api.get_recommendations().await,
            }
        });
    }
    fn cycle_discovery_photo(&mut self, forward: bool) {
        let n = self
            .discovery
            .front()
            .map(|r| r.user.photos.len())
            .unwrap_or(0);
        if n > 0 {
            self.discovery_photo = if forward {
                (self.discovery_photo + 1) % n
            } else {
                (self.discovery_photo + n - 1) % n
            };
        }
    }
    pub fn swipe(&mut self, like: bool) {
        if self.swiping || self.refreshing || self.discovery_loading {
            return;
        }
        if like && self.likes_remaining == Some(0) {
            self.discovery_error =
                Some("No likes remaining. Try again after Tinder resets your allowance.".into());
            return;
        }
        let Some(rec) = self.discovery.front().cloned() else {
            return;
        };
        self.swiping = true;
        self.discovery_error = None;
        let api = self.api.clone();
        self.spawn(async move {
            AppEvent::SwipeDone {
                user_id: rec.user.id.clone(),
                like,
                result: api.swipe(&rec, like).await,
            }
        });
    }

    pub fn handle_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Session(generation, ev) => {
                if generation == self.generation {
                    self.handle_event(*ev);
                }
            }
            AppEvent::Startup(result) => {
                self.matches_loading = false;
                match result {
                    Ok((own, matches)) => {
                        self.adopt_identity(&own);
                        self.own = Some(own);
                        self.replace_matches(matches);
                        self.status = None;
                        self.last_sync = Some(Instant::now());
                        self.start_poller();
                    }
                    Err(e) => self.error(e),
                }
            }
            AppEvent::MatchesLoaded(result) => {
                self.matches_loading = false;
                match result {
                    Ok(matches) => {
                        self.replace_matches(matches);
                        self.status = None;
                        self.last_sync = Some(Instant::now());
                    }
                    Err(e) => self.error(e),
                }
            }
            AppEvent::MessagesLoaded {
                match_id,
                initial,
                result,
            } => {
                if self.chat_match_id.as_deref() != Some(&match_id) {
                    return;
                }
                self.chat_loading = false;
                self.loading_more = false;
                match result {
                    Ok(page) => {
                        self.has_more = page.next_page_token.is_some()
                            && page.next_page_token != self.page_token;
                        self.page_token = page.next_page_token;
                        for m in page.messages {
                            self.seen.insert(message_key(&match_id, &m));
                            merge_message(
                                &mut self.messages,
                                m,
                                self.own.as_ref().map(|p| p.id.as_str()),
                            );
                        }
                        sort_messages(&mut self.messages);
                        if initial {
                            self.msg_scroll = 0;
                        }
                        self.status = None;
                    }
                    Err(e) => self.error(e),
                }
            }
            AppEvent::MessageSent {
                match_id,
                local_id,
                result,
            } => {
                if self.removed_matches.contains(&match_id) {
                    return;
                }
                let active = self.chat_match_id.as_deref() == Some(&match_id);
                match result {
                    Ok(m) => {
                        let msgs = if active {
                            &mut self.messages
                        } else {
                            self.threads.entry(match_id.clone()).or_default()
                        };
                        msgs.retain(|c| c.m.id != local_id);
                        merge_message(msgs, m.clone(), None);
                        sort_messages(msgs);
                        self.last_msg.insert(match_id.clone(), m.message.clone());
                        self.seen.insert(message_key(&match_id, &m));
                        if let Some(mt) = self.matches.iter_mut().find(|mt| mt.id == match_id) {
                            mt.last_activity_date = m.time().map(|t| t.to_rfc3339());
                        }
                        self.sort_matches();
                        self.status = None;
                    }
                    Err(e) => {
                        let msgs = if active {
                            &mut self.messages
                        } else {
                            self.threads.entry(match_id.clone()).or_default()
                        };
                        if let Some(c) = msgs.iter_mut().find(|c| c.m.id == local_id) {
                            c.pending = false;
                            c.failed = true;
                        }
                        self.toasts.push(toast(
                            "Message not confirmed. Ctrl-R restores it for review.",
                        ));
                        self.error(e);
                    }
                }
            }
            AppEvent::ProfileLoaded { user_id, result } => {
                if self.profile.as_ref().is_none_or(|p| p.user_id != user_id) {
                    return;
                }
                let p = self.profile.as_mut().unwrap();
                p.loading = false;
                match result {
                    Ok(profile) => p.profile = Some(profile),
                    Err(e) => {
                        p.error = Some(e.to_string());
                        self.error(e);
                    }
                }
            }
            AppEvent::Updates(result) => match result {
                Ok(up) => {
                    self.status = None;
                    self.last_sync = Some(Instant::now());
                    self.apply_updates(up);
                }
                Err(e) => self.error(e),
            },
            AppEvent::RefreshDone(result) => {
                self.refreshing = false;
                match result {
                    Ok(res) => {
                        self.config.auth_token = Some(res.auth_token.clone());
                        if let Some(rt) = res.refresh_token.filter(|t| !t.is_empty()) {
                            self.config.refresh_token = Some(rt);
                        }
                        self.api.apply_auth(
                            res.auth_token,
                            self.config.device_id.clone().unwrap_or_default(),
                            self.config.refresh_token.clone(),
                        );
                        self.save_config();
                        self.status = None;
                        if self.own.is_none() || self.screen == Screen::Login {
                            let api = self.api.clone();
                            self.spawn(
                                async move { AppEvent::LoginDone(api.get_own_user().await) },
                            );
                        } else {
                            self.start_poller();
                            self.load_matches();
                            if self.screen == Screen::Discover {
                                self.load_discovery();
                            }
                        }
                    }
                    Err(e) => self.go_login(&format!("Could not renew session: {e}")),
                }
            }
            AppEvent::LoginDone(result) => {
                self.login.busy = false;
                match result {
                    Ok(own) => self.finish_login(own),
                    Err(e) => {
                        if e.is_auth()
                            && self.config.refresh_token.is_some()
                            && !self.login_refreshed
                        {
                            self.login.busy = true;
                            self.start_refresh();
                        } else {
                            self.login.error = Some(e.to_string());
                        }
                    }
                }
            }
            AppEvent::BrowserLoginDone(result) => {
                self.login.busy = false;
                self.login.browser_waiting = false;
                match result {
                    Ok((token, device, refresh, own)) => {
                        self.config.auth_token = Some(token.clone());
                        self.config.device_id = Some(device.clone());
                        self.config.refresh_token = refresh.clone();
                        self.api.apply_auth(token, device, refresh);
                        self.toasts
                            .push(toast(&format!("Connected as {} via browser!", own.name)));
                        self.finish_login(own);
                    }
                    Err(err) => {
                        self.login.error = Some(err);
                        self.status = None;
                    }
                }
            }
            AppEvent::DiscoveryLoaded(result) => {
                self.discovery_loading = false;
                match result {
                    Ok(recs) => {
                        self.discovery = recs
                            .into_iter()
                            .filter(|r| !self.swiped.contains(&r.user.id))
                            .collect();
                        self.discovery_photo = 0;
                        self.discovery_scroll = 0;
                        self.discovery_error = None;
                    }
                    Err(e) => {
                        self.discovery_error = Some(e.to_string());
                        self.error(e);
                    }
                }
            }
            AppEvent::SwipeDone {
                user_id,
                like,
                result,
            } => {
                self.swiping = false;
                match result {
                    Ok(result) => {
                        self.swiped.insert(user_id.clone());
                        self.discovery.retain(|r| r.user.id != user_id);
                        self.discovery_photo = 0;
                        self.discovery_scroll = 0;
                        if let Some(n) = result.likes_remaining {
                            self.likes_remaining = Some(n);
                        }
                        if result.matched {
                            self.toasts
                                .push(toast("It's a match! Your conversation is in the inbox."));
                            if let Some(m) = result.new_match.filter(|m| !m.id.is_empty()) {
                                self.apply_updates(UpdateResponse {
                                    matches: vec![m],
                                    ..Default::default()
                                });
                            }
                            self.load_matches();
                        } else {
                            self.toasts
                                .push(toast(if like { "Liked" } else { "Passed" }));
                        }
                        if self.discovery.is_empty() {
                            self.load_discovery();
                        }
                    }
                    Err(e) => {
                        self.discovery_error = Some(format!("Swipe not confirmed: {e}"));
                        self.error(e);
                    }
                }
            }
            AppEvent::DiscoveryRevision { revision, result } => {
                if revision == self.discovery_revision {
                    self.handle_event(AppEvent::DiscoveryLoaded(result));
                }
            }
            AppEvent::SuperLiked { id, result } => {
                self.premium_busy = false;
                self.premium = None;
                match result {
                    Ok((swipe, remaining)) => {
                        self.super_likes_remaining = remaining;
                        self.handle_event(AppEvent::SwipeDone {
                            user_id: id,
                            like: true,
                            result: Ok(swipe),
                        });
                        self.toasts.push(toast("Super Like confirmed."));
                    }
                    Err(e) => {
                        self.swiping = false;
                        self.discovery_error = Some(format!(
                            "Super Like not confirmed. Check Tinder before retrying: {e}"
                        ));
                        self.error(e);
                    }
                }
            }
            AppEvent::Boosted(result) => {
                self.premium_busy = false;
                self.premium = None;
                match result {
                    Ok(remaining) => {
                        self.boosts_remaining = remaining;
                        self.toasts.push(toast("Boost activated."));
                    }
                    Err(e) => {
                        self.toasts
                            .push(toast("Boost not confirmed. Check Tinder before retrying."));
                        self.error(e);
                    }
                }
            }
            AppEvent::AccountLoaded(result) => {
                self.account.loading = false;
                match result {
                    Ok(user) => {
                        self.account.load(&user);
                        self.own = Some(user);
                    }
                    Err(e) => {
                        self.account.error = Some(e.to_string());
                        self.error(e);
                    }
                }
            }
            AppEvent::AccountSaved { update, result } => {
                self.account.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(own) = &mut self.own {
                            update.apply(own);
                            self.account.load(own);
                        }
                        self.toasts.push(toast("Account changes saved to Tinder."));
                        self.discovery_revision += 1;
                        self.discovery_loading = false;
                        self.discovery.clear();
                    }
                    Err(e) => {
                        self.account.error = Some(format!(
                            "Save not confirmed. Check Tinder before retrying: {e}"
                        ));
                        self.error(e);
                    }
                }
            }
            AppEvent::Unmatched { id, result } => {
                self.unmatching = None;
                self.confirm_unmatch = None;
                match result {
                    Ok(()) => {
                        self.removed_matches.insert(id.clone());
                        self.matches.retain(|m| m.id != id);
                        self.config.pinned.remove(&id);
                        self.config.drafts.remove(&id);
                        self.config.read_at.remove(&id);
                        self.unread.remove(&id);
                        self.last_msg.remove(&id);
                        self.threads.remove(&id);
                        self.ensure_active_match();
                        if self
                            .profile
                            .as_ref()
                            .is_some_and(|p| p.from_match_id.as_deref() == Some(&id))
                        {
                            self.profile = None;
                            self.screen = Screen::Matches;
                        }
                        self.select_first_result();
                        self.save_config();
                        self.toasts
                            .push(toast("Unmatched. The conversation has been removed."));
                    }
                    Err(e) => {
                        self.toasts
                            .push(toast("Unmatch not confirmed; refresh before retrying."));
                        self.error(e);
                    }
                }
            }
            AppEvent::Photo(ready) => {
                let _ = (&ready.key, &ready.img, &ready.error);
            }
            AppEvent::Notice(s) => self.toasts.push(toast(&s)),
        }
        self.request_visible_photos();
    }
    fn replace_matches(&mut self, matches: Vec<Match>) {
        let selected = self.matches.get(self.match_sel).map(|m| m.id.clone());
        self.matches = matches
            .into_iter()
            .filter(|m| {
                !m.closed && !m.dead && !m.id.is_empty() && !self.removed_matches.contains(&m.id)
            })
            .collect();
        for m in &self.matches {
            if let Some(last) = m.messages.iter().max_by_key(|m| m.time()) {
                self.last_msg.insert(m.id.clone(), last.message.clone());
                let read = self.config.read_at.get(&m.id).copied().unwrap_or(0);
                if last.from.as_deref() != self.own.as_ref().map(|p| p.id.as_str())
                    && last.time().is_some_and(|t| t.timestamp_millis() > read)
                {
                    self.unread.entry(m.id.clone()).or_insert(1);
                }
            }
            for msg in &m.messages {
                self.seen.insert(message_key(&m.id, msg));
            }
        }
        self.sort_matches();
        if let Some(id) = selected
            && let Some(i) = self.matches.iter().position(|m| m.id == id)
        {
            self.match_sel = i;
        }
        self.ensure_active_match();
    }
    fn ensure_active_match(&mut self) {
        if self
            .chat_match_id
            .as_ref()
            .is_some_and(|id| !self.matches.iter().any(|m| &m.id == id))
        {
            self.chat_match_id = None;
            self.messages.clear();
            self.input.clear();
            self.cursor = 0;
            self.profile = None;
            if matches!(self.settings_back, Screen::Chat | Screen::Profile) {
                self.settings_back = Screen::Matches;
            }
            if matches!(self.screen, Screen::Chat | Screen::Profile) {
                self.screen = Screen::Matches;
            }
            self.toasts
                .push(toast("This conversation is no longer available."));
        }
    }
    fn apply_updates(&mut self, up: UpdateResponse) {
        if let Some(dt) = parse_datetime(up.last_activity_date.as_deref()) {
            let mut since = self.since.lock().unwrap();
            if dt > *since {
                *since = dt;
            }
        }
        self.matches.retain(|m| {
            !up.blocks.contains(&m.id)
                && !m.person.as_ref().is_some_and(|p| up.blocks.contains(&p.id))
        });
        for mut m in up.matches {
            if self.removed_matches.contains(&m.id) {
                continue;
            }
            m.messages.sort_by_key(Message::time);
            if m.closed || m.dead {
                self.matches.retain(|old| old.id != m.id);
                continue;
            }
            let mid = m.id.clone();
            if mid.is_empty() {
                continue;
            }
            let active = self.screen == Screen::Chat && self.chat_match_id.as_ref() == Some(&mid);
            for msg in &m.messages {
                let key = message_key(&mid, msg);
                if !self.seen.insert(key) {
                    continue;
                }
                self.last_msg.insert(mid.clone(), msg.message.clone());
                if self.chat_match_id.as_ref() == Some(&mid) {
                    merge_message(
                        &mut self.messages,
                        msg.clone(),
                        self.own.as_ref().map(|p| p.id.as_str()),
                    );
                } else if let Some(thread) = self.threads.get_mut(&mid) {
                    merge_message(
                        thread,
                        msg.clone(),
                        self.own.as_ref().map(|p| p.id.as_str()),
                    );
                }
                if !active && msg.from.as_deref() != self.own.as_ref().map(|p| p.id.as_str()) {
                    *self.unread.entry(mid.clone()).or_default() += 1;
                }
            }
            if active {
                self.mark_read(&mid);
            }
            if let Some(old) = self.matches.iter_mut().find(|old| old.id == mid) {
                if m.person.is_some() {
                    old.person = m.person.take();
                }
                if m.last_activity_date.is_some() {
                    old.last_activity_date = m.last_activity_date;
                }
                old.message_count = old.message_count.max(m.message_count);
            } else if m.person.is_some() {
                m.is_new_match = true;
                self.matches.push(m);
            } else {
                self.load_matches();
            }
        }
        sort_messages(&mut self.messages);
        self.sort_matches();
        self.ensure_active_match();
    }
    fn sort_matches(&mut self) {
        let id = self.matches.get(self.match_sel).map(|m| m.id.clone());
        self.matches
            .sort_by_key(|m| std::cmp::Reverse(m.last_activity()));
        self.match_sel = id
            .and_then(|id| self.matches.iter().position(|m| m.id == id))
            .unwrap_or(0)
            .min(self.matches.len().saturating_sub(1));
    }
    pub fn request_visible_photos(&self) {
        if !self.config.image_enabled {
            return;
        }
        if matches!(self.screen, Screen::Matches | Screen::Chat) {
            for m in self
                .matches
                .iter()
                .skip(self.match_sel.saturating_sub(4))
                .take(12)
            {
                if let Some(url) = m
                    .person
                    .as_ref()
                    .and_then(|p| p.first_photo())
                    .and_then(|p| p.display_url())
                {
                    self.photos.request(&url, SizeClass::Avatar);
                }
            }
            if let Some(url) = self
                .matches
                .get(self.match_sel)
                .and_then(|m| m.person.as_ref())
                .and_then(|p| p.first_photo())
                .and_then(|p| p.display_url())
            {
                self.photos.request(&url, SizeClass::Photo);
            }
        }
        let (profile, idx) = if self.screen == Screen::Discover {
            (
                self.discovery.front().map(|r| &r.user),
                self.discovery_photo,
            )
        } else {
            (
                self.profile.as_ref().and_then(|p| p.profile.as_ref()),
                self.profile.as_ref().map(|p| p.photo_idx).unwrap_or(0),
            )
        };
        if let Some(p) = profile {
            for photo in p.photos.iter().skip(idx).take(2) {
                if let Some(url) = photo.display_url() {
                    self.photos.request(&url, SizeClass::Photo);
                }
            }
        }
    }
    fn open_external_viewer(&mut self) {
        if !self.config.external_photo_viewer {
            self.toasts
                .push(toast("Enable the external photo viewer in settings first."));
            return;
        }
        let url = self
            .profile
            .as_ref()
            .and_then(|p| p.profile.as_ref().and_then(|u| u.photos.get(p.photo_idx)))
            .and_then(|p| p.display_url());
        let Some(url) = url else {
            return;
        };
        let api = self.api.clone();
        self.spawn(async move {
            let result = async {
                let bytes = api.download_photo(&url).await.map_err(|e| e.to_string())?;
                tokio::task::spawn_blocking(move || {
                    let ext = image::guess_format(&bytes)
                        .map_err(|e| e.to_string())?
                        .extensions_str()[0];
                    let dir = Config::photo_cache_dir().join("viewer");
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let path = dir.join(format!("{}.{}", uuid::Uuid::new_v4(), ext));
                    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                    crate::platform::open(path).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())?
            }
            .await;
            AppEvent::Notice(result.map(|_| "Opened photo".into()).unwrap_or_else(|e| e))
        });
    }
    pub fn tick(&mut self) {
        self.toasts.retain(|t| t.until > Instant::now());
        if self.screen == Screen::Chat
            && let Some(id) = &self.chat_match_id
        {
            let previous = self.config.drafts.get(id).map(String::as_str).unwrap_or("");
            if previous != self.input {
                self.config.drafts.insert(id.clone(), self.input.clone());
                self.draft_changed = Some(Instant::now());
            }
        }
        if self
            .draft_changed
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(750))
        {
            self.save_config();
        }
    }
}

fn previous_boundary(s: &str, cursor: usize) -> usize {
    s.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|i| *i < cursor)
        .last()
        .unwrap_or(0)
}
fn next_boundary(s: &str, cursor: usize) -> usize {
    s.grapheme_indices(true)
        .map(|(i, _)| i)
        .find(|i| *i > cursor)
        .unwrap_or(s.len())
}
fn message_key(mid: &str, m: &Message) -> String {
    if !m.id.is_empty() {
        format!("{mid}:{}", m.id)
    } else {
        format!(
            "{mid}:{}:{}:{}",
            m.timestamp,
            m.from.as_deref().unwrap_or(""),
            m.message
        )
    }
}

fn sort_messages(msgs: &mut [ChatMsg]) {
    msgs.sort_by_key(|c| c.m.time());
}
fn merge_message(msgs: &mut Vec<ChatMsg>, m: Message, own: Option<&str>) {
    if msgs.iter().any(|c| !m.id.is_empty() && c.m.id == m.id) {
        return;
    }
    if own.is_some()
        && m.from.as_deref() == own
        && let Some(c) = msgs
            .iter_mut()
            .find(|c| c.pending && c.m.message == m.message)
    {
        *c = ChatMsg {
            m,
            pending: false,
            failed: false,
        };
        return;
    }
    msgs.push(ChatMsg {
        m,
        pending: false,
        failed: false,
    });
}

#[cfg(test)]
mod tests;

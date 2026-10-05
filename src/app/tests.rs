use super::*;
use crate::mock::MockApi;
use ratatui::{Terminal, backend::TestBackend};

async fn fixture() -> (App, mpsc::UnboundedReceiver<AppEvent>) {
    let dir = std::env::temp_dir().join(format!("ttui-app-test-{}", uuid::Uuid::new_v4()));
    let config = Config {
        mock: true,
        image_enabled: false,
        storage_path: Some(dir.join("config.json")),
        ..Default::default()
    };
    let api: Arc<dyn TinderApi> = Arc::new(MockApi::new(&config));
    let (tx, rx) = mpsc::unbounded_channel();
    let (ptx, _) = mpsc::unbounded_channel();
    let photos = PhotoPipeline::new(api.clone(), dir.join("photos"), ptx);
    let mut app = App::new(config, api.clone(), photos, tx);
    app.handle_event(AppEvent::Startup(Ok((
        api.get_own_user().await.unwrap(),
        api.get_matches(100, false).await.unwrap(),
    ))));
    app.stop_poller();
    (app, rx)
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn control(app: &mut App, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
}
async fn pump_until(
    app: &mut App,
    rx: &mut mpsc::UnboundedReceiver<AppEvent>,
    done: impl Fn(&App) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !done(app) {
            app.handle_event(rx.recv().await.unwrap());
        }
    })
    .await
    .expect("app event timed out");
}
async fn chat(app: &mut App, rx: &mut mpsc::UnboundedReceiver<AppEvent>) {
    let id = app.matches[0].id.clone();
    app.open_chat(&id);
    pump_until(app, rx, |a| !a.chat_loading).await;
}
fn msg(id: &str, body: &str, from: &str) -> Message {
    Message {
        id: id.into(),
        message: body.into(),
        from: Some(from.into()),
        to: None,
        timestamp: Utc::now().timestamp_millis(),
        match_id: None,
        sent_date: None,
    }
}
fn frame(app: &App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buf = terminal.backend().buffer();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn ordinary_typing_and_paste_never_trigger_navigation_or_sends() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let count = app.messages.len();
    for c in "jklq? coffee?".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.input, "jklq? coffee?");
    assert_eq!(app.screen, Screen::Chat);
    assert!(!app.help);
    app.handle_paste("\nhello 日本語 🦀\n");
    assert!(app.input.ends_with("日本語 🦀\n"));
    assert_eq!(app.messages.len(), count);
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Delete);
    assert!(app.input.starts_with("klq?"));
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Backspace);
    key(&mut app, KeyCode::Backspace);
    assert!(app.input.ends_with("日本語 "));
    control(&mut app, 'u');
    assert!(app.input.is_empty());
    assert_eq!(app.cursor, 0);
}
#[tokio::test]
async fn drafts_survive_navigation_and_restart_configuration() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    app.handle_paste("question? 日本語");
    key(&mut app, KeyCode::Esc);
    let disk: Config =
        serde_json::from_slice(&std::fs::read(app.config.storage_path.as_ref().unwrap()).unwrap())
            .unwrap();
    assert_eq!(disk.drafts[&id], "question? 日本語");
    app.open_chat(&id);
    assert_eq!(app.input, "question? 日本語");
}
#[tokio::test]
async fn async_send_settles_after_changing_conversations() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let first = app.chat_match_id.clone().unwrap();
    app.handle_paste("q: a complete message?");
    app.send();
    let second = app.matches[1].id.clone();
    app.open_chat(&second);
    pump_until(&mut app, &mut rx, |a| {
        a.threads.get(&first).is_some_and(|ms| {
            ms.iter()
                .any(|m| m.m.message == "q: a complete message?" && !m.pending)
        })
    })
    .await;
    app.open_chat(&first);
    assert_eq!(
        app.messages
            .iter()
            .filter(|c| c.m.message == "q: a complete message?")
            .count(),
        1
    );
    assert!(!app.messages.last().unwrap().failed);
}
#[tokio::test]
async fn failed_send_is_explicit_and_does_not_erase_a_new_draft() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let mid = app.chat_match_id.clone().unwrap();
    app.messages.push(ChatMsg {
        m: msg("local-test", "delivery uncertain", "mock-own-user"),
        pending: true,
        failed: false,
    });
    app.handle_paste("next draft");
    app.handle_event(AppEvent::MessageSent {
        match_id: mid,
        local_id: "local-test".into(),
        result: Err(ApiError::Network("offline".into())),
    });
    assert!(app.messages.last().unwrap().failed);
    assert!(!app.messages.last().unwrap().pending);
    assert_eq!(app.input, "next draft");
    control(&mut app, 'r');
    assert_eq!(app.input, "next draft");
    control(&mut app, 'u');
    control(&mut app, 'r');
    assert_eq!(app.input, "delivery uncertain");
}
#[tokio::test]
async fn incoming_own_echo_and_send_ack_do_not_duplicate() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let mid = app.chat_match_id.clone().unwrap();
    let own = app.own_id().unwrap();
    let m = msg("server-new", "hello", &own);
    app.messages.push(ChatMsg {
        m: msg("local-test", "hello", &own),
        pending: true,
        failed: false,
    });
    let count = app.messages.len();
    app.handle_event(AppEvent::Updates(Ok(UpdateResponse {
        matches: vec![Match {
            id: mid.clone(),
            messages: vec![m.clone()],
            ..Default::default()
        }],
        ..Default::default()
    })));
    app.handle_event(AppEvent::MessageSent {
        match_id: mid,
        local_id: "local-test".into(),
        result: Ok(m),
    });
    assert_eq!(app.messages.len(), count);
    assert_eq!(
        app.messages
            .iter()
            .filter(|c| c.m.id == "server-new")
            .count(),
        1
    );
}
#[tokio::test]
async fn repeated_updates_do_not_inflate_unread_and_closed_chat_is_unread() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let mid = app.chat_match_id.clone().unwrap();
    key(&mut app, KeyCode::Esc);
    let update = UpdateResponse {
        matches: vec![Match {
            id: mid.clone(),
            messages: vec![msg("new", "hey", "other")],
            ..Default::default()
        }],
        ..Default::default()
    };
    app.handle_event(AppEvent::Updates(Ok(update.clone())));
    app.handle_event(AppEvent::Updates(Ok(update)));
    assert_eq!(app.unread[&mid], 1);
    app.open_chat(&mid);
    assert!(!app.unread.contains_key(&mid));
}
#[tokio::test]
async fn blocked_match_cannot_remain_sendable() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let mid = app.chat_match_id.clone().unwrap();
    app.handle_event(AppEvent::Updates(Ok(UpdateResponse {
        blocks: vec![mid.clone()],
        ..Default::default()
    })));
    assert!(!app.matches.iter().any(|m| m.id == mid));
    assert!(app.chat_match_id.is_none());
    assert_eq!(app.screen, Screen::Matches);
}
#[tokio::test]
async fn network_errors_keep_cached_inbox_and_selection() {
    let (mut app, _) = fixture().await;
    app.match_sel = 3;
    let selected = app.matches[3].id.clone();
    let count = app.matches.len();
    app.handle_event(AppEvent::MatchesLoaded(Err(ApiError::Network(
        "offline".into(),
    ))));
    assert_eq!(app.matches.len(), count);
    assert_eq!(app.matches[app.match_sel].id, selected);
    assert_eq!(app.screen, Screen::Matches);
}
#[tokio::test]
async fn auth_loss_renews_and_restarts_polling() {
    let (mut app, mut rx) = fixture().await;
    app.config.refresh_token = Some("mock-refresh".into());
    app.handle_event(AppEvent::Updates(Err(ApiError::Auth)));
    assert!(app.refreshing);
    pump_until(&mut app, &mut rx, |a| !a.refreshing).await;
    assert_eq!(app.config.auth_token.as_deref(), Some("mock-auth-token"));
    assert!(app.poller.is_some());
    app.stop_poller();
}
#[tokio::test]
async fn expired_session_without_refresh_returns_to_login() {
    let (mut app, _) = fixture().await;
    app.handle_event(AppEvent::Updates(Err(ApiError::Auth)));
    assert_eq!(app.screen, Screen::Login);
    assert!(app.poller.is_none());
    assert!(app.login.error.is_some());
}
#[tokio::test]
async fn sign_out_ignores_queued_work_and_clears_credentials() {
    let (mut app, _) = fixture().await;
    let old = app.generation;
    let own = app.own.clone().unwrap();
    let matches = app.matches.clone();
    app.config.auth_token = Some("secret".into());
    app.sign_out();
    app.handle_event(AppEvent::Session(
        old,
        Box::new(AppEvent::Startup(Ok((own, matches)))),
    ));
    assert!(app.own.is_none());
    assert!(app.matches.is_empty());
    assert_eq!(app.screen, Screen::Login);
    assert!(app.config.auth_token.is_none());
}

#[tokio::test]
async fn sign_out_save_failure_stays_visible_and_retry_clears_saved_session() {
    let (mut app, _) = fixture().await;
    let path = app.config.storage_path.clone().unwrap();
    let root = path.parent().unwrap().to_path_buf();
    app.config.auth_token = Some("synthetic-auth".into());
    app.config.refresh_token = Some("synthetic-refresh".into());
    app.config.device_id = Some("synthetic-device".into());
    app.config
        .drafts
        .insert("private-chat".into(), "Unsent draft".into());
    app.config.save().unwrap();
    let original_disk = std::fs::read(&path).unwrap();
    let generation = app.generation;
    let match_count = app.matches.len();
    // A directory cannot be replaced by a config file, on either supported OS.
    let blocked_path = root.join("blocked-config");
    std::fs::create_dir(&blocked_path).unwrap();
    app.config.storage_path = Some(blocked_path);
    app.sign_out();
    app.tick();
    assert!(app.confirm_signout);
    assert_eq!(app.generation, generation);
    assert_eq!(app.screen, Screen::Matches);
    assert_eq!(app.config.auth_token.as_deref(), Some("synthetic-auth"));
    assert_eq!(app.matches.len(), match_count);
    assert_eq!(std::fs::read(&path).unwrap(), original_disk);
    assert!(frame(&app, 120, 36).contains("Sign-out failed"));
    // The failure is a persistent modal, not an expiring toast.
    app.toasts.clear();
    app.tick();
    assert!(frame(&app, 120, 36).contains("Sign-out failed"));

    app.config.storage_path = Some(path.clone());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Login);
    assert!(app.signout_error.is_none());
    assert!(!app.confirm_signout);
    assert!(app.own.is_none());
    assert!(app.matches.is_empty());
    assert_eq!(app.generation, generation + 1);
    let saved: Config = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(saved.auth_token.is_none());
    assert!(saved.refresh_token.is_none());
    assert!(saved.device_id.is_none());
    assert!(saved.user_id.is_none());
    assert!(saved.drafts.is_empty());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn persisted_identity_switch_clears_cached_state_before_own_profile_exists() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let old_id = app.chat_match_id.clone().unwrap();
    app.handle_paste("Previous account draft");
    app.stash_chat();
    app.config.pinned.insert(old_id.clone());
    app.config.read_at.insert(old_id.clone(), 123);
    app.unread.insert(old_id.clone(), 2);
    app.own = None;
    app.adopt_identity(&UserProfile {
        id: "another-account".into(),
        name: "Another user".into(),
        ..Default::default()
    });
    assert_eq!(app.config.user_id.as_deref(), Some("another-account"));
    assert!(app.config.drafts.is_empty());
    assert!(app.config.pinned.is_empty());
    assert!(app.config.read_at.is_empty());
    assert!(app.matches.is_empty());
    assert!(app.threads.is_empty());
    assert!(app.messages.is_empty());
    assert!(app.unread.is_empty());
    assert!(app.chat_match_id.is_none());
    assert!(app.input.is_empty());
    assert_eq!(app.cursor, 0);
}

#[tokio::test]
async fn adopting_same_identity_retains_chat_and_account_state() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    app.handle_paste("Keep this draft");
    app.stash_chat();
    app.config.pinned.insert(id.clone());
    app.config.read_at.insert(id.clone(), 456);
    let message_count = app.messages.len();
    let match_count = app.matches.len();
    let mut own = app.own.clone().unwrap();
    own.name = "Updated display name".into();
    app.adopt_identity(&own);
    assert_eq!(
        app.config.user_name.as_deref(),
        Some("Updated display name")
    );
    assert_eq!(app.config.drafts[&id], "Keep this draft");
    assert!(app.config.pinned.contains(&id));
    assert_eq!(app.config.read_at[&id], 456);
    assert_eq!(app.matches.len(), match_count);
    assert_eq!(app.messages.len(), message_count);
    assert_eq!(app.threads[&id].len(), message_count);
    assert_eq!(app.chat_match_id.as_deref(), Some(id.as_str()));
    assert_eq!(app.input, "Keep this draft");
}

#[tokio::test]
async fn draft_save_debounces_changes_and_persists_after_quiet_period() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    let path = app.config.storage_path.clone().unwrap();
    app.config.save().unwrap();
    let read_saved =
        || -> Config { serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap() };
    app.handle_paste("First draft");
    app.tick();
    assert_eq!(app.config.drafts[&id], "First draft");
    assert_ne!(
        read_saved().drafts.get(&id).map(String::as_str),
        Some("First draft")
    );
    assert!(app.draft_changed.is_some());
    app.draft_changed = Some(Instant::now() - Duration::from_secs(1));
    app.handle_paste(" plus 日本語");
    app.tick();
    // A new edit resets even an already elapsed previous debounce deadline.
    assert_ne!(
        read_saved().drafts.get(&id).map(String::as_str),
        Some("First draft plus 日本語")
    );
    app.draft_changed = Some(Instant::now() - Duration::from_secs(1));
    app.tick();
    assert_eq!(read_saved().drafts[&id], "First draft plus 日本語");
    assert!(app.draft_changed.is_none());
    app.input.clear();
    app.cursor = 0;
    app.tick();
    app.draft_changed = Some(Instant::now() - Duration::from_secs(1));
    app.tick();
    assert_eq!(read_saved().drafts[&id], "");
    assert!(app.draft_changed.is_none());
}

#[tokio::test]
async fn load_more_matches_guards_reentry_and_caps_the_session_limit() {
    let (mut app, mut rx) = fixture().await;
    let configured_limit = app.config.match_count;
    let initial = app.inbox_limit;
    app.load_more_matches();
    assert_eq!(app.inbox_limit, initial + 100);
    assert!(app.matches_loading);
    app.load_more_matches();
    assert_eq!(app.inbox_limit, initial + 100);
    pump_until(&mut app, &mut rx, |a| !a.matches_loading).await;

    app.inbox_limit = 999;
    app.load_more_matches();
    assert_eq!(app.inbox_limit, 1000);
    pump_until(&mut app, &mut rx, |a| !a.matches_loading).await;
    for limit in [1000, 1001] {
        app.inbox_limit = limit;
        app.load_more_matches();
        assert_eq!(app.inbox_limit, limit);
        assert!(!app.matches_loading);
    }
    assert_eq!(app.config.match_count, configured_limit);
}
#[tokio::test]
async fn pagination_merges_and_stops_at_server_end() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let mid = app.chat_match_id.clone().unwrap();
    let original = app.messages[0].m.clone();
    let mut older = msg("older", "oldest", "other");
    older.timestamp = 1;
    app.handle_event(AppEvent::MessagesLoaded {
        match_id: mid.clone(),
        initial: false,
        result: Ok(MessagesData {
            messages: vec![original.clone(), older],
            next_page_token: Some("opaque+/=".into()),
        }),
    });
    assert_eq!(app.messages[0].m.id, "older");
    assert_eq!(
        app.messages
            .iter()
            .filter(|m| m.m.id == original.id)
            .count(),
        1
    );
    assert!(app.has_more);
    app.handle_event(AppEvent::MessagesLoaded {
        match_id: mid,
        initial: false,
        result: Ok(MessagesData::default()),
    });
    assert!(!app.has_more);
}
#[tokio::test]
async fn discovery_like_pass_and_duplicate_key_guard() {
    let (mut app, mut rx) = fixture().await;
    app.open_discovery();
    pump_until(&mut app, &mut rx, |a| !a.discovery_loading).await;
    let count = app.discovery.len();
    let first = app.discovery.front().unwrap().user.id.clone();
    let matches = app.matches.len();
    app.swipe(true);
    app.swipe(true);
    assert!(app.swiping);
    pump_until(&mut app, &mut rx, |a| !a.swiping).await;
    assert_eq!(app.discovery.len(), count - 1);
    assert!(app.swiped.contains(&first));
    assert_eq!(app.matches.len(), matches + 1);
    app.swipe(false);
    pump_until(&mut app, &mut rx, |a| !a.swiping).await;
    assert_eq!(app.discovery.len(), count - 2);
}
#[tokio::test]
async fn rejected_swipe_keeps_the_card() {
    let (mut app, mut rx) = fixture().await;
    app.open_discovery();
    pump_until(&mut app, &mut rx, |a| !a.discovery_loading).await;
    let id = app.discovery.front().unwrap().user.id.clone();
    let count = app.discovery.len();
    app.swiping = true;
    app.handle_event(AppEvent::SwipeDone {
        user_id: id.clone(),
        like: true,
        result: Err(ApiError::RateLimited),
    });
    assert!(!app.swiping);
    assert_eq!(app.discovery.len(), count);
    assert_eq!(app.discovery.front().unwrap().user.id, id);
    assert!(app.discovery_error.is_some());
}
#[tokio::test]
async fn inbox_selection_remains_visible_and_search_works() {
    let (mut app, _) = fixture().await;
    app.matches.truncate(3);
    key(&mut app, KeyCode::PageUp);
    assert_eq!(app.match_sel, 0);
    key(&mut app, KeyCode::End);
    let name = app
        .matches
        .last()
        .unwrap()
        .person
        .as_ref()
        .unwrap()
        .name
        .clone();
    assert!(frame(&app, 80, 24).contains(&name));
    key(&mut app, KeyCode::Char('/'));
    app.handle_paste(&name);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.filtered_matches().len(), 1);
    assert_eq!(
        app.matches[app.match_sel].person.as_ref().unwrap().name,
        name
    );
}
#[tokio::test]
async fn every_screen_and_overlay_handles_terminal_resizes() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    app.profile = Some(ProfileState {
        user_id: "a".into(),
        from_match_id: None,
        loading: false,
        profile: Some(app.api.get_user("mock-user-0").await.unwrap()),
        photo_idx: 0,
        info_scroll: 0,
        error: None,
    });
    app.discovery = app.api.get_recommendations().await.unwrap().into();
    app.login.auth_token = "do-not-display-auth".into();
    app.login.refresh_token = "do-not-display-refresh".into();
    for theme in [Theme::Dark, Theme::Light] {
        app.config.theme = theme;
        for screen in [
            Screen::Login,
            Screen::Matches,
            Screen::Chat,
            Screen::Profile,
            Screen::Discover,
            Screen::Settings,
        ] {
            app.screen = screen;
            for (w, h) in [
                (1, 1),
                (20, 5),
                (49, 15),
                (50, 16),
                (60, 20),
                (80, 24),
                (100, 30),
                (140, 45),
            ] {
                let s = frame(&app, w, h);
                assert!(!s.contains("do-not-display"));
                app.help = true;
                frame(&app, w, h);
                app.help = false;
                app.confirm_signout = true;
                frame(&app, w, h);
                app.confirm_signout = false;
            }
        }
    }
}

#[tokio::test]
async fn editing_removes_whole_emoji_and_combining_sequences() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    app.handle_paste("a👨‍👩‍👧‍👦e\u{301}");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.input, "a👨‍👩‍👧‍👦");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.input, "a");
}

fn click_action(app: &mut App, action: Action, w: u16, h: u16) {
    frame(app, w, h);
    let rect = app
        .hits
        .borrow()
        .iter()
        .find(|hit| hit.action == action)
        .unwrap_or_else(|| panic!("No visible mouse target for {action:?} at {w}×{h}"))
        .rect;
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    });
}

#[tokio::test]
async fn mouse_navigation_and_composer_preserve_unicode_and_drafts() {
    let (mut app, mut rx) = fixture().await;
    let id = app.matches[0].id.clone();
    click_action(&mut app, Action::Chat(id.clone()), 100, 30);
    pump_until(&mut app, &mut rx, |a| !a.chat_loading).await;
    app.handle_paste("hi 日本語 👩‍💻");
    frame(&app, 100, 30);
    let rect = app
        .hits
        .borrow()
        .iter()
        .find_map(|h| match h.action {
            Action::Editor { rect, .. } => Some(rect),
            _ => None,
        })
        .unwrap();
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x + 4,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.cursor, 3); // The second cell of 日 must resolve to its start.
    key(&mut app, KeyCode::Char('!'));
    assert_eq!(app.input, "hi !日本語 👩‍💻");
    click_action(&mut app, Action::Navigate(Screen::Discover), 50, 16);
    assert_eq!(app.config.drafts[&id], "hi !日本語 👩‍💻");
    click_action(&mut app, Action::Navigate(Screen::Matches), 50, 16);
    click_action(&mut app, Action::Chat(id), 50, 16);
    assert_eq!(app.input, "hi !日本語 👩‍💻");
}

#[tokio::test]
async fn mouse_theme_previews_cancel_apply_and_do_not_click_through() {
    let (mut app, _) = fixture().await;
    click_action(&mut app, Action::Themes, 100, 30);
    click_action(&mut app, Action::Theme(2), 100, 30);
    assert_eq!(app.config.theme, Theme::Dracula);
    assert_eq!(app.screen, Screen::Matches);
    assert!(
        app.hits
            .borrow()
            .iter()
            .all(|h| !matches!(h.action, Action::Navigate(_) | Action::Chat(_)))
    );
    click_action(&mut app, Action::CancelTheme, 50, 16);
    assert_eq!(app.config.theme, Theme::Dark);
    app.open_themes();
    key(&mut app, KeyCode::End);
    let picked = app.config.theme;
    click_action(&mut app, Action::ApplyTheme, 50, 16);
    let saved: Config =
        serde_json::from_slice(&std::fs::read(app.config.storage_path.as_ref().unwrap()).unwrap())
            .unwrap();
    assert_eq!(saved.theme, picked);
    app.open_themes();
    key(&mut app, KeyCode::Home);
    app.shutdown();
    assert_eq!(app.config.theme, picked);
}

#[tokio::test]
async fn inbox_filters_pins_and_sort_keep_visible_selection() {
    let (mut app, _) = fixture().await;
    let id = app.matches[app.match_sel].id.clone();
    click_action(&mut app, Action::Pin, 50, 16);
    assert!(app.config.pinned.contains(&id));
    app.inbox_sort = InboxSort::Name;
    assert_eq!(app.matches[app.filtered_matches()[0]].id, id);
    app.inbox_filter = InboxFilter::Pinned;
    assert_eq!(app.filtered_matches().len(), 1);
    app.inbox_filter = InboxFilter::Drafts;
    app.config.drafts.insert(id.clone(), "later".into());
    assert_eq!(app.filtered_matches().len(), 1);
    app.inbox_filter = InboxFilter::Unread;
    app.activate(Action::MarkAllRead);
    assert!(app.filtered_matches().is_empty());
    app.inbox_filter = InboxFilter::All;
    app.search = "phrase in preview".into();
    app.last_msg
        .insert(id.clone(), "a phrase in preview".into());
    assert_eq!(app.filtered_matches().len(), 1);
    app.select_first_result();
    assert_eq!(app.matches[app.match_sel].id, id);
}

#[tokio::test]
async fn account_edits_validate_then_save_only_changed_fields() {
    let (mut app, mut rx) = fixture().await;
    click_action(&mut app, Action::Navigate(Screen::Account), 50, 16);
    pump_until(&mut app, &mut rx, |a| !a.account.loading).await;
    app.account.select(0);
    app.account
        .key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.handle_paste("New bio 🦀\nSecond line");
    assert!(app.account.dirty());
    assert_ne!(
        app.api.get_own_user().await.unwrap().bio,
        app.account.fields[0]
    );
    let update = serde_json::to_value(app.account.update().unwrap()).unwrap();
    assert_eq!(update.as_object().unwrap().len(), 1);
    app.account.fields[1] = "17".into();
    click_action(&mut app, Action::SaveAccount, 50, 16);
    assert!(!app.account.saving);
    assert!(app.account.error.as_ref().unwrap().contains("between"));
    app.account.fields[1] = "60".into();
    app.account.fields[2] = "50".into();
    app.save_account();
    assert!(app.account.error.as_ref().unwrap().contains("exceed"));
    app.account.fields[1] = "25".into();
    click_action(&mut app, Action::SaveAccount, 50, 16);
    assert!(app.account.saving);
    app.handle_paste("must not alter pending update");
    app.save_account();
    pump_until(&mut app, &mut rx, |a| !a.account.saving).await;
    assert!(!app.account.dirty());
    assert_eq!(
        app.api.get_own_user().await.unwrap().bio,
        "New bio 🦀\nSecond line"
    );
    assert_eq!(
        app.api.get_own_user().await.unwrap().age_filter_min,
        Some(25)
    );
    assert!(!app.account.fields[0].contains("must not"));
}

#[tokio::test]
async fn account_save_failure_preserves_edits_and_discards_stale_discovery() {
    let (mut app, _) = fixture().await;
    app.account.load(app.own.as_ref().unwrap());
    app.account.fields[0] = "keep this edit".into();
    let update = app.account.update().unwrap();
    app.handle_event(AppEvent::AccountSaved {
        update: update.clone(),
        result: Err(ApiError::Network("offline".into())),
    });
    assert!(app.account.dirty());
    assert_ne!(app.own.as_ref().unwrap().bio, "keep this edit");
    assert!(app.account.error.is_some());
    let revision = app.discovery_revision;
    app.handle_event(AppEvent::AccountSaved {
        update,
        result: Ok(()),
    });
    app.handle_event(AppEvent::DiscoveryRevision {
        revision,
        result: Ok(vec![Recommendation {
            user: UserProfile {
                id: "old-deck".into(),
                ..Default::default()
            },
            ..Default::default()
        }]),
    });
    assert!(app.discovery.is_empty());
    assert!(!app.account.dirty());
}

#[tokio::test]
async fn unmatch_requires_confirmation_and_late_responses_cannot_resurrect_it() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    let old = app.matches.clone();
    app.config.pinned.insert(id.clone());
    app.config.drafts.insert(id.clone(), "private draft".into());
    click_action(&mut app, Action::Unmatch, 50, 16);
    assert!(app.unmatching.is_none());
    click_action(&mut app, Action::CancelUnmatch, 50, 16);
    assert!(app.matches.iter().any(|m| m.id == id));
    click_action(&mut app, Action::Unmatch, 50, 16);
    click_action(&mut app, Action::ConfirmUnmatch, 50, 16);
    app.submit_unmatch();
    pump_until(&mut app, &mut rx, |a| a.unmatching.is_none()).await;
    assert_eq!(app.screen, Screen::Matches);
    assert!(!app.config.drafts.contains_key(&id));
    assert!(!app.config.pinned.contains(&id));
    assert!(
        !app.api
            .get_matches(100, false)
            .await
            .unwrap()
            .iter()
            .any(|m| m.id == id)
    );
    app.handle_event(AppEvent::MatchesLoaded(Ok(old.clone())));
    app.handle_event(AppEvent::Updates(Ok(UpdateResponse {
        matches: old,
        ..Default::default()
    })));
    assert!(!app.matches.iter().any(|m| m.id == id));
}

#[tokio::test]
async fn unmatch_failure_keeps_conversation_and_draft() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    app.input = "still here".into();
    app.handle_event(AppEvent::Unmatched {
        id: id.clone(),
        result: Err(ApiError::Other("rejected".into())),
    });
    assert!(app.matches.iter().any(|m| m.id == id));
    assert_eq!(app.screen, Screen::Chat);
    assert_eq!(app.input, "still here");
}

#[tokio::test]
async fn premium_actions_require_confirmation_and_block_duplicate_submits() {
    let (mut app, mut rx) = fixture().await;
    app.open_discovery();
    pump_until(&mut app, &mut rx, |a| !a.discovery_loading).await;
    let id = app.discovery.front().unwrap().user.id.clone();
    click_action(&mut app, Action::AskSuperLike, 50, 16);
    assert!(!app.swiping);
    click_action(&mut app, Action::CancelPremium, 50, 16);
    assert_eq!(app.discovery.front().unwrap().user.id, id);
    click_action(&mut app, Action::AskSuperLike, 50, 16);
    click_action(&mut app, Action::ConfirmPremium, 50, 16);
    app.submit_premium();
    pump_until(&mut app, &mut rx, |a| !a.premium_busy).await;
    assert_eq!(app.super_likes_remaining, Some(4));
    assert_ne!(app.discovery.front().unwrap().user.id, id);
    click_action(&mut app, Action::AskBoost, 50, 16);
    click_action(&mut app, Action::ConfirmPremium, 50, 16);
    app.submit_premium();
    pump_until(&mut app, &mut rx, |a| !a.premium_busy).await;
    assert_eq!(app.boosts_remaining, Some(1));
}

#[tokio::test]
async fn all_theme_and_account_frames_fit_and_modal_targets_are_isolated() {
    let (mut app, _) = fixture().await;
    app.account.load(app.own.as_ref().unwrap());
    app.screen = Screen::Account;
    for &theme in Theme::ALL {
        app.config.theme = theme;
        for (w, h) in [(50, 16), (80, 24), (120, 40)] {
            for focus in 0..account::LABELS.len() {
                app.account.select(focus);
                assert!(frame(&app, w, h).contains(account::LABELS[focus]));
                for hit in app.hits.borrow().iter() {
                    assert!(
                        hit.rect.right() <= w && hit.rect.bottom() <= h,
                        "Out-of-bounds target {:?}",
                        hit
                    );
                }
            }
        }
    }
    app.open_themes();
    frame(&app, 50, 16);
    assert!(app.hits.borrow().iter().all(|h| matches!(
        h.action,
        Action::Theme(_) | Action::ApplyTheme | Action::CancelTheme | Action::Scroll(_)
    )));
}

#[tokio::test]
async fn mouse_wheel_routes_to_hovered_pane_and_not_hidden_screen() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    frame(&app, 120, 35);
    let rect = app
        .hits
        .borrow()
        .iter()
        .find(|h| h.action == Action::Scroll(interaction::ScrollTarget::Inbox))
        .unwrap()
        .rect;
    let scroll = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::ScrollDown,
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    };
    let selected = app.match_sel;
    app.handle_mouse(scroll);
    assert_ne!(app.match_sel, selected);
    assert_eq!(app.msg_scroll, 0);
    app.help = true;
    frame(&app, 120, 35);
    let selected = app.match_sel;
    app.handle_mouse(scroll);
    assert_eq!(app.match_sel, selected);
    assert_eq!(app.help_scroll, 3);
}

#[tokio::test]
async fn background_saves_cannot_persist_an_unapplied_theme_preview() {
    let (mut app, _) = fixture().await;
    app.config.theme = Theme::Nord;
    app.open_themes();
    app.activate(Action::Theme(2));
    app.save_config();
    let saved: Config =
        serde_json::from_slice(&std::fs::read(app.config.storage_path.as_ref().unwrap()).unwrap())
            .unwrap();
    assert_eq!(saved.theme, Theme::Nord);
    assert_eq!(app.config.theme, Theme::Dracula);
}

#[tokio::test]
async fn reconnect_cancels_old_account_mutations_and_pending_callbacks() {
    let (mut app, _) = fixture().await;
    app.account.load(app.own.as_ref().unwrap());
    let generation = app.generation;
    app.account.fields[0] = "unsaved".into();
    let update = app.account.update().unwrap();
    app.go_login("expired");
    assert_eq!(app.screen, Screen::Login);
    app.handle_event(AppEvent::Session(
        generation,
        Box::new(AppEvent::AccountSaved {
            update,
            result: Ok(()),
        }),
    ));
    assert_ne!(app.own.as_ref().unwrap().bio, "unsaved");
    app.handle_event(AppEvent::LoginDone(Ok(UserProfile {
        id: "another-user".into(),
        ..Default::default()
    })));
    assert!(app.account.fields.is_empty());
    assert!(app.matches.is_empty());
    assert!(app.config.drafts.is_empty());
}

#[tokio::test]
async fn clickable_toolbars_remain_reachable_in_the_smallest_supported_window() {
    let (mut app, mut rx) = fixture().await;
    for screen in [
        Screen::Matches,
        Screen::Discover,
        Screen::Settings,
        Screen::Account,
    ] {
        app.screen = screen;
        let text = frame(&app, 50, 16);
        for label in [
            "[Discover]",
            "[Settings]",
            "[Account]",
            "[Help]",
            "[Themes]",
        ] {
            assert!(text.contains(label));
        }
        let expected = match screen {
            Screen::Matches => vec![
                Action::SelectedChat,
                Action::Filter,
                Action::Sort,
                Action::MarkAllRead,
            ],
            Screen::Discover => vec![Action::AskSuperLike, Action::AskBoost, Action::ReportHelp],
            Screen::Settings => vec![Action::Themes, Action::Key(KeyCode::Enter)],
            Screen::Account => vec![
                Action::SaveAccount,
                Action::DiscardAccount,
                Action::ReloadAccount,
                Action::Web,
            ],
            _ => unreachable!(),
        };
        for action in expected {
            assert!(
                app.hits.borrow().iter().any(|h| h.action == action),
                "Missing {action:?}"
            );
        }
    }
    chat(&mut app, &mut rx).await;
    frame(&app, 50, 16);
    for action in [
        Action::Send,
        Action::Profile,
        Action::Older,
        Action::Latest,
        Action::RestoreDraft,
        Action::Unmatch,
        Action::ReportHelp,
    ] {
        assert!(
            app.hits.borrow().iter().any(|h| h.action == action),
            "Missing {action:?}"
        );
    }
}

#[tokio::test]
async fn repeated_mouse_connect_and_chat_clicks_do_not_duplicate_requests() {
    let (mut app, mut rx) = fixture().await;
    chat(&mut app, &mut rx).await;
    let id = app.chat_match_id.clone().unwrap();
    let count = app.tasks.len();
    app.activate(Action::Chat(id));
    assert_eq!(app.tasks.len(), count);
    app.go_login("Reconnect");
    app.login.auth_token = "test-token".into();
    app.login.device_id = "test-device".into();
    click_action(&mut app, Action::Connect, 50, 16);
    let count = app.tasks.len();
    click_action(&mut app, Action::Connect, 50, 16);
    assert_eq!(app.tasks.len(), count);
}

#[tokio::test]
async fn browser_login_success_updates_config_and_transitions_to_matches() {
    let (mut app, _rx) = fixture().await;
    app.go_login("Session expired");
    assert_eq!(app.screen, Screen::Login);

    app.handle_event(AppEvent::BrowserLoginDone(Ok((
        "new-auth-token".into(),
        "new-device-id".into(),
        Some("new-refresh-token".into()),
        UserProfile {
            id: "user-123".into(),
            name: "Alex".into(),
            ..Default::default()
        },
    ))));

    assert_eq!(app.screen, Screen::Matches);
    assert_eq!(app.config.auth_token.as_deref(), Some("new-auth-token"));
    assert_eq!(app.config.device_id.as_deref(), Some("new-device-id"));
    assert_eq!(
        app.config.refresh_token.as_deref(),
        Some("new-refresh-token")
    );
    assert_eq!(app.config.user_id.as_deref(), Some("user-123"));
    assert_eq!(app.config.user_name.as_deref(), Some("Alex"));
    assert!(!app.login.busy);
    assert!(!app.login.browser_waiting);
}

#[tokio::test]
async fn browser_login_cancel_resets_waiting_state() {
    let (mut app, _rx) = fixture().await;
    app.go_login("Session expired");
    app.login.busy = true;
    app.login.browser_waiting = true;

    key(&mut app, KeyCode::Esc);
    assert!(!app.login.busy);
    assert!(!app.login.browser_waiting);
}

#[tokio::test]
async fn login_screen_has_browser_login_button() {
    let (mut app, _rx) = fixture().await;
    app.go_login("Please sign in");
    frame(&app, 80, 24);
    assert!(
        app.hits
            .borrow()
            .iter()
            .any(|h| h.action == Action::BrowserLogin),
        "Missing BrowserLogin button on Login screen"
    );
}

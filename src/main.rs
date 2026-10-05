mod api;
mod app;
pub mod browser;
mod cache;
mod config;
mod graphics;
mod images;
mod mock;
mod photos;
mod platform;
mod secrets;
mod text;
mod themes;

use std::io::{IsTerminal, Read, Write};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::ExecutableCommand;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use api::client::{RealApi, TinderApi};
use app::{App, AppEvent};
use config::Config;
use mock::MockApi;
use photos::{PhotoPipeline, PhotoReady};

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}
fn restore_terminal() {
    let mut out = std::io::stdout();
    let _ = out.execute(DisableMouseCapture);
    let _ = out.execute(DisableBracketedPaste);
    let _ = out.execute(Show);
    let _ = out.execute(LeaveAlternateScreen);
    let _ = out.flush();
    let _ = disable_raw_mode();
}

fn main() -> Result<()> {
    let mut mock = false;
    let mut check = false;
    let mut import = false;
    let mut browser_login = false;
    let mut doctor = false;
    let mut clear_cache = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--mock" | "--demo" => mock = true,
            "--live" => mock = false,
            "--check" => check = true,
            "--doctor" => doctor = true,
            "--clear-cache" => clear_cache = true,
            "--import-session" => import = true,
            "--login" | "--browser-login" => browser_login = true,
            "--version" | "-V" => {
                println!("ttui {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "T-TUI — a quieter place for your conversations\n\nUsage: ttui [--mock | --live]\n\n  --mock, --demo      Offline demo with separate settings\n  --live              Connect to your Tinder account (default)\n  --login, --browser-login  Open browser and automatically log in\n  --check             Check the saved session without sending messages or swipes\n  --import-session    Import a copied cURL request from stdin\n  --version           Show version\n\nF1 for keys. Ctrl-Q or Ctrl-C to quit.\nConfig: {}",
                    Config::config_path().display()
                );
                return Ok(());
            }
            _ => bail!("Unknown argument {arg:?}. Run ttui --help."),
        }
    }
    if doctor {
        println!(
            "T-TUI {} | {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        println!(
            "Interactive input/output: {}/{}",
            std::io::stdin().is_terminal(),
            std::io::stdout().is_terminal()
        );
        println!(
            "Credential storage: {}",
            if cfg!(windows) {
                "Windows current-user DPAPI"
            } else {
                "owner-readable config (0600)"
            }
        );
        println!("Photo cache: 256 MiB / 7 days. Use --clear-cache with T-TUI closed.");
        println!(
            "No account data read and no network requests sent. Use --check for account connectivity."
        );
        return Ok(());
    }
    if clear_cache {
        println!(
            "Removed {} cached photos.",
            cache::prune(&Config::photo_cache_dir(), true)?
        );
        return Ok(());
    }
    if !check
        && !import
        && !browser_login
        && (!std::io::stdin().is_terminal() || !std::io::stdout().is_terminal())
    {
        bail!("Open ttui in an interactive terminal. Run ttui --help for options.");
    }
    let mut config = Config::load(mock)?;
    if import {
        let mut input = String::new();
        std::io::stdin().take(262145).read_to_string(&mut input)?;
        if input.len() > 262144 {
            bail!("Session import is too large (maximum 256 KB).");
        }
        let mut login = app::login::LoginView::new(&Config::default());
        login.paste(&input);
        if login.error.is_some() || login.auth_token.is_empty() || login.device_id.is_empty() {
            bail!(
                "No session headers found. Copy a Tinder API request as cURL, then import again."
            );
        }
        config.auth_token = Some(login.auth_token);
        config.device_id = Some(login.device_id);
        config.refresh_token = (!login.refresh_token.is_empty()).then_some(login.refresh_token);
    }
    let api: Arc<dyn TinderApi> = if mock {
        Arc::new(MockApi::new(&config))
    } else {
        Arc::new(RealApi::new(&config)?)
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if browser_login {
        if mock {
            bail!("Browser login is unavailable in offline demo mode.");
        }
        println!("Opening browser to https://tinder.com...");
        let _ = browser::open_browser_to_tinder();
        println!("Waiting for Tinder login in your browser (press Ctrl-C to cancel)...");
        let baseline_ts = browser::extract_freshest_credentials()
            .map(|c| c.timestamp)
            .unwrap_or(0);
        let creds = rt.block_on(async {
            browser::poll_browser_login(api.clone(), baseline_ts, Duration::from_secs(180)).await
        });
        match creds {
            Ok((tok, dev, ref_tok, own)) => {
                config.auth_token = Some(tok.clone());
                config.device_id = Some(dev.clone());
                config.refresh_token = ref_tok.clone();
                config.set_identity(own.id.clone(), own.name.clone());
                config.save()?;
                api.apply_auth(tok, dev, ref_tok);
                println!(
                    "Successfully authenticated as {} (ID: {})!",
                    own.name, own.id
                );
                println!("Session saved to {}.", Config::config_path().display());
                if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                    return Ok(());
                }
                println!("Launching T-TUI...");
            }
            Err(e) => bail!("Browser login failed: {e}"),
        }
    }
    if import {
        return rt.block_on(async move {
            let own = api.get_own_user().await?;
            config.set_identity(own.id, own.name);
            config.save()?;
            println!("Session verified and saved locally. Open ./ttui to continue.");
            Ok(())
        });
    }
    if check {
        return rt.block_on(async move {
            if !config.mock && !config.is_authenticated() && config.refresh_token.is_none() {
                bail!("No saved session. Connect in ./ttui first.");
            }
            let own = match api.get_own_user().await {
                Ok(own) => own,
                Err(e) if e.is_auth() && config.refresh_token.is_some() => {
                    let renewed = api
                        .refresh_token(config.refresh_token.as_deref().unwrap())
                        .await?;
                    config.auth_token = Some(renewed.auth_token);
                    if let Some(refresh) = renewed.refresh_token {
                        config.refresh_token = Some(refresh);
                    }
                    config.save()?;
                    api.get_own_user().await?
                }
                Err(e) => return Err(e.into()),
            };
            println!("Account connection: OK");
            let matches = api.get_matches(config.match_count, false).await?;
            println!("Inbox: OK ({} conversations)", matches.len());
            if let Some(m) = matches.first() {
                let page = api.get_messages(&m.id, 10, None).await?;
                println!("Message history: OK ({} messages)", page.messages.len());
                if let Some(cursor) = page.next_page_token {
                    let older = api.get_messages(&m.id, 10, Some(cursor)).await?;
                    println!("Older history: OK ({} messages)", older.messages.len());
                }
                if let Some(id) = m.other_id(&own.id) {
                    api.get_user(&id).await?;
                    println!("Profile details: OK");
                }
            }
            let recs = api.get_recommendations().await?;
            println!("Discovery: OK ({} profiles)", recs.len());
            api.get_updates(chrono::Utc::now()).await?;
            println!("Update sync: OK");
            if let Some(url) = recs
                .iter()
                .flat_map(|r| &r.user.photos)
                .find_map(|p| p.display_url())
            {
                let bytes = api.download_photo(&url).await?;
                if images::render(&bytes, 30, 18).is_none() {
                    bail!("Photo decode failed");
                }
                println!("Photo download and decode: OK");
            }
            println!("Read-only checks complete; no messages or swipes sent.");
            Ok(())
        });
    }
    rt.block_on(async move {
        let (photo_tx, mut photo_rx) = mpsc::unbounded_channel::<PhotoReady>();
        let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
        let photos = PhotoPipeline::new(api.clone(), Config::photo_cache_dir(), photo_tx);
        let fwd_tx = app_tx.clone();
        tokio::spawn(async move {
            while let Some(pr) = photo_rx.recv().await {
                if fwd_tx.send(AppEvent::Photo(pr)).is_err() {
                    break;
                }
            }
        });
        let mut app = App::new(config, api, photos, app_tx);
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous_hook(info);
        }));
        enable_raw_mode()?;
        let _guard = TerminalGuard;
        let mut stdout = std::io::stdout();
        stdout
            .execute(EnterAlternateScreen)?
            .execute(EnableBracketedPaste)?
            .execute(EnableMouseCapture)?
            .execute(Hide)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        // Capability queries must finish before the input pump starts reading stdin.
        app.photos.graphics.detect();
        let (key_tx, mut key_rx) = mpsc::channel::<std::io::Result<Event>>(128);
        // Blocking terminal reads never occupy an async runtime worker.
        let pump = std::thread::spawn(move || {
            while !key_tx.is_closed() {
                match event::poll(Duration::from_millis(80)) {
                    Ok(false) => continue,
                    Ok(true) => match event::read() {
                        Ok(ev) => {
                            if key_tx.blocking_send(Ok(ev)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = key_tx.blocking_send(Err(e));
                            break;
                        }
                    },
                    Err(e) => {
                        let _ = key_tx.blocking_send(Err(e));
                        break;
                    }
                }
            }
        });
        app.start();
        let result = run_loop(&mut terminal, &mut app, &mut key_rx, &mut app_rx).await;
        app.shutdown();
        drop(key_rx);
        let _ = pump.join();
        result
    })
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    key_rx: &mut mpsc::Receiver<std::io::Result<Event>>,
    app_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        app.tick();
        terminal.draw(|f| app::ui::draw(f, app))?;
        tokio::select! {
            ev = key_rx.recv() => match ev {
                Some(Ok(Event::Key(k))) if k.kind != KeyEventKind::Release => {
                    if app.handle_key(k) { return Ok(()); }
                }
                Some(Ok(Event::Mouse(ev))) => app.handle_mouse(ev),
                Some(Ok(Event::Paste(s))) => app.handle_paste(&s),
                Some(Ok(Event::Resize(_, _))) => app.photos.graphics.update_cell_size(),
                Some(Ok(_)) => {},
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            },
            ev = app_rx.recv() => match ev {
                Some(ev) => app.handle_event(ev),
                None => return Ok(()),
            },
            _ = tick.tick() => {},
        }
    }
}

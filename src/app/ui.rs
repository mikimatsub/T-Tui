//! A responsive interface with shared keyboard and mouse actions. Ratatui handles cell widths and clipping.
use super::interaction::{Action, ScrollTarget};
use super::{App, Screen};
use crate::themes::Palette;
use crate::{
    api::types::{Photo, UserProfile},
    config::Theme,
    photos::SizeClass,
    text,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn inset(r: Rect, x: u16, y: u16) -> Rect {
    Rect::new(
        r.x + x.min(r.width),
        r.y + y.min(r.height),
        r.width.saturating_sub(x * 2),
        r.height.saturating_sub(y * 2),
    )
}
fn row(r: Rect, y: u16) -> Rect {
    Rect::new(r.x, r.y + y.min(r.height), r.width, u16::from(y < r.height))
}
fn put(f: &mut Frame, r: Rect, line: impl Into<Line<'static>>, style: Style) {
    let mut line = line.into();
    let width = (line.width() as u16).min(r.width);
    let offset = match line.alignment {
        Some(ratatui::layout::Alignment::Right) => r.width.saturating_sub(width),
        Some(ratatui::layout::Alignment::Center) => r.width.saturating_sub(width) / 2,
        _ => 0,
    };
    line.alignment = None;
    f.render_widget(
        Paragraph::new(line).style(style),
        Rect::new(r.x + offset, r.y, width, r.height),
    );
}

fn panel(f: &mut Frame, r: Rect, title: &str, p: Palette, active: bool) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active { p.accent } else { p.line }))
        .style(Style::default().bg(p.panel))
        .title(
            Line::from(format!(" {title} ")).style(Style::default().fg(if active {
                p.accent
            } else {
                p.dim
            })),
        );
    let inner = block.inner(r);
    f.render_widget(block, r);
    inner
}
fn empty(f: &mut Frame, r: Rect, title: &str, detail: &str, p: Palette) {
    let y = r.height.saturating_sub(4) / 2;
    put(
        f,
        row(r, y),
        Line::from(title.to_owned()).centered(),
        p.accent(),
    );
    let rect = Rect::new(
        r.x + 2,
        r.y + y + 2,
        r.width.saturating_sub(4),
        r.height.saturating_sub(y + 2),
    );
    f.render_widget(
        Paragraph::new(detail)
            .style(p.dim())
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true }),
        rect,
    );
}
fn hint(app: &App) -> &'static str {
    match app.screen {
        Screen::Login => {
            "Ctrl-B browser login   Enter connect   Tab next field   Ctrl-U clear   F1 help   Ctrl-Q quit"
        }
        Screen::Matches if app.searching => {
            "Type a name   Enter finish search   Esc clear   ↑↓ select"
        }
        Screen::Matches => {
            "↑↓ select   Enter chat   → profile   / search   d discover   s settings   F1 help"
        }
        Screen::Chat => {
            "Enter send   Esc inbox   Ctrl-P profile   ↑↓ scroll   PgUp history   F1 help"
        }
        Screen::Profile => "←→ photos   ↑↓ read more   Enter chat   v open photo   Esc back",
        Screen::Discover => {
            "y like   n pass   ←→ photos   ↑↓ read more   m inbox   r refresh   F1 help"
        }
        Screen::Settings => "↑↓ select   Enter change   Esc back   F2 themes   F1 help",
        Screen::Account => "Tab next field   Ctrl-S save   Ctrl-U clear   Esc inbox",
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    app.hits.borrow_mut().clear();
    let p = Palette::new(app.config.theme);
    let area = f.area();
    f.render_widget(Block::default().style(p.base().bg(p.bg)), area);
    if area.width < 50 || area.height < 16 {
        f.render_widget(
            Paragraph::new("T-TUI\n\nMake the terminal at least 50 × 16.\nCtrl-Q to quit.")
                .style(p.base())
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inset(area, 1, 0));
    let head = chunks[0];
    let label = if app.config.mock {
        "DEMO · offline"
    } else if app.refreshing {
        "Reconnecting…"
    } else if app.screen == Screen::Login {
        "Sign in"
    } else if app.own.is_some() {
        "Connected"
    } else {
        "Welcome"
    };
    put(
        f,
        row(head, 0),
        Line::from(vec![
            Span::styled(" T–TUI ", p.accent()),
            Span::styled(" /  a little more connection", p.dim()),
        ]),
        p.base(),
    );
    let name = app
        .own
        .as_ref()
        .map(|u| format!("{}  ·  ", u.name))
        .unwrap_or_default();
    let badge = format!("{name}{label} ");
    let badge_width = badge.width() as u16;
    if head.width > 42 + badge_width {
        put(
            f,
            Rect::new(head.right() - badge_width, head.y, badge_width, 1),
            badge,
            Style::default().fg(p.good).bg(p.bg),
        );
    }
    if app.screen != Screen::Login {
        buttons(
            f,
            Rect::new(head.x, head.y + 1, head.width, 2),
            app,
            p,
            &[
                ("Discover".into(), Action::Navigate(Screen::Discover)),
                (
                    format!("Inbox {}", app.matches.len()),
                    Action::Navigate(Screen::Matches),
                ),
                ("Settings".into(), Action::Navigate(Screen::Settings)),
                ("Account".into(), Action::Navigate(Screen::Account)),
                ("Help".into(), Action::Help),
                ("Themes".into(), Action::Themes),
            ],
        );
    }
    let body = if app.screen == Screen::Login {
        chunks[1]
    } else {
        let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(chunks[1]);
        draw_actions(f, parts[1], app, p);
        parts[0]
    };
    match app.screen {
        Screen::Login => draw_login(f, body, app, p),
        Screen::Matches => draw_inbox(f, body, app, p),
        Screen::Chat => draw_chat(f, body, app, p),
        Screen::Profile => draw_profile(f, body, app, p),
        Screen::Discover => draw_discovery(f, body, app, p),
        Screen::Settings => draw_settings(f, body, app, p),
        Screen::Account => draw_account(f, body, app, p),
    }
    let status = app
        .toasts
        .last()
        .map(|t| t.text.clone())
        .or_else(|| app.status.clone())
        .unwrap_or_else(|| {
            if app.matches_loading {
                "Loading your conversations…".into()
            } else if app.refreshing {
                "Renewing your session…".into()
            } else if app.config.mock {
                "Offline demo · fictional profiles · your real account is untouched".into()
            } else if let Some(sync) = app.last_sync {
                format!("Synced {}s ago", sync.elapsed().as_secs())
            } else {
                "Your session stays on this device".into()
            }
        });
    put(
        f,
        row(chunks[2], 0),
        format!(" {status}"),
        if app.status.is_some() {
            p.base().fg(p.error)
        } else {
            p.dim()
        },
    );
    put(f, row(chunks[2], 1), format!(" {}", hint(app)), p.dim());
    if app.help {
        app.hits.borrow_mut().clear();
        draw_help(f, area, p, app.help_scroll);
        app.hit(area, Action::Scroll(ScrollTarget::Help));
        let r = centered(area, 80, 24);
        buttons(
            f,
            Rect::new(r.x + 2, r.bottom() - 2, r.width.saturating_sub(4), 1),
            app,
            p,
            &[
                ("Close".into(), Action::Help),
                ("↑".into(), Action::Key(crossterm::event::KeyCode::Up)),
                ("↓".into(), Action::Key(crossterm::event::KeyCode::Down)),
            ],
        );
    }
    if app.theme_original.is_some() {
        draw_themes(f, area, app, p);
    }
    if app.premium.is_some() {
        draw_premium(f, area, app, p);
    }
    if app.confirm_signout || app.confirm_unmatch.is_some() {
        app.hits.borrow_mut().clear();
        let unmatch = app.confirm_unmatch.as_ref();
        let r = centered(area, 64, 10);
        f.render_widget(Clear, r);
        let inner = panel(
            f,
            r,
            if unmatch.is_some() {
                "Unmatch"
            } else {
                "Sign out"
            },
            p,
            true,
        );
        let message = if let Some((_, name)) = unmatch {
            if app.unmatching.is_some() {
                "Removing this conversation…".into()
            } else {
                format!(
                    "Unmatch {name}?\n\nYou will disappear from each other's inbox. This cannot be undone."
                )
            }
        } else if let Some(error) = &app.signout_error {
            error.clone()
        } else {
            "Remove the saved session and local drafts?\n\nYour Tinder account and conversations stay intact.".into()
        };
        let body = inset(inner, 2, 0);
        f.render_widget(
            Paragraph::new(message)
                .style(p.base())
                .wrap(Wrap { trim: true }),
            Rect::new(body.x, body.y, body.width, body.height.saturating_sub(2)),
        );
        if app.unmatching.is_none() {
            buttons(
                f,
                Rect::new(body.x, body.bottom() - 1, body.width, 1),
                app,
                p,
                &[
                    (
                        if unmatch.is_some() {
                            "Unmatch"
                        } else {
                            "Sign out"
                        }
                        .into(),
                        if unmatch.is_some() {
                            Action::ConfirmUnmatch
                        } else {
                            Action::ConfirmSignout
                        },
                    ),
                    (
                        "Cancel".into(),
                        if unmatch.is_some() {
                            Action::CancelUnmatch
                        } else {
                            Action::CancelSignout
                        },
                    ),
                ],
            );
        }
    }
}

fn draw_login(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    let r = centered(area, 78, 23);
    let inner = panel(f, r, "Welcome to T-TUI", p, true);
    let a = inset(inner, 2, 0);
    let compact = a.height < 19;
    let intro = if compact {
        "Connect your Tinder session"
    } else {
        "Good conversations deserve a little space."
    };
    put(f, row(a, 0), intro, p.accent());
    if !compact {
        put(
            f,
            row(a, 2),
            "Paste a Tinder request copied as cURL, or enter each field.",
            p.dim(),
        );
    }
    let start = if compact { 2 } else { 4 };
    let stride = if compact { 2 } else { 3 };
    for (i, (label, value)) in [
        ("Auth token", &app.login.auth_token),
        ("Device ID", &app.login.device_id),
        ("Refresh token · optional", &app.login.refresh_token),
    ]
    .iter()
    .enumerate()
    {
        let y = start + i as u16 * stride;
        let focus = app.login.focus == i;
        app.hit(
            Rect::new(a.x, a.y + y, a.width, 2.min(a.height.saturating_sub(y))),
            Action::LoginField(i),
        );
        put(
            f,
            row(a, y),
            format!("{} {label}", if focus { "›" } else { " " }),
            if focus { p.accent() } else { p.dim() },
        );
        let shown = if value.is_empty() {
            if focus {
                "Paste here…".into()
            } else {
                "—".into()
            }
        } else if i != 1 {
            format!(
                "{}  {} characters",
                "•".repeat(value.chars().count().min(18)),
                value.chars().count()
            )
        } else {
            text::truncate(value, a.width.saturating_sub(3) as usize)
        };
        put(
            f,
            row(a, y + 1),
            format!("  {shown}"),
            Style::default()
                .fg(p.fg)
                .bg(if focus { p.selected } else { p.panel }),
        );
    }
    let after = start + 3 * stride;
    buttons(
        f,
        row(a, after),
        app,
        p,
        &[
            ("Browser Login".into(), Action::BrowserLogin),
            ("Connect".into(), Action::Connect),
            ("Clear field".into(), Action::ClearLogin),
            ("Help".into(), Action::Help),
            ("Themes".into(), Action::Themes),
        ],
    );
    let after = after + 1;
    let status = if app.login.browser_waiting {
        "Opening browser to https://tinder.com… Log in in your browser (Esc to cancel)".to_owned()
    } else if app.login.busy {
        "Connecting…".to_owned()
    } else if let Some(e) = &app.login.error {
        e.clone()
    } else {
        "Ctrl-B browser login   ·   Enter connect   ·   Tab next   ·   Ctrl-U clear field".into()
    };
    let height = a.height.saturating_sub(after);
    f.render_widget(
        Paragraph::new(status)
            .style(p.base().fg(if app.login.error.is_some() {
                p.error
            } else {
                p.accent
            }))
            .wrap(Wrap { trim: true }),
        Rect::new(a.x, a.y + after, a.width, height.min(3)),
    );
    if a.height > after + 3 {
        put(
            f,
            row(a, a.height - 1),
            "Setup guide: README.md   ·   Try it offline: ./ttui --mock",
            p.dim(),
        );
    }
}

fn draw_match_list(f: &mut Frame, area: Rect, app: &App, p: Palette, compact: bool) {
    let title = if app.searching {
        format!("Search loaded: {}▏", app.search)
    } else if !app.search.is_empty() {
        format!("Search loaded: {}", app.search)
    } else {
        format!(
            "Conversations · {} · {}",
            app.inbox_filter.label(),
            app.inbox_sort.label()
        )
    };
    let inner = panel(f, area, &title, p, app.screen == Screen::Matches);
    app.hit(inner, Action::Scroll(ScrollTarget::Inbox));
    app.hit(
        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1),
        Action::Search,
    );
    let indices = app.filtered_matches();
    if indices.is_empty() {
        empty(
            f,
            inner,
            if app.matches_loading {
                "Loading…"
            } else if !app.search.is_empty() {
                "No results"
            } else {
                "Your next hello awaits"
            },
            if !app.search.is_empty() {
                "Esc clears your search."
            } else {
                "Press d to discover people."
            },
            p,
        );
        return;
    }
    let row_h = if compact { 3 } else { 4 };
    let visible = (inner.height / row_h).max(1) as usize;
    let pos = indices
        .iter()
        .position(|i| *i == app.match_sel)
        .unwrap_or(0);
    let start = (pos / visible) * visible;
    for (offset, idx) in indices.iter().skip(start).take(visible).enumerate() {
        let m = &app.matches[*idx];
        let y = inner.y + offset as u16 * row_h;
        let r = Rect::new(
            inner.x,
            y,
            inner.width,
            row_h.min(inner.bottom().saturating_sub(y)),
        );
        app.hit(r, Action::Chat(m.id.clone()));
        let selected = *idx == app.match_sel;
        let bg = if selected { p.selected } else { p.panel };
        f.render_widget(Block::default().style(Style::default().bg(bg)), r);
        let has_avatar = !compact && app.config.image_enabled && r.width >= 30;
        let x = if has_avatar { 9 } else { 2 };
        if selected {
            put(f, Rect::new(r.x, r.y, 1, r.height), "▎", p.accent().bg(bg));
        }
        if has_avatar {
            let av = Rect::new(r.x + 1, r.y, 6, 3.min(r.height));
            photo(
                f,
                av,
                m.person.as_ref().and_then(|p| p.first_photo()),
                SizeClass::Avatar,
                app,
                p,
                false,
            );
        }
        let name = m
            .person
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "New connection".into());
        let name = if app.config.pinned.contains(&m.id) {
            format!("★ {name}")
        } else {
            name
        };
        let badge = match app.unread.get(&m.id) {
            Some(n) if *n > 0 => format!(" · {n}"),
            _ if m.is_new_match => " · new".into(),
            _ => String::new(),
        };
        let time = m
            .last_activity()
            .map(text::relative_time)
            .unwrap_or_default();
        let text_r = Rect::new(r.x + x, r.y, r.width.saturating_sub(x + 1), r.height);
        let name_w = text_r.width.saturating_sub(time.width() as u16 + 1) as usize;
        put(
            f,
            row(text_r, 0),
            text::truncate(&format!("{name}{badge}"), name_w),
            Style::default()
                .fg(if selected { p.accent } else { p.fg })
                .bg(bg)
                .bold(),
        );
        put(
            f,
            row(text_r, 0),
            Line::from(time).right_aligned(),
            Style::default().fg(p.dim).bg(bg),
        );
        let draft = app.config.drafts.get(&m.id).filter(|s| !s.is_empty());
        let preview = draft
            .map(|s| format!("Draft: {s}"))
            .or_else(|| app.last_msg.get(&m.id).cloned())
            .unwrap_or_else(|| "Say hello and start something.".into());
        put(
            f,
            row(text_r, 1),
            text::truncate(&preview, text_r.width as usize),
            Style::default()
                .fg(if draft.is_some() { p.accent } else { p.dim })
                .bg(bg),
        );
    }
    if indices.len() > visible {
        let counter = format!(
            " {}–{} / {} ",
            start + 1,
            (start + visible).min(indices.len()),
            indices.len()
        );
        put(
            f,
            Rect::new(
                area.x + 2,
                area.bottom() - 1,
                area.width.saturating_sub(4),
                1,
            ),
            Line::from(counter).right_aligned(),
            p.dim(),
        );
    }
}
fn draw_inbox(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    if area.width < 84 {
        draw_match_list(f, area, app, p, false);
        return;
    }
    let parts = Layout::horizontal([
        Constraint::Length((area.width * 42 / 100).clamp(34, 52)),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(area);
    draw_match_list(f, parts[0], app, p, false);
    let a = panel(f, parts[1], "A closer look", p, false);
    let Some(m) = app
        .matches
        .get(app.match_sel)
        .filter(|_| !app.filtered_matches().is_empty())
    else {
        empty(
            f,
            a,
            "Make room for a new connection",
            "Discover someone, then bring the conversation here.",
            p,
        );
        return;
    };
    let a = inset(a, 2, 1);
    let Some(person) = &m.person else {
        empty(
            f,
            a,
            "New connection",
            "Enter to open your conversation.",
            p,
        );
        return;
    };
    let ph = a.height.saturating_sub(9);
    let img = Rect::new(a.x, a.y, a.width, ph);
    photo(f, img, person.first_photo(), SizeClass::Photo, app, p, true);
    app.hit(img, Action::Profile);
    let title = format!(
        "{}{}",
        person.name,
        person.age().map(|n| format!(", {n}")).unwrap_or_default()
    );
    put(f, row(a, ph + 1), Line::from(title).centered(), p.accent());
    let bio = if person.bio.is_empty() {
        "A new conversation starts with a hello."
    } else {
        &person.bio
    };
    f.render_widget(
        Paragraph::new(bio)
            .style(p.base().bg(p.panel))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true }),
        Rect::new(
            a.x,
            a.y + ph + 3,
            a.width,
            3.min(a.height.saturating_sub(ph + 3)),
        ),
    );
    if a.height > 2 {
        put(
            f,
            row(a, a.height - 2),
            Line::from("Enter  start talking     →  full profile").centered(),
            p.accent(),
        );
    }
}

fn photo(
    f: &mut Frame,
    r: Rect,
    photo: Option<&Photo>,
    class: SizeClass,
    app: &App,
    p: Palette,
    label: bool,
) {
    if r.is_empty() {
        return;
    }
    f.render_widget(Block::default().style(Style::default().bg(p.bg)), r);
    // Native images are atomic graphics regions. Do not let a partially covered
    // image draw over a modal whose anchor lies outside the modal's rectangle.
    if app.modal_open() {
        return;
    }
    if !app.config.image_enabled {
        if label {
            empty(f, r, "Photos paused", "Enable them in settings.", p);
        }
        return;
    }
    let url = photo.and_then(|p| p.display_url());
    if let Some(url) = url {
        if let Some(img) = app.photos.get(&url, class)
            && app.photos.graphics.draw(
                &url,
                &img,
                f.buffer_mut(),
                r,
                p.bg,
                class == SizeClass::Avatar,
            )
        {
            return;
        }
        if label {
            empty(
                f,
                r,
                if app.photos.failed(&url, class) {
                    "Photo unavailable"
                } else {
                    "Loading photo…"
                },
                "",
                p,
            );
        }
    } else if label {
        empty(f, r, "No photo", "", p);
    }
}

#[derive(Clone)]
struct MessageLine {
    text: String,
    own: bool,
    meta: bool,
    failed: bool,
    day: bool,
}
fn message_lines(app: &App, width: usize) -> Vec<MessageLine> {
    let mut lines = vec![];
    let mut previous_day = String::new();
    let bubble_width = (width * 3 / 4).max(12).min(width.saturating_sub(2)).max(1);
    for c in &app.messages {
        let own = c.m.from.as_deref() == app.own.as_ref().map(|u| u.id.as_str());
        if let Some(t) = c.m.time() {
            let day = text::day_label(t);
            if day != previous_day {
                previous_day = day.clone();
                lines.push(MessageLine {
                    text: format!("─ {day} ─"),
                    own: false,
                    meta: true,
                    failed: false,
                    day: true,
                });
            }
        }
        for part in text::wrap(&c.m.message, bubble_width.saturating_sub(2).max(1)) {
            lines.push(MessageLine {
                text: format!(" {part} "),
                own,
                meta: false,
                failed: c.failed,
                day: false,
            });
        }
        let state = if c.pending {
            "Sending…".into()
        } else if c.failed {
            "Not confirmed · Ctrl-R to restore".into()
        } else {
            c.m.time().map(text::time_of_day).unwrap_or_default()
        };
        lines.push(MessageLine {
            text: state,
            own,
            meta: true,
            failed: c.failed,
            day: false,
        });
        lines.push(MessageLine {
            text: String::new(),
            own,
            meta: true,
            failed: false,
            day: false,
        });
    }
    lines
}
fn draw_chat(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    let main = if area.width >= 90 {
        let parts = Layout::horizontal([Constraint::Length(29), Constraint::Min(1)])
            .spacing(1)
            .split(area);
        draw_match_list(f, parts[0], app, p, true);
        parts[1]
    } else {
        area
    };
    let name = app
        .matches
        .iter()
        .find(|m| Some(&m.id) == app.chat_match_id.as_ref())
        .and_then(|m| m.person.as_ref())
        .map(|p| p.name.as_str())
        .unwrap_or("Conversation");
    let a = panel(f, main, &format!("{name}  ·  Ctrl-P profile"), p, false);
    app.hit(
        Rect::new(main.x + 1, main.y, main.width.saturating_sub(2), 1),
        Action::Profile,
    );
    let input_h = (text::wrap(&app.input, a.width.saturating_sub(6) as usize).len() as u16 + 2)
        .clamp(3, 6)
        .min(a.height.saturating_sub(3));
    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(input_h)]).split(a);
    let msg = inset(parts[0], 2, 0);
    app.hit(msg, Action::Scroll(ScrollTarget::Messages));
    if app.messages.is_empty() {
        empty(
            f,
            msg,
            if app.chat_loading {
                "Loading your conversation…"
            } else {
                "Start with a hello."
            },
            "Something in their profile caught your eye? Start there.",
            p,
        );
    } else {
        let lines = message_lines(app, msg.width as usize);
        let visible = msg.height as usize;
        let start = lines
            .len()
            .saturating_sub(visible)
            .saturating_sub(app.msg_scroll.min(lines.len().saturating_sub(visible)));
        for (i, l) in lines.iter().skip(start).take(visible).enumerate() {
            let fg = if l.failed {
                p.error
            } else if l.meta {
                p.dim
            } else {
                p.fg
            };
            let bg = if !l.meta && l.own { p.own } else { p.panel };
            let style = Style::default().fg(fg).bg(bg);
            let mut line = Line::from(l.text.clone());
            if l.day {
                line = line.centered();
            } else if l.own {
                line = line.right_aligned();
            }
            put(f, row(msg, i as u16), line, style);
        }
    }
    if app.loading_more || app.msg_scroll > 0 {
        let label = if app.loading_more {
            " Loading older messages… "
        } else {
            " History · ↓ or PgDn to return "
        };
        put(f, row(msg, 0), Line::from(label).centered(), p.accent());
    }
    let composer = panel(
        f,
        parts[1],
        if app.input.is_empty() {
            "Message"
        } else {
            "Draft"
        },
        p,
        true,
    );
    let input = inset(composer, 1, 0);
    if app.input.is_empty() {
        app.hit(
            input,
            Action::Editor {
                rect: input,
                start: 0,
            },
        );
        put(f, row(input, 0), "Write something…", p.dim());
        if !app.modal_open() {
            f.set_cursor_position((input.x, input.y));
        }
    } else {
        let (lines, cx, cy) = editor_lines(&app.input, app.cursor, input.width.max(1) as usize);
        let start = cy.saturating_sub(input.height.saturating_sub(1) as usize);
        app.hit(input, Action::Editor { rect: input, start });
        f.render_widget(
            Paragraph::new(
                lines
                    .into_iter()
                    .skip(start)
                    .map(Line::from)
                    .collect::<Vec<_>>(),
            )
            .style(p.base().bg(p.panel)),
            input,
        );
        if !app.modal_open() && input.height > 0 {
            f.set_cursor_position((
                input.x + (cx as u16).min(input.width.saturating_sub(1)),
                input.y + (cy - start) as u16,
            ));
        }
    }
}
fn editor_lines(input: &str, cursor: usize, width: usize) -> (Vec<String>, usize, usize) {
    let mut lines = vec![String::new()];
    let mut x = 0;
    let mut cx = 0;
    let mut cy = 0;
    for (idx, c) in input.grapheme_indices(true) {
        let w = c.width();
        if c != "\n" && x + w >= width && x > 0 {
            lines.push(String::new());
            x = 0;
        }
        if idx == cursor {
            cx = x;
            cy = lines.len() - 1;
        }
        if c == "\n" {
            lines.push(String::new());
            x = 0;
        } else {
            lines.last_mut().unwrap().push_str(c);
            x += w;
        }
    }
    if cursor >= input.len() {
        cx = x;
        cy = lines.len() - 1;
    }
    (lines, cx, cy)
}

fn draw_profile(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    if let Some(ps) = &app.profile {
        if let Some(profile) = &ps.profile {
            draw_person(f, area, profile, ps.photo_idx, ps.info_scroll, app, p);
        } else if let Some(error) = &ps.error {
            empty(
                f,
                area,
                "Profile unavailable",
                &format!("{error}\nPress r to retry or Esc to return."),
                p,
            );
        } else {
            empty(f, area, "A moment…", "Loading their profile.", p);
        }
    }
}
fn draw_discovery(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(area);
    if let Some(rec) = app.discovery.front() {
        draw_person(
            f,
            parts[0],
            &rec.user,
            app.discovery_photo,
            app.discovery_scroll,
            app,
            p,
        );
    } else {
        empty(
            f,
            parts[0],
            if app.discovery_loading {
                "Finding your next connection…"
            } else {
                "You're all caught up."
            },
            if app.discovery_loading {
                "Getting nearby profiles."
            } else {
                "Press r to check for more people, or m to open your inbox."
            },
            p,
        );
    }
    let text = if let Some(error) = &app.discovery_error {
        error.clone()
    } else if app.swiping {
        "Saving your choice…".into()
    } else {
        format!(
            "{} in this deck{}",
            app.discovery.len(),
            app.likes_remaining
                .map(|n| format!(" · {n} likes left"))
                .unwrap_or_default()
        )
    };
    f.render_widget(
        Paragraph::new(text)
            .style(if app.discovery_error.is_some() {
                p.base().fg(p.error)
            } else {
                p.accent()
            })
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true }),
        inset(parts[1], 1, 0),
    );
}
fn draw_person(
    f: &mut Frame,
    area: Rect,
    profile: &UserProfile,
    idx: usize,
    scroll: usize,
    app: &App,
    p: Palette,
) {
    let parts = Layout::horizontal([Constraint::Percentage(43), Constraint::Percentage(57)])
        .spacing(1)
        .split(area);
    let photo_title = format!(
        "Photos  {} / {}",
        if profile.photos.is_empty() {
            0
        } else {
            idx + 1
        },
        profile.photos.len()
    );
    let ph = panel(f, parts[0], &photo_title, p, false);
    photo(
        f,
        inset(ph, 1, 0),
        profile.photos.get(idx),
        SizeClass::Photo,
        app,
        p,
        true,
    );
    app.hit(
        Rect::new(ph.x, ph.y, ph.width / 2, ph.height),
        Action::Photo(false),
    );
    app.hit(
        Rect::new(
            ph.x + ph.width / 2,
            ph.y,
            ph.width - ph.width / 2,
            ph.height,
        ),
        Action::Photo(true),
    );
    buttons(
        f,
        Rect::new(
            parts[0].x + 1,
            parts[0].bottom() - 1,
            parts[0].width.saturating_sub(2),
            1,
        ),
        app,
        p,
        &[
            ("◀".into(), Action::Photo(false)),
            ("▶".into(), Action::Photo(true)),
        ],
    );
    let info = panel(f, parts[1], "About", p, false);
    app.hit(info, Action::Scroll(ScrollTarget::Details));
    let info = inset(info, 2, 0);
    let w = info.width as usize;
    let mut lines: Vec<Line<'static>> = vec![];
    let mut add = |s: String, style: Style| {
        for l in text::wrap(&s, w) {
            lines.push(Line::from(l).style(style));
        }
    };
    add(profile.summary(), p.accent());
    add(String::new(), p.dim());
    if let Some(intent) = &profile.relationship_intent {
        if let Some(title) = &intent.title_text {
            add(title.clone(), p.accent());
        }
        if let Some(body) = &intent.body_text {
            add(body.clone(), p.dim());
        }
        add(String::new(), p.dim());
    }
    add(
        if profile.bio.is_empty() {
            "A little mystery. Ask them about themselves.".into()
        } else {
            profile.bio.clone()
        },
        p.base(),
    );
    add(String::new(), p.dim());
    for job in &profile.jobs {
        let title = job
            .pointer("/title/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let company = job
            .pointer("/company/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        add(
            [title, company]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            p.dim(),
        );
    }
    for school in &profile.schools {
        if let Some(s) = school.get("name").and_then(|v| v.as_str()) {
            add(s.into(), p.dim());
        }
    }
    if !profile.selected_descriptors.is_empty() {
        add("THE LITTLE THINGS".into(), p.accent());
        add(
            profile
                .selected_descriptors
                .iter()
                .filter_map(|d| d.name.clone())
                .collect::<Vec<_>>()
                .join(" · "),
            p.dim(),
        );
        add(String::new(), p.dim());
    }
    if let Some(prompts) = &profile.user_prompts {
        for prompt in &prompts.prompts {
            if let Some(q) = &prompt.question_text {
                add(q.clone(), p.accent());
            }
            if let Some(a) = &prompt.answer_text {
                add(a.clone(), p.base());
            }
            add(String::new(), p.dim());
        }
    }
    let visible = info.height as usize;
    let start = scroll.min(lines.len().saturating_sub(visible));
    let more = lines.len() > visible;
    f.render_widget(
        Paragraph::new(lines.into_iter().skip(start).collect::<Vec<_>>())
            .style(p.base().bg(p.panel)),
        info,
    );
    if more {
        put(
            f,
            Rect::new(
                parts[1].x + 2,
                parts[1].bottom() - 1,
                parts[1].width.saturating_sub(4),
                1,
            ),
            Line::from(" Scroll to read more ").right_aligned(),
            p.dim(),
        );
    }
}

fn draw_settings(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    let r = centered(area, 86, 19);
    let a = panel(f, r, "Make it yours", p, false);
    let a = inset(a, 2, 0);
    let on = |b| {
        if b {
            "On".to_string()
        } else {
            "Off".to_string()
        }
    };
    let rows = vec![
        ("Theme", app.config.theme.name().into()),
        (
            "Session",
            if app.config.mock {
                "Demo · offline".into()
            } else {
                "Tinder · live".into()
            },
        ),
        (
            "In-terminal photos",
            if app.config.image_enabled {
                format!("On · {}", app.photos.graphics.label())
            } else {
                on(false)
            },
        ),
        (
            "External photo viewer",
            on(app.config.external_photo_viewer),
        ),
        (
            "Sync interval",
            format!("{} seconds", app.config.poll_interval_ms / 1000),
        ),
        ("Inbox size", format!("{} matches", app.config.match_count)),
        (
            "Text photo width",
            format!("{} cells", app.config.photo_width_cells),
        ),
        (
            "Text photo height",
            format!("{} cells", app.config.photo_height_cells),
        ),
        (
            "Text avatar detail",
            format!("{} cells", app.config.avatar_cells),
        ),
        ("Refresh inbox", "Enter".into()),
        ("Sign out", "Enter…".into()),
    ];
    app.hit(a, Action::Scroll(ScrollTarget::Settings));
    let visible = a.height.saturating_sub(3).max(1) as usize;
    let start = (app.settings.sel / visible) * visible;
    for (y, (i, (label, value))) in rows
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        let selected = app.settings.sel == i;
        let style = Style::default()
            .bg(if selected { p.selected } else { p.panel })
            .fg(if selected { p.accent } else { p.fg });
        let r = row(a, y as u16);
        app.hit(r, Action::Settings(i));
        f.render_widget(Block::default().style(style), r);
        put(
            f,
            r,
            format!("{} {label}", if selected { "›" } else { " " }),
            style,
        );
        put(f, r, Line::from(value.clone()).right_aligned(), style);
    }
    if a.height > 2 {
        put(
            f,
            row(a, a.height - 2),
            "Changes save automatically on this device.",
            p.dim(),
        );
    }
}
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
fn draw_help(f: &mut Frame, area: Rect, p: Palette, scroll: usize) {
    let r = centered(area, 80, 24);
    f.render_widget(Clear, r);
    let a = panel(f, r, "At your fingertips", p, true);
    let lines = vec![
        "EVERYWHERE",
        "F1 help · F2 themes · Ctrl-O account · Ctrl-Q quit · Esc back",
        "Click buttons, tabs and conversations; wheel scrolls the pane.",
        "Click or drag in the composer to position the cursor.",
        "Hold Shift for your terminal’s native text selection.",
        "",
        "DISCOVER",
        "y like · n pass · u Super Like · b Boost · w Tinder Web",
        "←→ photos · ↑↓ scroll · m inbox · r refresh",
        "",
        "INBOX",
        "j/k or ↑↓ select · Enter chat · → profile · / search",
        "d discover · s settings · r refresh · L load more (up to 1,000)",
        "f filter · o sort · p pin · a mark all read (local)",
        "",
        "CONVERSATION",
        "Type freely, including j, k, l, q and ? · Enter sends",
        "←→ / Home / End edit · Ctrl-U clear · Alt-Enter newline",
        "Ctrl-P profile · Ctrl-R restore failed draft · Ctrl-S settings",
        "Ctrl-X unmatch (confirmation required)",
        "↑↓ scroll · PgUp older history · PgDn latest · Esc saves draft",
        "",
        "PROFILE",
        "←→ photos · ↑↓ scroll · v external photo · Enter chat",
        "",
        "ACCOUNT",
        "Tab / Shift-Tab fields · Ctrl-S save · Ctrl-U clear field",
        "Edits stay local until Save; Discard restores loaded values.",
        "THEMES",
        "↑↓ preview · Enter apply · Esc cancel · wheel scroll",
        "Esc or F1 to close",
    ];
    let inner = inset(a, 2, 0);
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let wrapped: Vec<Line<'static>> = lines
        .into_iter()
        .flat_map(|s| text::wrap(s, body.width as usize))
        .map(Line::from)
        .collect();
    let start = scroll.min(wrapped.len().saturating_sub(body.height as usize));
    f.render_widget(
        Paragraph::new(wrapped.into_iter().skip(start).collect::<Vec<_>>()).style(p.base()),
        body,
    );
    put(
        f,
        row(inner, inner.height.saturating_sub(1)),
        "↑↓ scroll   Esc / F1 close",
        p.accent(),
    );
}

fn buttons(f: &mut Frame, area: Rect, app: &App, p: Palette, items: &[(String, Action)]) {
    let mut x = 0;
    let mut y = 0;
    for (label, action) in items {
        let label = format!("[{label}] ");
        let width = (label.width() as u16).min(area.width);
        if x + width > area.width {
            x = 0;
            y += 1;
        }
        if y >= area.height {
            break;
        }
        let r = Rect::new(area.x + x, area.y + y, width, 1);
        let active = matches!(action,Action::Navigate(screen) if *screen == app.screen || (*screen == Screen::Matches && matches!(app.screen,Screen::Chat | Screen::Profile)));
        put(
            f,
            r,
            label,
            if active {
                p.accent().bg(p.selected)
            } else {
                p.accent()
            },
        );
        app.hit(
            Rect::new(r.x, r.y, r.width.saturating_sub(1), r.height),
            action.clone(),
        );
        x += width;
    }
}
fn draw_actions(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    use crossterm::event::KeyCode;
    let back = ("Back".into(), Action::Key(KeyCode::Esc));
    let items: Vec<(String, Action)> = match app.screen {
        Screen::Matches => vec![
            ("Chat".into(), Action::SelectedChat),
            ("Profile".into(), Action::Profile),
            ("Find".into(), Action::Search),
            ("Clear".into(), Action::ClearSearch),
            (app.inbox_filter.label().into(), Action::Filter),
            (app.inbox_sort.label().into(), Action::Sort),
            ("Pin".into(), Action::Pin),
            ("Read all".into(), Action::MarkAllRead),
            ("Refresh".into(), Action::Key(KeyCode::Char('r'))),
            ("More".into(), Action::Key(KeyCode::Char('L'))),
        ],
        Screen::Chat => vec![
            back,
            ("Send".into(), Action::Send),
            ("Profile".into(), Action::Profile),
            ("Older".into(), Action::Older),
            ("Latest".into(), Action::Latest),
            ("Retry".into(), Action::RestoreDraft),
            ("Pin".into(), Action::Pin),
            ("Unmatch".into(), Action::Unmatch),
            ("Report help".into(), Action::ReportHelp),
        ],
        Screen::Profile => {
            let mut items = vec![
                back,
                ("◀".into(), Action::Photo(false)),
                ("▶".into(), Action::Photo(true)),
                ("↑".into(), Action::Key(KeyCode::Up)),
                ("↓".into(), Action::Key(KeyCode::Down)),
                ("Open photo".into(), Action::ExternalPhoto),
            ];
            if app
                .profile
                .as_ref()
                .is_some_and(|p| p.from_match_id.is_some())
            {
                items.push(("Chat".into(), Action::Key(KeyCode::Enter)));
                items.push(("Unmatch".into(), Action::Unmatch));
            }
            items.push(("Report help".into(), Action::ReportHelp));
            items
        }
        Screen::Discover => vec![
            ("n Pass".into(), Action::Key(KeyCode::Char('n'))),
            ("y Like".into(), Action::Key(KeyCode::Char('y'))),
            ("u Super Like".into(), Action::AskSuperLike),
            ("b Boost".into(), Action::AskBoost),
            ("◀".into(), Action::Photo(false)),
            ("▶".into(), Action::Photo(true)),
            ("↑".into(), Action::Key(KeyCode::Up)),
            ("↓".into(), Action::Key(KeyCode::Down)),
            ("Refresh".into(), Action::Key(KeyCode::Char('r'))),
            ("Report help".into(), Action::ReportHelp),
        ],
        Screen::Settings => vec![
            back,
            ("Themes".into(), Action::Themes),
            ("↑".into(), Action::Key(KeyCode::Up)),
            ("↓".into(), Action::Key(KeyCode::Down)),
            ("Change".into(), Action::Key(KeyCode::Enter)),
        ],
        Screen::Account => vec![
            ("Save".into(), Action::SaveAccount),
            ("Discard".into(), Action::DiscardAccount),
            ("Reload".into(), Action::ReloadAccount),
            ("Preview".into(), Action::OwnProfile),
            ("Tinder Web".into(), Action::Web),
            ("↑".into(), Action::Key(KeyCode::BackTab)),
            ("↓".into(), Action::Key(KeyCode::Tab)),
            back,
        ],
        Screen::Login => vec![],
    };
    buttons(f, area, app, p, &items);
}

fn draw_themes(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    app.hits.borrow_mut().clear();
    app.hit(area, Action::Scroll(ScrollTarget::Themes));
    let r = centered(area, 78, 29);
    f.render_widget(Clear, r);
    let inner = panel(f, r, "Themes · live preview", p, true);
    let a = inset(inner, 2, 0);
    put(
        f,
        row(a, 0),
        "↑↓ or click to preview · Enter apply · Esc cancel",
        p.dim(),
    );
    let visible = a.height.saturating_sub(4).max(1) as usize;
    let start = app.theme_sel.saturating_sub(visible - 1);
    for (y, (i, theme)) in Theme::ALL
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        let r = row(a, y as u16 + 2);
        let selected = i == app.theme_sel;
        let style = if selected {
            p.accent().bg(p.selected)
        } else {
            p.base().bg(p.panel)
        };
        f.render_widget(Block::default().style(style), r);
        put(
            f,
            r,
            format!("{} {}", if selected { "›" } else { " " }, theme.name()),
            style,
        );
        if r.width >= 42 {
            let colors = theme.palette();
            let line = Line::from(vec![
                Span::styled(" ● ", Style::default().fg(colors.accent)),
                Span::styled("● ", Style::default().fg(colors.good)),
                Span::styled("● ", Style::default().fg(colors.error)),
            ])
            .right_aligned();
            put(f, r, line, style);
        }
        app.hit(r, Action::Theme(i));
    }
    buttons(
        f,
        row(a, a.height.saturating_sub(1)),
        app,
        p,
        &[
            ("Apply".into(), Action::ApplyTheme),
            ("Cancel".into(), Action::CancelTheme),
            (
                format!("{}/{}", app.theme_sel + 1, Theme::ALL.len()),
                Action::Scroll(ScrollTarget::Themes),
            ),
        ],
    );
}

fn draw_account(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    let r = centered(area, 90, 28);
    let inner = panel(f, r, "My account · profile & discovery", p, true);
    let a = inset(inner, 2, 0);
    app.hit(a, Action::Scroll(ScrollTarget::Account));
    if app.account.fields.is_empty() {
        empty(
            f,
            a,
            if app.account.loading {
                "Loading your account…"
            } else {
                "Account unavailable"
            },
            app.account
                .error
                .as_deref()
                .unwrap_or("Use Reload to try again."),
            p,
        );
        return;
    }
    let status = if app.account.saving {
        "Saving to Tinder…"
    } else if app.account.loading {
        "Reloading…"
    } else if app.account.dirty() {
        "Unsaved changes · Save applies them to your account"
    } else {
        "Click a field to edit · Tab to move · Ctrl-S to save"
    };
    put(f, row(a, 0), status, p.dim());
    let body = Rect::new(a.x, a.y + 2, a.width, a.height.saturating_sub(4));
    // Keep the focused field completely visible, including at 50 × 16.
    let bio_height = 5usize.min(body.height as usize).max(1);
    let offsets: Vec<usize> = (0..super::account::LABELS.len())
        .map(|i| if i == 0 { 0 } else { bio_height + (i - 1) * 2 })
        .collect();
    let focus = app.account.focus;
    let focus_height = if focus == 0 { bio_height } else { 2 };
    let start = (offsets[focus] + focus_height).saturating_sub(body.height as usize);
    for (i, label) in super::account::LABELS.iter().enumerate() {
        let offset = offsets[i];
        let h = if i == 0 { bio_height } else { 2 };
        if offset < start || offset >= start + body.height as usize {
            continue;
        }
        let r = Rect::new(
            body.x,
            body.y + (offset - start) as u16,
            body.width,
            (h as u16).min(body.height - (offset - start) as u16),
        );
        let active = i == focus;
        let style = if active {
            p.accent().bg(p.selected)
        } else {
            p.base().bg(p.panel)
        };
        f.render_widget(Block::default().style(style), r);
        app.hit(r, Action::AccountField(i));
        if i == 0 {
            put(
                f,
                row(r, 0),
                format!(
                    "{} Bio · {}/500",
                    if active { "›" } else { " " },
                    app.account.fields[0].chars().count()
                ),
                style,
            );
            let edit = Rect::new(
                r.x + 1,
                r.y + 1,
                r.width.saturating_sub(2),
                r.height.saturating_sub(1),
            );
            let (lines, cx, cy) = editor_lines(
                &app.account.fields[0],
                app.account.cursor.min(app.account.fields[0].len()),
                edit.width.max(1) as usize,
            );
            let first = if active {
                cy.saturating_sub(edit.height.saturating_sub(1) as usize)
            } else {
                0
            };
            f.render_widget(
                Paragraph::new(
                    lines
                        .into_iter()
                        .skip(first)
                        .map(Line::from)
                        .collect::<Vec<_>>(),
                )
                .style(p.base().bg(if active { p.selected } else { p.panel })),
                edit,
            );
            app.hit(
                edit,
                Action::AccountCursor {
                    rect: edit,
                    start: first,
                },
            );
            if active
                && edit.height > 0
                && !app.account.saving
                && !app.help
                && app.theme_original.is_none()
            {
                f.set_cursor_position((
                    edit.x + (cx as u16).min(edit.width.saturating_sub(1)),
                    edit.y + (cy - first) as u16,
                ));
            }
        } else {
            let value = &app.account.fields[i];
            put(
                f,
                row(r, 0),
                format!("{} {label}", if active { "›" } else { " " }),
                style,
            );
            let value_x = (body.width / 2).max(22).min(body.width.saturating_sub(4));
            let value_area = if body.width < 60 {
                Rect::new(
                    r.x + 2,
                    r.y + 1,
                    r.width.saturating_sub(3),
                    u16::from(r.height > 1),
                )
            } else {
                Rect::new(r.x + value_x, r.y, r.width.saturating_sub(value_x), 1)
            };
            let shown = if value.is_empty() {
                "Not supplied".to_string()
            } else {
                value.clone()
            };
            put(f, value_area, shown, style);
            if active && i <= 3 && !app.account.saving && !app.help && app.theme_original.is_none()
            {
                f.set_cursor_position((
                    value_area.x
                        + (app.account.cursor as u16).min(value_area.width.saturating_sub(1)),
                    value_area.y,
                ));
            }
        }
    }
    if let Some(error) = &app.account.error {
        f.render_widget(
            Paragraph::new(error.as_str())
                .style(p.base().fg(p.error))
                .wrap(Wrap { trim: true }),
            Rect::new(a.x, a.bottom().saturating_sub(2), a.width, 2.min(a.height)),
        );
    }
}

fn draw_premium(f: &mut Frame, area: Rect, app: &App, p: Palette) {
    app.hits.borrow_mut().clear();
    let Some(action) = &app.premium else {
        return;
    };
    let r = centered(area, 64, 11);
    f.render_widget(Clear, r);
    let (title,detail) = match action {
        super::interaction::PremiumAction::SuperLike(rec) => ("Super Like",format!("Send a Super Like to {}? This uses one of your available Super Likes.",rec.user.name)),
        super::interaction::PremiumAction::Boost => ("Boost", "Activate Boost now? This consumes an available Boost and increases your profile's visibility.".into()),
    };
    let a = inset(panel(f, r, title, p, true), 2, 0);
    let text = if app.premium_busy {
        "Waiting for Tinder…".into()
    } else {
        format!("{detail}\n\nRequires account credits. This does not purchase any credits.")
    };
    f.render_widget(
        Paragraph::new(text)
            .style(p.base())
            .wrap(Wrap { trim: true }),
        Rect::new(a.x, a.y, a.width, a.height.saturating_sub(2)),
    );
    if !app.premium_busy {
        buttons(
            f,
            row(a, a.height.saturating_sub(1)),
            app,
            p,
            &[
                ("Confirm".into(), Action::ConfirmPremium),
                ("Cancel".into(), Action::CancelPremium),
            ],
        );
    }
}

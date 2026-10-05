# T-TUI status — 2026-09-18 — v0.3.0

## New in this upgrade

- 24 named palettes inspired by editor themes, including Dracula and all four
  Catppuccin variants. F2 or Themes opens a live preview picker; Apply persists
  the selection, Cancel restores it, and background saves cannot commit a
  temporary preview. Existing dark/light config files remain compatible.
- Mouse capture and clickable navigation/actions across login, inbox, chat,
  profiles, discovery, settings, account editing, help, and confirmation dialogs.
  Hit targets are recorded from the actual rendered rectangles and rebuilt on
  resize. The wheel routes to the pane beneath the pointer. Clicking/dragging
  in the composer places the cursor at Unicode grapheme boundaries.
- Inbox filters (All/Unread/New/Drafts/Pinned), Recent/Name/Newest sorting,
  persistent pins, search across names and message previews, and local mark-all-read.
- Account editor for bio, age range, distance in miles, gender preference,
  discovery visibility, and gender display. Only changed fields are submitted;
  edits require Save and can be discarded. Reload and own-profile preview are
  available. Unsaved account edits stay in memory while navigating.
- Unmatch with confirmation, acknowledgement handling, cleanup of local drafts
  and conversation state, and protection against stale responses resurrecting
  a removed conversation.
- Super Like and Boost with confirmation, duplicate-submit guards, demo credit
  accounting, server rejection handling, and no automatic purchases or retries.
- Browser handoffs for Tinder Web and official reporting instructions. Reporting
  instructions do not submit a report.
- Additional reconnect isolation, prevention of duplicate mouse-triggered login
  requests, and native image suppression beneath every modal.

## Verification

- `cargo test --locked` — 77 tests passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all --check` — passed.
- `cargo build --release --locked` — passed.
- `scripts/pty_e2e.py --binary target/release/ttui` — 25 terminal checks passed.
- Demo screenshots of the inbox, theme picker, account editor, and narrow-window
  account layout were visually inspected. Captures live under `/tmp/ttui-qa`.

The new API mutations are covered by local HTTP contracts and the offline demo;
they have not been exercised against the live account. No real messages, swipes,
unmatches, profile edits, or paid credits were sent/consumed during this upgrade.
Sixel is emulated in terminal regression tests; Kitty/iTerm2 were not visually
revalidated in this pass. The prior photo/session functionality remains covered
by the existing tests.

## Running

The release executable is installed at `~/.local/bin/ttui`. Restart the app to
use v0.3.0. Run `ttui --mock` for an isolated demo, press F2 for themes, and use
Account or Ctrl-O for profile/discovery editing. Rerun `./scripts/install.sh`
after source updates to refresh the installed executable.

This is not yet full Tinder Web parity. [FEATURES.md](FEATURES.md) records the
implemented workflows and the remaining gaps, including uploads, media messages,
Likes You/Top Picks/Explore, advanced premium features, purchases, verification,
and native reporting. [README.md](README.md) documents all controls and storage.

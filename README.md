# T-TUI

**Terminal client for Tinder.** Discover people, browse profiles, keep up with conversations, and edit your bio and discovery preferences with the keyboard or the mouse.

The session lives only on this machine. Tokens are masked and saved atomically: Unix uses owner-readable (`0600`) files; Windows protects credentials with current-user DPAPI. Drafts and settings remain readable JSON. T-TUI parses a pasted cURL command for headers and does not execute it. It does not bypass verification, buy credits, or submit reports.

Tinder can change these endpoints or send you back to the website to finish verification. Request shapes were cross-checked on 2026-09-18 against the [tinder-api client](https://github.com/Miguelo981/tinder-api/blob/main/src/tinder.ts). Local contract tests are not a substitute for checking a signed-in account. The supported surface and the remaining website-only workflows are in [FEATURES.md](FEATURES.md).

---

## Preparing 1.0

The current source is **1.0.0-rc.1**. Linux and native Windows are equal priorities. A stable release is not yet published. [CHANGELOG.md](CHANGELOG.md) records changes, [PLAN.md](PLAN.md) tracks execution, and [docs/RELEASING.md](docs/RELEASING.md) defines release gates.

GitHub Releases will provide standalone x86-64 binaries. `@mikimatsub/ttui` is the planned npm convenience package; WinGet is planned after a public stable release and Microsoft review. These are prepared channels, not claims of current availability. ARM and macOS are outside the 1.0 support promise.

## Quick start

You need:

- Rust and Cargo, to build the binary
- An interactive terminal. The layout works from 50 × 16 cells. 100 × 30 or larger shows the full split view
- For sharp photos, a terminal with native graphics, such as Foot (Sixel), Kitty, or iTerm2. Other terminals use a truecolor text fallback

### Install

From this repository:

```sh
./scripts/install.sh
```

On Windows, run `./scripts/install.ps1` in PowerShell with Rust and the MSVC C++ build tools installed. It installs to `%LOCALAPPDATA%\Programs\T-TUI`. Add that directory to your user PATH or launch `ttui.exe` directly. The script does not change PATH or require administrator privileges.

The installer builds a release binary and copies it to `~/.local/bin/ttui`. Set `TTUI_INSTALL_DIR` to use another directory. That directory must be on your `PATH`. The install needs no administrator privileges and no system package. Your saved account and settings stay in the config directory, so the installed command keeps working if you move or remove this repository. Rerun the installer after source updates.

### Run

```sh
ttui             # your account
ttui --mock      # offline demo with fictional profiles
```

To run from a checkout instead of the installed command:

```sh
./ttui           # your account
./ttui --mock    # offline demo
```

`./ttui` builds an optimized binary the first time you launch it.

### Verify it works

Offline, with no account and no network:

```sh
ttui --mock
```

The inbox opens with fictional profiles. Press `F1` for keys. Press `Ctrl-Q` to quit. In the demo, every Like becomes a match so you can walk the whole flow.

After a real session is saved, this check is read-only. It sends no messages and no swipes:

```sh
ttui --check
```

A healthy session prints `Account connection: OK`, then inbox, history, profile, discovery, sync, and photo lines, and ends with `Read-only checks complete; no messages or swipes sent.`

| Command | What it does |
|---|---|
| `ttui` | Open the saved account. This is the default. |
| `ttui --mock`, `ttui --demo` | Offline demo. Settings and drafts stay in `demo.json`. |
| `ttui --live` | Use the real account. Same as running `ttui` with no flag. |
| `ttui --login`, `ttui --browser-login` | Experimental Chromium extraction; ambiguous profiles require explicit selection. |
| `ttui --check` | Read-only check of the saved session. |
| `ttui --import-session` | Read one copied cURL request from stdin and save the session. |
| `ttui --version`, `ttui -V` | Print the installed version. |
| `ttui --doctor` | Local diagnostics without reading account data or contacting Tinder. |
| `ttui --clear-cache` | Remove managed cached photos; run with T-TUI closed. |
| `ttui --help`, `ttui -h` | Print usage and the config path. |

## Connect your account

### Browser login

This is experimental. Chromium storage layouts may change. Use manual session import if it fails. When multiple Tinder profiles are present, set `TTUI_BROWSER_PROFILE` to the intended absolute Chromium profile directory. T-TUI will not silently choose the newest account. Sign in on Tinder Web first, then try:

```sh
ttui --login
```

Or press **Ctrl-B** or click **[Browser Login]** on the connection screen. T-TUI opens Tinder, attempts extraction from one selected profile, and verifies the credentials before saving. Browser login is disabled in offline demo mode.

### Manual connection

1. Sign in to [Tinder Web](https://tinder.com).
2. Open developer tools (`F12`), select **Network**, and reload Tinder.
3. Choose a request to **api.gotinder.com**. Right-click it, then **Copy → Copy as cURL (bash)**.
4. Paste into the **Auth token** field with your terminal paste shortcut, usually `Ctrl-Shift-V`.
5. Press **Enter** to connect.

You can also paste `x-auth-token` and `persistent-device-id` from the request headers into their own fields. A refresh token in the third field renews the session. A refresh token and device ID are enough when the auth token is already expired.

Treat the copied request as a password. Paste it only into your own T-TUI. An expired session with no refresh token returns you to the connection screen.

On Linux you can import the clipboard without printing the request:

```sh
wl-paste --no-newline | ttui --import-session
ttui --check
```

## Everyday keys

| Where | Keys |
|---|---|
| Everywhere | `F1` help, `F2` themes, `Ctrl-O` account, `Ctrl-Q` / `Ctrl-C` quit |
| Inbox | `↑↓` or `j/k` select, `Enter` chat, `→` profile, `/` search, `Esc` clear search |
| Inbox tools | `f` filter, `o` sort, `p` pin or unpin, `a` mark all read on this device |
| Inbox | `d` discover, `s` settings, `r` refresh, `Home` / `End` first or last |
| Discovery | `y` like, `n` pass, `←→` photos, `↑↓` profile details, `m` inbox, `r` refresh |
| Discovery extras | `u` Super Like, `b` Boost (both ask for confirmation), `w` Tinder Web |
| Chat | Type normally, `Enter` send, `Alt-Enter` newline, `Esc` save the draft and return |
| Chat editing | `←→`, `Home` / `End`, `Backspace` / `Delete`, `Ctrl-U` clear |
| Chat tools | `Ctrl-P` profile, `Ctrl-S` settings, `Ctrl-R` restore a failed message for review |
| Chat safety | `Ctrl-X` unmatch (confirmation required). **Report help** opens Tinder's reporting instructions |
| Chat history | `↑↓` scroll, `PgUp` fetch older history, `PgDn` scroll toward the latest |
| Profile | `←→` photos, `↑↓` scroll, `Enter` chat, `v` external viewer, `Esc` back |
| Settings | `↑↓` select, `Enter` change, `Esc` return |
| Account | `Tab` / `Shift-Tab` choose a field, type to edit, `Ctrl-U` clear, `Ctrl-S` save, `Esc` inbox |
| Themes | `↑↓` or the mouse wheel to preview, `Enter` apply, `Esc` cancel |

Like and Pass run only from their buttons or from `y` and `n`. The arrow keys change photos. A card stays put when the server rejects the swipe. Further keypresses are ignored while a swipe is in flight.

Drafts save after 750 ms without changes, when you leave a conversation, or when you quit. Newlines in a bracketed paste stay in the draft and do not send. A message is first pending, then unconfirmed. If a send times out, delivery is uncertain: read the conversation before you send it again. Restoring a failed message puts the text back for review and does not send it.

## Mouse, photos, and themes

Click the top tabs, a conversation row, or any bracketed action. Click either half of a profile photo to move backward or forward. The wheel scrolls the pane under the pointer: inbox, history, profile details, settings, account fields, help, or themes. Click or drag in the composer to place the cursor. Unicode characters stay intact. A dialog captures clicks so they cannot reach the screen under it. Mouse capture is restored on exit. Hold **Shift** for the terminal's own text selection where the terminal supports it.

On Unix, T-TUI detects Sixel, Kitty, or iTerm2 graphics and the terminal's pixel size at startup. Photos keep their aspect ratio. Avatars fill their tiles. Resizing runs in background workers. **Settings → In-terminal photos** shows the detected renderer. The three **Text** size settings affect only the text fallback. Native photos fit the space they are given. Rendering uses [ratatui-image](https://github.com/ratatui/ratatui-image).

Open **Themes** or press **F2**. **Apply** saves the palette. **Cancel** restores the previous one. A saved dark or light choice from an older config still loads.

Included palettes: T-TUI Dark (the default), T-TUI Light, Dracula, Catppuccin Mocha, Macchiato, Frappe, and Latte, Nord, Gruvbox Dark and Light, Tokyo Night, Storm, and Day, One Dark and Light, Solarized Dark and Light, Rose Pine, Moon, and Dawn, Monokai, Material Ocean, and GitHub Dark and Light. These are T-TUI adaptations of the editor themes.

Inbox tools are All, Unread, New, Drafts, and Pinned filters, Recent, Name, and Newest sorting, search across names and message previews, persistent pins, and local read markers. Pinned conversations sort first. Filters and search apply only to loaded conversations. **More** or uppercase **L** expands the session limit by 100, up to 1,000. It refetches the expanded inbox through existing server pagination. Pins and read markers stay on this device.

## Configuration

| Name | Default | Notes |
|---|---|---|
| `TTUI_INSTALL_DIR` | `~/.local/bin` | Destination used by `scripts/install.sh`. |
| Account file | `$XDG_CONFIG_HOME/ttui/config.json` | Usually `~/.config/ttui/config.json`. Mode `0600`. Holds the session, drafts, pins, read markers, and settings. |
| Demo file | `demo.json` in that same directory | Separate settings and drafts for `--mock`. |
| Photo cache | `$XDG_CACHE_HOME/ttui/photos` | Usually `~/.cache/ttui/photos`. |
| Inbox size | 60 | Initial load 20–100; **More** increases it up to 1,000 for this session. |
| Poll interval | 5000 ms | Backs off after failures. |
| In-terminal photos | On | Turn off to force the text fallback. |
| Text photo size | 46 × 26 cells, avatars 10 | Used only by the text fallback. |
| External viewer | Off | `v` uses the Windows shell or Linux `xdg-open` after you enable it. |
| Theme | T-TUI Dark | Saved when you apply a palette. |

Account edits send only the fields you changed, and only after **Save**. Bio (500 characters), age range, distance in miles, gender preference, discovery visibility, and gender display are supported. **Discard** restores the loaded values. Unsaved edits survive navigation in memory. They are dropped when you quit. **Account → Preview** shows your own profile. **Account → Tinder Web** opens the website for workflows the terminal does not cover.

Super Like and Boost ask for confirmation and require credits the account already has. A rejected action leaves the current card in place. An ambiguous response is reported for you to review.

Unmatch asks for confirmation. Local conversation state is removed after the server acknowledges it. **Report help** opens [Tinder's reporting instructions](https://www.help.tinder.com/hc/en-us/articles/115003822043-Reporting-profiles-and-content). It does not file a report.

Sign out, after confirmation, removes the saved credentials, drafts, and read markers from this machine. Your Tinder account and the conversations on the server stay. The photo cache stays on disk, capped at 256 MiB and seven days; close T-TUI and use `--clear-cache` to remove it.

## Build and verify

Day-to-day use stops at [Quick start](#quick-start). This section is for changing the source.

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all --check
python -m pip install --require-hashes -r scripts/test-requirements.txt
python scripts/pty_e2e.py --binary target/release/ttui
```

Unit and HTTP contract tests cover session recovery, stale responses, message acknowledgement, opaque pagination cursors, swipes, config file permissions, Unicode input, and resizing. They use isolated local HTTP servers and a fictional offline backend. The PTY suite drives the real binary with temporary settings and cache, checks the terminal cells, and writes screenshots under `/tmp/ttui-qa`. It emulates Sixel replies to check pixel size, frame caching, carousels, overlays, resizing, and the photo toggle. It does not connect to a real account or change one.

After editing source, run `./scripts/install.sh` to refresh the installed command. For the project launcher only, `cargo build --release --locked` is enough. `./ttui` uses the release binary when it is already built.

## Documentation

- [FEATURES.md](FEATURES.md): implemented workflows, website-only gaps, how the tests are bounded, and the API sources used on 2026-09-18
- [PROGRESS.md](PROGRESS.md): what landed in 0.3.0 and the checks that were run

## License

[MIT](LICENSE). See [SECURITY.md](SECURITY.md) for local storage and reporting guidance.

## Trademarks

Tinder is a trademark of Match Group, LLC. This project is not affiliated with, endorsed by, or sponsored by Tinder or Match Group.

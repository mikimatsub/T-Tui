<div align="center">

<img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/banner.svg" alt="T-TUI: Tinder, in your terminal. Discover, chat, profiles and themes on Linux and Windows." width="100%">

<h3>A fast, keyboard-first Tinder client for Linux and Windows terminals.</h3>

[![CI](https://github.com/mikimatsub/T-Tui/actions/workflows/ci.yml/badge.svg)](https://github.com/mikimatsub/T-Tui/actions/workflows/ci.yml) [![Security](https://github.com/mikimatsub/T-Tui/actions/workflows/security.yml/badge.svg)](https://github.com/mikimatsub/T-Tui/actions/workflows/security.yml) [![npm version](https://img.shields.io/npm/v/@mikimatsub/ttui.svg)](https://www.npmjs.com/package/@mikimatsub/ttui) [![GitHub release](https://img.shields.io/github/v/release/mikimatsub/T-Tui)](https://github.com/mikimatsub/T-Tui/releases) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/mikimatsub/T-Tui/blob/main/LICENSE) ![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows-8a63d2) ![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584)

[**Install**](#-install) · [**Features**](#-features) · [**Screenshots**](#-screenshots) · [**Keys**](#%EF%B8%8F-keyboard-shortcuts) · [**FAQ**](#-faq) · [**Releases**](https://github.com/mikimatsub/T-Tui/releases)

<img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/demo.gif" alt="Animated tour of the offline demo: inbox, profile, discovery, chat, theme picker, account editor and light theme, all with fictional profiles" width="860">

<sub>Recorded from <code>ttui --mock</code>. Every profile and photo is fictional.</sub>

</div>

---

## ✨ Features

<table>
<tr>
<td width="50%" valign="top">

### 🔥 Discover
Swipe with `y` and `n`, page through photos with the arrows, read the full profile, and use Super Like or Boost with a confirmation first.

</td>
<td width="50%" valign="top">

### 💬 Chat
Inbox with search, filters, sorting, pins and read markers. Drafts save automatically, older history loads on demand, and failed sends can be restored for review.

</td>
</tr>
<tr>
<td valign="top">

### 🖼️ Real photos in the terminal
Sixel, Kitty and iTerm2 graphics with sharp resampling, including Windows Terminal 1.22 and later. Other terminals get a truecolor text fallback.

</td>
<td valign="top">

### 🎨 24 themes
T-TUI Dark and Light plus adaptations of Dracula, Catppuccin, Nord, Gruvbox, Tokyo Night, One, Solarized, Rose Pine, Monokai, Material and GitHub. Preview live with `F2`.

</td>
</tr>
<tr>
<td valign="top">

### ✍️ Your profile
Edit your bio, age range, distance, gender preference and discovery visibility. Only the fields you change are sent, and only after **Save**.

</td>
<td valign="top">

### 🔒 Private by design
The session stays on your machine: `0600` files on Linux, current-user DPAPI on Windows. Tokens are masked. No telemetry, no bypassing verification, no buying credits.

</td>
</tr>
<tr>
<td valign="top">

### 🖱️ Keyboard and mouse
Every action has a key, and every bracketed button is clickable. The wheel scrolls whatever is under the pointer.

</td>
<td valign="top">

### 🧪 Offline demo
`ttui --mock` opens a full demo with fictional profiles. Every Like becomes a match so you can try the whole flow without an account.

</td>
</tr>
</table>

---

## 📦 Install

### npm (Linux and Windows, x86-64)

```sh
npm install --global @mikimatsub/ttui
ttui --mock
```

The package bundles both native binaries. It has no dependencies, no install scripts and no runtime download. Requires Node >=24.21.0.

### Standalone binary (no Node or Rust needed)

1. Download the archive for your system and its `.sha256` file from [**GitHub Releases**](https://github.com/mikimatsub/T-Tui/releases).
2. Compare the SHA-256, then extract. Linux archives contain `ttui`; Windows archives contain `ttui.exe`.
3. Run it directly or put its folder on your `PATH`.

<details>
<summary><b>Build from source</b></summary>

To build and install from this repository on Linux:

```sh
./scripts/install.sh
```

On Windows, run `./scripts/install.ps1` in PowerShell with Rust and the MSVC C++ build tools installed. It installs to `%LOCALAPPDATA%\Programs\T-TUI`. Add that directory to your user PATH or launch `ttui.exe` directly. The script does not change PATH or require administrator privileges.

The installer builds a release binary and copies it to `~/.local/bin/ttui`. Set `TTUI_INSTALL_DIR` to use another directory. That directory must be on your `PATH`. The install needs no administrator privileges and no system package. Your saved account and settings stay in the config directory, so the installed command keeps working if you move or remove this repository. Rerun the installer after source updates.

</details>

### Requirements

| | |
|---|---|
| **OS** | Linux x86-64 or Windows x86-64 |
| **Terminal size** | Works from 50 × 16; 100 × 30 or larger shows the full split view |
| **Sharp photos** | A terminal with native graphics: **Windows Terminal 1.22+**, Foot (Sixel), Kitty or iTerm2. Others use a text fallback |

---

## 🚀 Quick start

```sh
ttui --mock      # try the offline demo first
ttui             # open your account
ttui --check     # read-only health check of a saved session
```

Press `F1` for help and `Ctrl-Q` to quit. A healthy `--check` ends with `Read-only checks complete; no messages or swipes sent.`

<details>
<summary><b>All commands</b></summary>

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

</details>

### Connect your account

Sign in on [Tinder Web](https://tinder.com), then press **Ctrl-B** on the connection screen for browser login (experimental), or paste a copied request into the **Auth token** field.

<details>
<summary><b>Step-by-step connection guide</b></summary>

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

</details>

---

## 📸 Screenshots

<table>
<tr>
<td><img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/inbox.png" alt="Inbox with conversation list, message previews and a profile preview" width="100%"></td>
<td><img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/discover.png" alt="Discovery card with photo carousel and profile prompts" width="100%"></td>
</tr>
<tr>
<td align="center"><sub>Inbox</sub></td>
<td align="center"><sub>Discover</sub></td>
</tr>
<tr>
<td><img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/chat.png" alt="Chat conversation with sent and received messages" width="100%"></td>
<td><img src="https://raw.githubusercontent.com/mikimatsub/T-Tui/main/docs/assets/themes-catppuccin.png" alt="Theme picker previewing Catppuccin" width="100%"></td>
</tr>
<tr>
<td align="center"><sub>Chat</sub></td>
<td align="center"><sub>Themes</sub></td>
</tr>
</table>

---

## ⌨️ Keyboard shortcuts

| Where | Keys |
|---|---|
| Everywhere | `F1` help · `F2` themes · `Ctrl-O` account · `Ctrl-Q` quit |
| Inbox | `↑↓`/`j k` select · `Enter` chat · `→` profile · `/` search · `d` discover |
| Discover | `y` like · `n` pass · `←→` photos · `u` Super Like · `b` Boost |
| Chat (Linux) | `Enter` send · `Alt-Enter` newline · `Esc` save draft |
| Chat (Windows) | `Ctrl-S` send · `Enter` newline |

<details>
<summary><b>Full key reference</b></summary>

| Where | Keys |
|---|---|
| Everywhere | `F1` help, `F2` themes, `Ctrl-O` account, `Ctrl-Q` / `Ctrl-C` quit |
| Inbox | `↑↓` or `j/k` select, `Enter` chat, `→` profile, `/` search, `Esc` clear search |
| Inbox tools | `f` filter, `o` sort, `p` pin or unpin, `a` mark all read on this device |
| Inbox | `d` discover, `s` settings, `r` refresh, `Home` / `End` first or last |
| Discovery | `y` like, `n` pass, `←→` photos, `↑↓` profile details, `m` inbox, `r` refresh |
| Discovery extras | `u` Super Like, `b` Boost (both ask for confirmation), `w` Tinder Web |
| Chat on Linux | Type normally, `Enter` send, `Alt-Enter` newline, `Esc` save the draft and return |
| Chat on Windows | `Ctrl-S` or **Send** sends; `Enter` inserts a newline. This keeps multiline terminal paste in the draft. |
| Chat editing | `←→`, `Home` / `End`, `Backspace` / `Delete`, `Ctrl-U` clear |
| Chat tools | `Ctrl-P` profile, `Ctrl-R` restore a failed message; settings: `Ctrl-S` on Linux or `Esc` then `s` on Windows |
| Chat safety | `Ctrl-X` unmatch (confirmation required). **Report help** opens Tinder's reporting instructions |
| Chat history | `↑↓` scroll, `PgUp` fetch older history, `PgDn` scroll toward the latest |
| Profile | `←→` photos, `↑↓` scroll, `Enter` chat, `v` external viewer, `Esc` back |
| Settings | `↑↓` select, `Enter` change, `Esc` return |
| Account | `Tab` / `Shift-Tab` choose a field, type to edit, `Ctrl-U` clear, `Ctrl-S` save, `Esc` inbox |
| Themes | `↑↓` or the mouse wheel to preview, `Enter` apply, `Esc` cancel |

Like and Pass run only from their buttons or from `y` and `n`. The arrow keys change photos. A card stays put when the server rejects the swipe. Further keypresses are ignored while a swipe is in flight.

Drafts save after 750 ms without changes, when you leave a conversation, or when you quit. Newlines in a bracketed paste stay in the draft and do not send. A message is first pending, then unconfirmed. If a send times out, delivery is uncertain: read the conversation before you send it again. Restoring a failed message puts the text back for review and does not send it.

</details>

<details>
<summary><b>Mouse, photos, themes and inbox tools</b></summary>

Click the top tabs, a conversation row, or any bracketed action. Click either half of a profile photo to move backward or forward. The wheel scrolls the pane under the pointer: inbox, history, profile details, settings, account fields, help, or themes. Click or drag in the composer to place the cursor. Unicode characters stay intact. A dialog captures clicks so they cannot reach the screen under it. Mouse capture is restored on exit. Hold **Shift** for the terminal's own text selection where the terminal supports it.

On Unix, T-TUI detects Sixel, Kitty, or iTerm2 graphics and the terminal's pixel size at startup. Photos keep their aspect ratio. Avatars fill their tiles. Resizing runs in background workers. **Settings → In-terminal photos** shows the detected renderer. The three **Text** size settings affect only the text fallback. Native photos fit the space they are given. Rendering uses [ratatui-image](https://github.com/ratatui/ratatui-image).

Open **Themes** or press **F2**. **Apply** saves the palette. **Cancel** restores the previous one. A saved dark or light choice from an older config still loads.

Included palettes: T-TUI Dark (the default), T-TUI Light, Dracula, Catppuccin Mocha, Macchiato, Frappe, and Latte, Nord, Gruvbox Dark and Light, Tokyo Night, Storm, and Day, One Dark and Light, Solarized Dark and Light, Rose Pine, Moon, and Dawn, Monokai, Material Ocean, and GitHub Dark and Light. These are T-TUI adaptations of the editor themes.

Inbox tools are All, Unread, New, Drafts, and Pinned filters, Recent, Name, and Newest sorting, search across names and message previews, persistent pins, and local read markers. Pinned conversations sort first. Filters and search apply only to loaded conversations. **More** or uppercase **L** expands the session limit by 100, up to 1,000. It refetches the expanded inbox through existing server pagination. Pins and read markers stay on this device.

</details>

<details>
<summary><b>⚙️ Configuration and data locations</b></summary>

| Name | Default | Notes |
|---|---|---|
| `TTUI_INSTALL_DIR` | `~/.local/bin` | Destination used by `scripts/install.sh`. |
| `TTUI_DATA_DIR` | Platform directories | Optional isolated root with `config` and `cache` subdirectories. |
| Windows account file | `%APPDATA%\ttui\config.json` | Credentials use current-user DPAPI; drafts and settings remain readable JSON. |
| Windows photo cache | `%LOCALAPPDATA%\ttui\photos` | Managed cache files; `--clear-cache` removes them. |
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

</details>

---

## ❓ FAQ

<details>
<summary><b>Photos look blocky.</b></summary>

Your terminal has no native graphics, so T-TUI uses the text fallback. Use Windows Terminal 1.22 or later, Foot, Kitty or iTerm2. **Settings → In-terminal photos** shows which renderer was detected.

</details>

<details>
<summary><b>I'm sent back to the connection screen.</b></summary>

The session expired and there was no refresh token. Copy a fresh request from Tinder Web and paste it again, then run `ttui --check`.

</details>

<details>
<summary><b>Browser login can't find my account.</b></summary>

Browser login is experimental. If several Chromium profiles exist, set `TTUI_BROWSER_PROFILE` to the right profile directory, or use the manual connection steps.

</details>

<details>
<summary><b>Does it work on macOS or ARM?</b></summary>

Not yet. Linux x86-64 and Windows x86-64 are the supported targets.

</details>

<details>
<summary><b>Something is off. How do I check?</b></summary>

`ttui --doctor` prints local diagnostics without reading account data or contacting Tinder. Please [open an issue](https://github.com/mikimatsub/T-Tui/issues/new/choose) without tokens, copied requests, photos or message content.

</details>

---

## 🛠️ Development

<details>
<summary><b>Build and verify</b></summary>

Day-to-day use stops at [Quick start](#quick-start). This section is for changing the source.

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --package ttui -- --check
python -m pip install --require-hashes -r scripts/test-requirements.txt
python scripts/pty_e2e.py --binary target/release/ttui
```

Unit and HTTP contract tests cover session recovery, stale responses, message acknowledgement, opaque pagination cursors, swipes, config file permissions, Unicode input, and resizing. They use isolated local HTTP servers and a fictional offline backend. The PTY suite drives the real binary with temporary settings and cache, checks the terminal cells, and writes screenshots under `/tmp/ttui-qa`. It emulates Sixel replies to check pixel size, frame caching, carousels, overlays, resizing, and the photo toggle. It does not connect to a real account or change one.

After editing source, run `./scripts/install.sh` to refresh the installed command. For the project launcher only, `cargo build --release --locked` is enough. `./ttui` uses the release binary when it is already built.

</details>

See [`CONTRIBUTING.md`](https://github.com/mikimatsub/T-Tui/blob/main/CONTRIBUTING.md) before opening a pull request.

---

## 📚 Documentation

- [`FEATURES.md`](https://github.com/mikimatsub/T-Tui/blob/main/FEATURES.md): supported workflows and what stays on Tinder Web
- [`SECURITY.md`](https://github.com/mikimatsub/T-Tui/blob/main/SECURITY.md): local storage and private vulnerability reporting
- [`CONTRIBUTING.md`](https://github.com/mikimatsub/T-Tui/blob/main/CONTRIBUTING.md): toolchain, checks and commit conventions
- [`CODE_OF_CONDUCT.md`](https://github.com/mikimatsub/T-Tui/blob/main/CODE_OF_CONDUCT.md): community expectations
- [`CHANGELOG.md`](https://github.com/mikimatsub/T-Tui/blob/main/CHANGELOG.md): version history
- [`docs/RELEASING.md`](https://github.com/mikimatsub/T-Tui/blob/main/docs/RELEASING.md) and [`docs/VERIFICATION.md`](https://github.com/mikimatsub/T-Tui/blob/main/docs/VERIFICATION.md): release gates and verification record

---

## ⚠️ Disclaimer

T-TUI is an **unofficial** client. It is not affiliated with, endorsed by, or sponsored by Tinder or Match Group, LLC. Tinder is a trademark of Match Group, LLC. Tinder can change its private API at any time or ask you to finish verification on the website. Use it at your own risk and within Tinder's terms.

## License

MIT: see [LICENSE](https://github.com/mikimatsub/T-Tui/blob/main/LICENSE). Provided "as is" without warranty.

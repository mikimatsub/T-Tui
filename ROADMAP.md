# T-TUI: review and roadmap to 1.0

Reviewed **2026-10-05**, against commit `8c5dd94` on `main` and current GitHub metadata. This is a proposed plan, not a claim that the work below has shipped. Linux and native Windows are equal priorities, as confirmed by the maintainer.

## Recommendation

Keep the existing Rust application. It already contains enough functionality for a useful 1.0. Spend the next effort on dependable sessions, truthful action results, both operating systems, and repeatable releases. Add at most two small convenience features before the release candidate.

**Proposed vision:** A dependable, keyboard-first Tinder companion for Linux and Windows, focused on browsing profiles and keeping up with conversations, with local settings and drafts and clear feedback when the service cannot complete an action.

1.0 should mean a defined set of workflows has been checked on both platforms. It cannot mean permanent compatibility with Tinder's private service or full parity with its website.

## Inventory

| Area | What exists | Assessment |
| --- | --- | --- |
| Application | One Rust binary; 18 direct dependencies; 21 Rust files totaling 10,274 lines, including tests | A manageable hobby project; no rewrite needed |
| Architecture | `TinderApi` trait, real HTTP adapter, fictional `MockApi`, event-driven application state, separate rendering and photo processing | Useful separation already exists |
| Sessions | Browser login, copied-cURL import, saved tokens, refresh-token renewal, reconnect, sign-out | Implemented, but browser extraction and persistence failure paths need work |
| Inbox | Pagination, previews, local unread markers, search, filters, sort, pins, drafts | Substantial functionality; search/filter scope is the loaded inbox, capped at 100 by settings |
| Messaging | Text composer, Unicode editing, history pagination, update polling, pending/failed delivery states | Several valuable regression tests already exist |
| Discovery | Profile details, photo carousel, Like, Pass, allowance display, duplicate-submit guard | Manual actions implemented; no auto-swipe scheduler |
| Account actions | Bio/discovery settings, own-profile preview, confirmed unmatch, Super Like, Boost | Local contracts exist; live mutation verification is explicitly absent from the prior status report |
| Presentation | Keyboard and mouse navigation, narrow layouts, 24 themes, terminal photos, external viewer | Feature-rich; Windows support is incomplete |
| Persistence | JSON config, separate demo config, drafts/pins/read markers, photo disk cache | Unix config creation sets `0600`; Windows privacy guarantees and cache retention need explicit treatment |
| Tests | 83 Rust tests in this checkout; Unix PTY script with terminal-cell and emulated Sixel checks | Useful foundation, but current Windows checks fail |
| Distribution | Cargo lockfile, shell launcher, shell installer, README, feature checklist, historical status report | No Windows installer, tracked CI workflow, release pipeline, toolchain file, or license file |
| GitHub | Private repository; one local commit; no published releases, open issues, open PRs, or returned workflow runs | A small release process still needs to be established |

Keep the strongest existing work: server acknowledgements for messages and several mutations, stale-session event guards, explicit premium/unmatch confirmation, opaque pagination cursors, Unicode handling, photo download limits, and separate credentials-free CDN requests.

## Checks performed in this review

| Check | Current result |
| --- | --- |
| Initial working tree | Clean on `main`, tracking `origin/main` |
| `cargo build --locked` on Windows | Passed, with an unused-variable warning; debug build only |
| `cargo test --locked` on Windows | **82 passed, 1 failed** out of 83 |
| `cargo clippy --locked --all-targets -- -D warnings` on Windows | **Failed**: unused `home` at `src/browser.rs:31`, needless return at `src/browser.rs:326` |
| `cargo fmt --all --check` | Passed |
| Built binary `--help` and `--version` | Passed; reports `ttui 0.3.0` and the Windows roaming config path |
| Linux build / Unix PTY suite | Not rerun; Ubuntu WSL has Python and a C compiler, but Cargo/Rust were not available through the probed PATH |
| Windows interactive terminal / graphical photos | Not manually validated in this review |
| Live Tinder HTTP checks and account actions | Not performed |
| Dependency vulnerability audit | Not performed; no installed Cargo audit command was found |

`PROGRESS.md` records 77 passing tests and 25 terminal checks from an earlier pass. Those are historical results, not evidence that this checkout passes today.

The existing default test suite also calls `extract_freshest_credentials()` against local browser profiles (`src/browser.rs:468`). That test ran as part of the suite; no credential values were printed in the captured results. It must become an explicit opt-in integration check. Its passing result does not establish successful authentication.

The failing photo test generated `D:\nonexistent\photo.png` outside the checkout. Its timestamp and 480-by-600 PNG dimensions matched the test's mock generator. Automatic approval review blocked the attempted cleanup with “blocked by policy,” so the generated file was left in place. Build outputs remain under ignored `target/`. No application source was changed during this review.

## Findings to address before 1.0

### 1. Sign-out must not hide failure to remove saved credentials — high priority

`src/app/mod.rs:1114` clears credentials in memory and calls `save_config()`. That helper only adds a toast on a write failure (`src/app/mod.rs:356`). Later in the same sign-out path, `self.toasts.clear()` removes that warning (`src/app/mod.rs:1168`).

If replacing the config fails, the old credential-bearing file can remain while the UI returns to the login screen. This is a source-confirmed failure path; it was not induced against a real saved session.

**Finish when:** failed credential removal remains visible, the application never reports a successful persistent sign-out in that state, and isolated tests cover denied/failed writes and the subsequent restart. Persist an active draft periodically with a short debounce so a crash does not discard everything since entering the conversation.

### 2. Ordinary swipes accept responses that do not confirm success — high priority

`src/api/client.rs:304` rejects numeric error statuses and non-null `error` fields, then produces `Ok(SwipeResult)` for other JSON. Consequently, `{}`, `null`, and `{"success":false}` can be treated as successful swipes. The UI then removes the card and displays “Liked” or “Passed” (`src/app/mod.rs:1499`). This is code-path analysis, not a claim that Tinder currently returns those payloads.

The stricter acknowledgement helper used for some other mutations does not protect this method. Also, HTTP 429 loses any response retry metadata in `execute()` (`src/api/client.rs:159`). Exponential backoff exists for the update poller, not as a shared cooldown for manual actions.

**Finish when:** Like and Pass each require a success shape supported by captured, sanitized service evidence; unknown results remain uncertain; no ambiguous mutation is automatically retried; repeated user input honors a shared service cooldown. Missing allowance information must not mean unlimited allowance.

### 3. Browser login is a storage heuristic, not a dependable browser integration — high priority

`src/browser.rs:135` selects the newest credential timestamp across discovered Chromium profiles. `scan_indexeddb()` searches raw `.log`/`.ldb` bytes for embedded JSON. `scan_local_storage()` looks for the first UUID near `Web/uuid`, without checking that record's origin or version. It is not parsing LevelDB records. Within IndexedDB scanning, a newer record missing one token can also retain that token from an older record.

This creates plausible failure cases involving stale/deleted records, multiple accounts, and device-ID mismatches. Firefox is offered as a Linux browser-launch fallback but is not among the credential sources. The current tests use a fabricated byte blob and an optional local scan; they do not prove a fresh browser login works.

**Finish when:** the user chooses the intended supported browser/profile, sees which account will connect, cancellation works, tokens are treated as a coherent record, and a fresh login plus refresh/restart is demonstrated on both OSes. Keep explicit session import as a documented fallback. If automatic extraction cannot be made dependable within the budget, label it experimental rather than making it the only recommended path.

### 4. Windows needs a deliberate support pass — release requirement

- `src/app/interaction.rs:545` opens website/report links with `xdg-open` on every non-macOS platform, including Windows.
- `src/app/mod.rs:1835` uses the same Unix-only choice for external photos.
- `src/graphics.rs:291` unconditionally selects half-block text graphics on non-Unix platforms, even if a Windows terminal supports an image protocol.
- `src/config.rs:158` applies explicit private file permissions only under `cfg(unix)`. This does **not** prove the Windows file is publicly readable; it means the documented owner-only guarantee is not established there.
- Installation is a shell script, and the terminal harness imports `fcntl`, `pty`, and `termios`.

**Finish when:** a native Windows binary can be installed without WSL, standard links/viewers work, persisted credentials are protected and tested, and an actual Windows terminal session exercises mouse, Unicode, resizing, reconnect, and restoration after exit. Provide a reliable photo-viewing path on each OS; choose native graphics by tested terminal capability and keep the text fallback. Exact graphics protocol parity is not necessary for equal support.

### 5. Make the tests independent of filesystem permissions and personal browser data — release requirement

The failing test is `photos::tests::pipeline_reports_failure_for_garbage_url` (`src/photos.rs:289`). It requests `/nonexistent/photo.png` and expects a failure. `MockApi::download_photo()` (`src/mock.rs:649`) generates any missing requested image, including that path if writable. A permissions-dependent test can pass on one machine and fail on another.

Use an explicitly failing test backend or injected download error, and a fresh temporary root for every filesystem test. This is a controlled failure test, not a substitute for real upstream payload fixtures. The default suite must neither scan browser sessions nor write to arbitrary absolute paths. Add Linux/Windows CI, then fix the two Windows lint failures rather than weakening the checks.

### 6. Close modest privacy and maintenance gaps

- **Cache retention:** memory photos are bounded, but `src/photos.rs:215` persists disk photos without an eviction policy. Sign-out clears in-memory photos, not the disk cache. Add a disk budget/expiry and an explicit “clear local data” action, with private cache directory permissions where supported.
- **Account isolation:** compare the persisted account identity as well as the currently loaded identity when connecting. Keep tokens separate from preferences and account-scoped drafts; make migration behavior explicit. JSON is sufficient at this size.
- **Maintainability:** `src/app/mod.rs` is 1,945 lines and `src/app/ui.rs` is 1,682. Extract session, chat, and discovery logic as those areas change. Keep `TinderApi`; avoid a wholesale rewrite or a new framework.
- **Evidence:** HTTP contract tests currently use hand-authored JSON. Supplement them with sanitized captures whose collection date and operation are recorded. Never commit tokens, real messages, identifiable profile details, or browser databases.
- **Diagnostics:** extend the current read-only check with terminal/backend, config-write capability, cache size, and actionable connection status. Redact identifiers and response bodies before offering a diagnostic export.

## A small release roadmap

These are proposed milestones, not new package-version pins. Each can be delivered through small changes; the exit criteria matter more than the number on the milestone.

| Milestone | Work | Exit criteria |
| --- | --- | --- |
| **1. Establish a trustworthy baseline** | Fix the photo test and Windows lint failures; remove real-browser scans from default tests; add Linux/Windows build, test, lint, and formatting CI; define supported OS/terminal combinations and a verified Rust toolchain | Fresh checkouts pass on both platforms without real credentials; test writes stay in temporary roots; historical verification claims are clearly labeled |
| **2. Make core workflows dependable** | Fix sign-out persistence, swipe acknowledgements and cooldowns; verify account isolation and login/refresh; unify OS-specific browser/viewer launching; add Windows installation and credential protection; cap/clear photo cache | A deliberate checklist passes on both platforms: connect, restart, inbox, history, send, receive, Like/Pass, reconnect, and sign-out; failures preserve drafts and avoid duplicate actions |
| **3. Add focused usability** | Recommended additions: incremental “load more” inbox and opt-in private notifications; improve connection status and troubleshooting. Consider the bounded auto-swipe experiment below only after milestone 2 | New behavior has targeted tests and a demonstrated terminal flow on both OSes; optional work cannot hold the release indefinitely |
| **4. Release candidate, then 1.0** | Create downloadable Linux/Windows archives with checksums; validate clean install/update/uninstall; add release notes, screenshots using demo data, troubleshooting and a short contribution guide; decide distribution/license terms | One week of ordinary use on each platform without unresolved data-loss, credential-retention, or duplicate-action defects; fresh-user setup succeeds; all advertised stable workflows have recorded evidence |

Do not silently retain unverified premium/account mutations in the stable promise. Verify them through explicitly authorized checks, mark them experimental, or hide them from the stable surface. Testing send/unmatch/paid actions is a separate deliberate step; CI uses local backends and never acts on real accounts.

For an initial compact matrix, target Linux x86-64 and native Windows x86-64, with specific tested terminal applications. Treat ARM and macOS as later targets unless there is an actual user need. Record OS/terminal versions when testing; select package/runtime versions only after checking the upstream registry and publish date.

## Extra functionality, ranked by value for effort

| Idea | Why it helps | Relative effort / recommendation |
| --- | --- | --- |
| Incremental inbox loading | Search and filters currently only see the loaded 20–100 conversations; expose the existing pagination instead of implying account-wide search | Medium; recommended before 1.0 |
| Optional desktop notifications | Makes incoming messages useful when the terminal is not focused; default to generic text with no names or message preview | Medium across two OSes; recommended if it fits the scope |
| Better connection/diagnostic screen | Explains expiry, verification, cooldowns, photo fallback, and save failures | Small–medium; include with reliability work |
| Configurable key bindings | Reduces terminal shortcut conflicts and lets users choose comfortable Like/Pass keys | Medium; good first follow-up |
| Search within a loaded conversation | Helps find context without pretending the full server history has been searched | Small–medium; alternate to notifications |
| Private session counters | Shows confirmed likes, passes, matches, and elapsed time without retaining a profile history | Small; useful alongside a session feature |
| Bounded auto-like session | Provides the requested automation using the existing Like path | Medium implementation; high compatibility/verification burden; optional experiment |
| Uploads, media chat, Likes You, Rewind, Passport, Explore parity | Each adds more service-specific behavior and maintenance | Defer until core use creates demand and current service evidence exists |

Avoid expanding themes, adding a hosted backend, an AI matching system, telemetry infrastructure, plugins, or a database for 1.0. None addresses the current release gaps.

## Auto-swiping right: technically plausible, optional scope

The application already has a discovery queue, a `swipe(true)` action, one-request-at-a-time guarding, known-zero allowance checks, and match refresh behavior (`src/app/mod.rs:1262`). A scheduler can reuse those pieces. There is no evidence here that unattended automation is permitted or that any particular rate is safe.

Tinder's current [Terms of Use](https://policies.tinder.com/terms/intl/en/), effective March 5, 2026, restrict automated access and third-party applications/API access without written consent (section 2c). This is a constraint on the existing client as well as auto-swipe. Noncommercial status and a slower request rate do not establish permission or immunity from account restrictions.

If pursued, keep the first implementation small and explicit:

1. A visible **Start session** action; off by default; demo-mode validation first.
2. A finite user-chosen maximum count and duration. No unattended startup, scheduled background runs, or resume after restart.
3. One request in flight, with counters advanced only on confirmed results. Respect service retry guidance; do not invent a “ban-safe” delay.
4. Stop on pause/quit, sign-out, account change, auth/verification failure, rate limit, known exhausted allowance, empty recommendations, network uncertainty, or an unknown response shape. Never retry an uncertain Like automatically.
5. Display confirmed results and the exact stop reason. Make pause/stop available independently of the current view. An already submitted request cannot be undone locally.
6. No automatic messages, purchases, Super Likes, or Boosts. No CAPTCHA bypass, account rotation, or attempts to conceal automation.

Build this as a small state machine that requests ordinary swipe commands; do not implement it as a timer injecting `y` keypresses. Add mock scenarios for stop-during-request, late responses, allowance exhaustion, and reconnection before any deliberate live experiment.

**Recommendation:** keep auto-swipe out of the mandatory 1.0 exit criteria. It can be a separately labeled experiment after core reliability is established. The project should remain worth using when it is disabled.

## Definition of done for 1.0

- Clean install and update work on Linux and native Windows without requiring users to install Rust.
- The selected stable feature set has been exercised through the actual UI on both systems, including at least one real supported graphics/viewer path on each.
- No known credential-removal failure is hidden, no ambiguous send/swipe is shown as confirmed, and reconnect does not duplicate actions or mix accounts.
- Default automated tests are isolated, pass on both OSes, and never contact a real account. The Unix PTY suite and Windows terminal checks have recorded results.
- Local state locations, credential protection, cache deletion, unofficial-service limitations, and experimental features are accurately documented.
- Downloadable artifacts, checksums, release notes, and a clear license/distribution decision exist.

## Evidence references

Local findings refer to the reviewed source at `8c5dd94`, especially `src/browser.rs`, `src/config.rs`, `src/api/client.rs`, `src/api/client_tests.rs`, `src/app/mod.rs`, `src/app/interaction.rs`, `src/graphics.rs`, `src/photos.rs`, `src/mock.rs`, and `scripts/pty_e2e.py`.

The repository's cited third-party implementation references were revisited: [Tinder client](https://github.com/Miguelo981/tinder-api/blob/main/src/tinder.ts), [profile adapter](https://github.com/Miguelo981/tinder-api/blob/main/src/adapters/update-profile.ts), and [older unmatch implementation](https://github.com/fbessez/Tinder/blob/master/tinder_api.py). They document what those clients implement; they do not establish a supported Tinder API contract or successful behavior for this account.

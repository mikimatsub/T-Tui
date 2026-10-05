# Release candidate verification

Recorded 2026-10-05 for the work in [PR #1](https://github.com/mikimatsub/T-Tui/pull/1). Check the PR's final commit and its completed runs before release; earlier green runs do not cover later edits.

## Completed local checks

- Rust formatting and strict Clippy checks pass with the pinned MSVC toolchain.
- Windows Rust suite: 101 passed, one explicitly ignored test that reads personal browser storage. No live credentials were read.
- Thirteen Python packaging tests pass, covering version mismatches, exact archive-member selection, duplicate/link rejection, checksum tampering, executable modes, license inclusion and stable-only WinGet manifests. One Node launcher test passes.
- Native Windows ConPTY test passes using the optimized executable: Unicode and emoji, multiline paste, on-disk draft restoration, explicit Ctrl-S demo send, discovery, settings, resize, help and clean terminal exit.
- Linux hosted terminal, unit, lint and coverage jobs passed on `68c9be8`: 99 Rust tests and all 25 terminal checks passed. [CI run 37265761913](https://github.com/mikimatsub/T-Tui/actions/runs/37265761913) also passed native Windows and installed npm-package checks on both systems. The final updated commit must pass again.

Unit-test line coverage is 73.02% on Linux at `68c9be8` (4,840 of 6,628 lines); the earlier Windows baseline was 73.37%. These are baselines, not thresholds or claims about every interaction. Coverage does not include terminal-script execution. The command-line entry point, browser integration and external process opening still need acceptance beyond mock tests.

The actual locally packed npm tarball passed Windows install/command checks. WinGet's native validator accepted a generated three-file manifest set without warnings using an explicitly non-installable schema fixture. This does not verify a public installer URL or replace stable-artifact installation testing.

The full-history Gitleaks check identified two literal offline test values in the original commit. Their exact finding fingerprints are documented in `.gitleaksignore`; no file, rule or commit is broadly excluded. The subsequent full-history scan passed. CI now runs this complete-history check in addition to the Action's change-range scan.

## Behavioral evidence

| Requirement | Evidence |
| --- | --- |
| Failed sign-out must remain actionable | `sign_out_save_failure_stays_visible_and_retry_clears_saved_session` checks saved bytes, retained state, modal visibility and successful retry. |
| Save drafts after typing and recover from disk failure | `draft_save_debounces_changes_and_persists_after_quiet_period` and `failed_draft_save_keeps_pending_retry_until_the_disk_write_succeeds` assert exact persisted Unicode content. |
| Keep Windows multiline input unsent until explicit send | `windows_multiline_keys_never_send_until_control_s` and `scripts/windows_e2e.py` cover Enter/Ctrl-Enter/Ctrl-J/Ctrl-M and the real console path. |
| Preserve account boundaries and protected credentials | Identity-change regressions, DPAPI round-trip/tamper tests and legacy/demo compatibility checks. |
| Reject ambiguous service results and avoid retries | Adapter acknowledgement and cooldown tests; real-service compatibility remains unverified. |
| Install the actual npm package | `scripts/package_smoke.py` packs, inspects hashes and complete contents, installs without scripts, executes the installed command and checks error exit status on each CI platform. |

Test fixtures model existing adapter contracts and parser edge cases. They are not newly captured Tinder responses and must not be described as live upstream validation. The local Crossterm fix has [documented upstream provenance](../vendor/crossterm/LOCAL_PATCH.md); rerun the ConPTY test when removing it.

## Remaining release acceptance

### 2026-10-05 candidate acceptance update

PR #1 and its merged commit `eed6019d247afc21dbfe26b68447a4a3af637484` passed every required check. The first tagged [release run](https://github.com/mikimatsub/T-Tui/actions/runs/37269415931) also passed CI and security, then stopped at checksum verification before creating any release. Windows text output had produced CRLF checksum records. Candidate 2 writes exact ASCII bytes with LF, and the existing archive regression now verifies raw checksum bytes. The strengthened test failed on Windows before the fix and passed afterward.

CodeQL alert 1 was reviewed and dismissed as a false positive: the profile resource ID travels to the fixed HTTPS API endpoint, redirects are disabled, credentials stay in headers, and network errors remove URLs. No scanner rule or path was excluded.

The candidate 1 native Windows executable connected through the selected Chrome profile and passed live read-only checks for account access, inbox, message history, profile details, discovery, update synchronization and photo decoding. A native ConPTY session passed navigation, saved-session restart and clean terminal exit. Sign-out was verified using a temporary session copy: credentials and account state were removed, the login screen appeared, and a subsequent check rejected access without a saved session. Personal screen content and credential values were not retained as test evidence. No messages, swipes, profile edits, unmatches or paid actions were sent.

The Linux candidate from the checked CI run passed the same read-only account checks under Ubuntu 26.04 / WSL2, plus PTY navigation, saved-session restart, isolated sign-out and `0600` config permissions. These were CI artifacts; they were not downloaded from a published release. The selected Windows Chrome profile was imported separately on each OS, without copying Windows-encrypted credentials into Linux.

Published-download checks, token renewal, older history when available, incoming-message behavior, another-account switching and deliberately authorized mutations remain unverified at this checkpoint. PR CI now also runs the release's GNU checksum command against both native archives, so future cross-platform checksum failures are caught before tagging.

Follow [RELEASING.md](RELEASING.md) for login/import, refresh, restart, pagination, incoming updates, photos, sign-out and account-switching checks on Linux and Windows. Only a deliberately authorized check may send a message or perform another live mutation. Record date, version and terminal without credentials, personal messages or photos.

The repository became public after the private checks passed. `main` requires a PR and eleven CI/security checks, including Linux and Windows builds, both package jobs and four CodeQL languages; these rules apply to administrators too. Force pushes and deletion are blocked. The release environment accepts only `v*` tags. GitHub secret scanning, push protection, dependency alerts and private vulnerability reporting are enabled. Public CodeQL and the final commit's checks must pass before merging.

Registry ownership/trusted publishing, public archive installation, npm provenance and WinGet validation/acceptance must each be verified independently. The Windows live checks above cover only the listed operations; they do not establish full acceptance. npm publication and WinGet acceptance remain unverified. Renovate configuration inherits the shared policy; app access to this repository still needs confirmation.

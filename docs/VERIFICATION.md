# Release candidate verification

Recorded 2026-10-05 for the work in [PR #1](https://github.com/mikimatsub/T-Tui/pull/1). Check the PR's final commit and its completed runs before release; earlier green runs do not cover later edits.

## Completed local checks

- Rust formatting and strict Clippy checks pass with the pinned MSVC toolchain.
- Windows Rust suite: 101 passed, one explicitly ignored test that reads personal browser storage. No live credentials were read.
- Thirteen Python packaging tests pass, covering version mismatches, exact archive-member selection, duplicate/link rejection, checksum tampering, executable modes, license inclusion and stable-only WinGet manifests. One Node launcher test passes.
- Native Windows ConPTY test passes using the optimized executable: Unicode and emoji, multiline paste, on-disk draft restoration, explicit Ctrl-S demo send, discovery, settings, resize, help and clean terminal exit.
- Linux hosted terminal, unit, lint and coverage jobs passed on `01328a0`; the final updated commit must pass again. Hosted package checks exercise the real installed npm shim on both operating systems.

The unit-test line coverage baselines before the final input/retry tests were 73.37% on Windows and 73.01% on Linux. These are baselines, not thresholds or claims about every interaction. Coverage does not include terminal-script execution. The command-line entry point, browser integration and external process opening still need acceptance beyond mock tests.

The actual locally packed npm tarball passed Windows install/command checks. WinGet's native validator accepted a generated three-file manifest set without warnings using an explicitly non-installable schema fixture. This does not verify a public installer URL or replace stable-artifact installation testing.

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

Follow [RELEASING.md](RELEASING.md) for login/import, refresh, restart, pagination, incoming updates, photos, sign-out and account-switching checks on Linux and Windows. Only a deliberately authorized check may send a message or perform another live mutation. Record date, version and terminal without credentials, personal messages or photos.

Repository visibility/protection, registry ownership/trusted publishing, public archive installation, npm provenance and WinGet validation/acceptance must each be verified independently. No live-account acceptance, npm publication or WinGet acceptance has been completed by these automated checks.

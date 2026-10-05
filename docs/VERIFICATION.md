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

Follow [RELEASING.md](RELEASING.md) for login/import, refresh, restart, pagination, incoming updates, photos, sign-out and account-switching checks on Linux and Windows. Only a deliberately authorized check may send a message or perform another live mutation. Record date, version and terminal without credentials, personal messages or photos.

The repository became public after the private checks passed. `main` requires a PR and eleven CI/security checks, including Linux and Windows builds, both package jobs and four CodeQL languages; these rules apply to administrators too. Force pushes and deletion are blocked. The release environment accepts only `v*` tags. GitHub secret scanning, push protection, dependency alerts and private vulnerability reporting are enabled. Public CodeQL and the final commit's checks must pass before merging.

Registry ownership/trusted publishing, public archive installation, npm provenance and WinGet validation/acceptance must each be verified independently. No live-account acceptance, npm publication or WinGet acceptance has been completed by these automated checks. Renovate configuration inherits the shared policy; app access to this repository still needs confirmation.

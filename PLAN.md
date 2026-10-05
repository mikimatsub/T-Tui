# Execution plan for T-TUI 1.0

Started 2026-10-05. Linux and native Windows have equal priority. The reference is the current `mikimatsub/swsd-mcp` main branch, not its older local checkout. Implementation is on `codex/1.0-release`.

## 1. Establish the release baseline

- [x] Inspect current source, review findings, reference workflows, and distribution documentation.
- [ ] Fix platform-specific lint errors and make default tests independent of personal browser sessions and filesystem permissions.
- [ ] Adopt the reference project's MIT license, conventional commits, contribution/security guidance, ownership, PR/issue templates, and inherited Renovate policy.
- [ ] Pin verified tool versions and GitHub Actions commits; record sources and verification dates.
- [ ] Add Linux/Windows CI, coverage evidence, secret scanning, dependency scanning, and CodeQL for supported languages.

## 2. Harden the existing application

- [ ] Preserve a persistent, actionable error when sign-out cannot clear saved credentials.
- [ ] Reject unconfirmed swipe responses; share service cooldowns across requests without replaying mutations.
- [ ] Isolate account state on account changes and debounce draft persistence.
- [ ] Make automatic browser extraction explicitly experimental, avoid silently selecting among multiple profiles, and stop mixing credential records.
- [ ] Protect persisted Windows secrets using the current user's Windows data-protection API; retain private Unix config files.
- [ ] Unify platform-specific URL/photo launching, add Windows installation, and provide bounded/clearable photo storage.
- [ ] Add regression coverage for changed behavior and run actual terminal smoke checks on both platforms where available.

## 3. Add focused usability

- [ ] Add incremental inbox loading with honest search scope.
- [ ] Add optional notifications with generic text and no profile/message content.
- [ ] Improve local diagnostic and connection feedback.
- [ ] Keep auto-swipe out of the stable release requirements; preserve its reviewed design as a later experiment.

## 4. Prepare and verify distribution

- [ ] Build native Linux x86-64 and Windows x86-64 archives, checksums, and provenance from a release tag.
- [ ] Prepare an npm convenience package containing the native binaries and a small launcher, without install scripts or install-time binary downloads.
- [ ] Generate a WinGet portable manifest from the actual Windows archive hash; validate it before submission.
- [ ] Document GitHub Releases as the primary binary channel, WinGet as the Windows package-manager channel, npm as an optional Node-based installation path, and crates.io as an optional source-build channel.
- [ ] Add release/version/package-content checks and trusted publishing patterned after swsd-mcp.
- [ ] Open a reviewable PR, run remote checks, repair failures, and record exact evidence.
- [ ] Publish 1.0 only after the advertised stable workflows and release artifacts pass their gates. Record any remaining live-account, public-visibility, registry-authentication, or external-review requirement explicitly.

## Boundaries

Keep the Rust application and local state model. No hosted service, container deployment, MCP Registry entry, or Docker pipeline is needed for this terminal application. Do not send messages, swipes, unmatches, or paid actions during automated testing. Public registry versions and WinGet acceptance must never be claimed before the upstream systems confirm them.

## Progress

The current reference project uses SHA-pinned Actions, minimal workflow permissions, concurrency cancellation, locked installs, tests/coverage, CodeQL, secret and OSV scanning, an inherited Renovate policy, and tag-based OIDC publishing. These practices are being adapted to Rust and native binaries.

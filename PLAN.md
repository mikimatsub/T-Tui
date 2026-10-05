# Execution plan for T-TUI 1.0

Started 2026-10-05. Linux and native Windows have equal priority. The reference is the current `mikimatsub/swsd-mcp` main branch. Implementation is on `codex/1.0-release`, reviewed in [PR #1](https://github.com/mikimatsub/T-Tui/pull/1). The original inventory and priorities remain in [ROADMAP.md](ROADMAP.md).

## 1. Establish the release baseline

- [x] Inspect source, review findings, reference workflows, and distribution documentation.
- [x] Fix platform-specific lint errors and isolate default tests from personal browser sessions and arbitrary filesystem locations.
- [x] Adopt MIT licensing, conventional commits, contribution/security guidance, ownership, PR/issue templates, and inherited Renovate policy.
- [x] Pin verified tools and Actions commits; record sources and publication dates in [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md).
- [x] Add Linux/Windows CI, coverage artifacts, full-history secret scanning, OSV dependency scanning, and public-repository CodeQL.

## 2. Harden the existing application

- [x] Preserve an actionable sign-out error when saved credentials cannot be cleared.
- [x] Reject unconfirmed swipe responses and share cooldowns without replaying mutations.
- [x] Isolate account state and debounce draft saves, retaining failed saves for retry.
- [x] Keep browser extraction experimental, require a profile choice when ambiguous, and avoid mixing credential records.
- [x] Protect Windows credentials with current-user DPAPI; retain private Unix files.
- [x] Add native Windows URL/photo launching and installation, plus bounded/clearable photo storage.
- [x] Add behavioral regressions and native terminal suites. Fix Windows emoji and multiline input defects discovered by ConPTY testing.

## 3. Add focused usability

- [x] Add Load More, increasing the fetched inbox by 100 up to 1,000 conversations. Search explicitly covers loaded conversations. This refetches the expanded inbox rather than retaining a pagination cursor.
- [x] Add account-free local diagnostics and clearer session failure feedback.
- [x] Keep auto-swipe as a later opt-in experiment, subject to the design and service restrictions in the roadmap.
- [ ] Later: generic desktop notifications. Platform notification packaging adds work without improving the core release gates; defer until the existing workflows are accepted.

## 4. Prepare and verify distribution

- [x] Implement native Linux/Windows archives, checksums, and tag-based build provenance.
- [x] Prepare an npm package with both native binaries and a launcher, with no install scripts or binary downloads.
- [x] Implement a stable-only WinGet portable manifest generator using the actual Windows archive hash.
- [x] Document GitHub Releases as the primary channel, npm as an optional Node-based installer, and WinGet as the eventual Windows package-manager channel. Defer crates.io.
- [x] Add version, archive, package-content and installed-command checks; prepare OIDC publication.
- [x] Open a reviewable PR and run hosted checks; repair failures with targeted regressions.
- [ ] Confirm all checks on the final PR commit; make the repository public as authorized, then run CodeQL and enable repository protections.
- [ ] Establish npm package ownership/authentication and configure the trusted publisher. No long-lived publishing token belongs in the repository.
- [ ] Complete the redacted live-account acceptance checklist on both systems before calling the existing service integration stable.
- [ ] Publish and verify the release candidate from actual public download/registry channels, then promote to 1.0 after acceptance.
- [ ] Validate and test the stable WinGet manifest before submission; record Microsoft acceptance separately.

## Boundaries and evidence

Keep the Rust application and local state model. No hosted service, container deployment, MCP Registry entry, or Docker pipeline is needed. Automated tests use fictional accounts and must never send real messages, swipes, unmatches, or paid actions. Public registry versions and WinGet acceptance must be confirmed upstream.

[docs/VERIFICATION.md](docs/VERIFICATION.md) records completed checks and remaining limits. A generated workflow or manifest is preparation, not proof that publication succeeded. The source remains `1.0.0-rc.1` until stable acceptance is complete.

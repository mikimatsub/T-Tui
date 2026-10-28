# Changelog

## 1.0.0 (2026-10-27)

First stable release. The maintainer verified the real-account workflows listed in [docs/VERIFICATION.md](docs/VERIFICATION.md).

- Rebuild the README as a product landing page: banner, animated offline-demo tour, feature grid, screenshots, install and requirements, FAQ and collapsible reference sections. Images use absolute URLs so they also display on npm. Add a social preview image and allow running the Security workflow manually.
- Show sharp Sixel photos in Windows Terminal 1.22 and later (detected with a bounded terminal query; other Windows consoles keep the text fallback).
- Restyle the README to match the maintainer's project page style (status badges, section dividers, a complete documentation index) and add a Code of Conduct and issue template contact links.
- Update rustls 0.23.46, rustls-webpki 0.103.16, webpki-root-certs 1.0.11 and tokio 1.54.1 to fix RUSTSEC-2026-0346 to 0349, 0382, 0383 and 0386, plus other semver-compatible Cargo updates past the three-day release soak.
- Update the test-only Python requirements: Pillow 13.0.0 (fixes its published advisories), wcwidth 0.9.4 and pywinpty 3.1.0.
- Update Rust to 1.99.1, CI Node to 24.22.1, and pinned GitHub Actions (setup-node, setup-python, upload/download-artifact, CodeQL bundle 2.27.3, OSV-Scanner 2.7.0).
- Document a narrow, expiring OSV-Scanner suppression for block-buffer GHSA-qwgh-2vcv-g2f7, which is in the lockfile but never compiled into T-TUI.

## 1.0.0-rc.2 (2026-10-05)

- Write archive checksum files with LF line endings on both platforms. The first candidate stopped before publication because GNU `sha256sum` interpreted the Windows CRLF terminator as part of the archive filename. The packaging regression now compares raw bytes so text-mode normalization cannot hide this failure.
- Run the release's GNU checksum verification against both native archives during pull-request CI, before packaging the npm bundle.
- Record initial live Windows acceptance and the completed post-merge checks. Stable acceptance and registry publication remain separate gates.

## 1.0.0-rc.1 (2026-10-05)

- Add Linux/Windows CI, terminal tests, package installation checks, coverage artifacts, secret/dependency scanning, public-repository CodeQL, and shared Renovate policy.
- Add native release archives, checksums, build provenance, an npm package with bundled binaries, and a stable-release WinGet manifest generator.
- Protect Windows credentials with current-user DPAPI; preserve sign-out failures and clear account-specific state when identities change.
- Reject ambiguous swipe acknowledgements and share server cooldowns across API requests without automatic mutation retries.
- Make browser profile selection explicit when ambiguous; keep auth and refresh tokens from the same record and isolate demo mode.
- Add native Windows URL/photo opening, an installation script, local diagnostics, bounded/clearable photo storage, quiet-period draft saves, and Load More up to 1,000 conversations.
- Preserve Windows emoji through a documented local Crossterm patch. On Windows, Enter adds a newline and Ctrl-S sends, keeping multiline paste in the draft.
- Add MIT licensing, contribution guidance, security documentation and issue/PR templates.

Live-account acceptance checks and registry setup remain release gates. Desktop notifications and auto-swipe remain future work.

## 0.3.0

Initial repository snapshot with discovery, chat, profile editing, themes, mouse support and the offline demo. See [PROGRESS.md](PROGRESS.md) for its historical verification record.

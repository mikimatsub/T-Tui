# Changelog

## 1.0.0-rc.1 (unreleased)

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

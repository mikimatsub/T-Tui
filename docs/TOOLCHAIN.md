# Verified toolchain

Registry/release evidence checked 2026-10-05. These are selected pins, not promises to remain the latest. Renovate inherits the maintainer's policy. Existing Cargo dependencies are resolved by the committed lockfile; the only new runtime dependency is Windows system bindings.

| Tool/package | Selected version | Published | Authoritative source |
| --- | --- | --- | --- |
| Rust | 1.99.0 | 2026-10-01 | [stable manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml) |
| Node LTS | 24.21.0 | 2026-09-07 | [release index](https://nodejs.org/dist/index.json), [support schedule](https://github.com/nodejs/Release/blob/main/schedule.json) |
| Python | 3.14.8 | 2026-09-30 | [release/support index](https://endoflife.date/api/python.json) |
| windows-sys | 0.61.2 | 2025-10-06 | [crates.io](https://crates.io/api/v1/crates/windows-sys) |
| cargo-llvm-cov | 0.9.1 | 2026-09-06 | [crates.io](https://crates.io/api/v1/crates/cargo-llvm-cov) |
| Gitleaks binary | 8.30.1 | 2026-03-21 | [upstream release](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1) |
| crossterm (local patch) | 0.29.0 | 2025-04-05 | [crates.io](https://crates.io/api/v1/crates/crossterm), [patch provenance](../vendor/crossterm/LOCAL_PATCH.md) |
| pyte | 0.8.2 | 2023-11-12 | [PyPI](https://pypi.org/pypi/pyte/json) |
| Pillow | 12.3.0 | 2026-07-01 | [PyPI](https://pypi.org/pypi/pillow/json) |
| pywinpty | 3.0.5 | 2026-06-10 | [PyPI](https://pypi.org/pypi/pywinpty/json) |
| wcwidth | 0.9.1 | 2026-09-23 | [PyPI release](https://pypi.org/pypi/wcwidth/0.9.1/json) |

pyte remains the upstream stable release despite its age; it is used only as an offline terminal parser, and the terminal regression suites validate its use here. wcwidth 0.9.2 was published on 2026-10-05 and is deliberately held for the inherited release-age policy. Python test requirements include every transitive dependency and hashes obtained from the selected PyPI releases. pywinpty supplies CPython 3.14 Windows wheels; its published API/source defines the ConPTY test interface.

| GitHub Action | Version | Published |
| --- | --- | --- |
| actions/checkout | 7.0.1 | 2026-07-20 |
| actions/setup-node | 7.0.0 | 2026-07-14 |
| actions/setup-python | 7.0.0 | 2026-07-20 |
| actions/upload-artifact | 7.0.1 | 2026-04-10 |
| actions/download-artifact | 8.0.1 | 2026-03-11 |
| actions/cache | 6.1.0 | 2026-06-26 |
| github/codeql-action bundle | 2.27.1 | 2026-09-22 |
| google/osv-scanner-action | 2.6.0 | 2026-09-14 |
| gitleaks/gitleaks-action | 3.0.0 | 2026-05-30 |
| actions/attest-build-provenance | 4.2.2 | 2026-08-06 |

Action versions/publication dates were checked against each repository's GitHub release metadata; exact commit IDs were resolved from its tag with `git ls-remote` and are pinned in the workflow files. Action inputs were checked in the corresponding `action.yml` or reusable workflow. GitHub runner labels were checked against [runner-images](https://github.com/actions/runner-images).

WinGet uses the published [1.12.0 manifest schema](https://github.com/microsoft/winget-cli/blob/master/schemas/JSON/manifests/v1.12.0/manifest.singleton.1.12.0.json) for zip/portable installers. It is a compatibility schema, not a pin on the latest WinGet client.

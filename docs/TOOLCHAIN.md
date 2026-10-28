# Verified toolchain

Registry/release evidence checked 2026-10-27. These are selected pins, not promises to remain the latest. Renovate inherits the maintainer's policy. Existing Cargo dependencies are resolved by the committed lockfile; the only new runtime dependency is Windows system bindings.

| Tool/package | Selected version | Published | Authoritative source |
| --- | --- | --- | --- |
| Rust | 1.99.1 | 2026-10-15 | [stable manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml) |
| Node LTS | 24.22.1 (CI; package minimum stays 24.21.0) | 2026-10-20 | [release index](https://nodejs.org/dist/index.json), [support schedule](https://github.com/nodejs/Release/blob/main/schedule.json) |
| Python | 3.14.8 | 2026-09-30 | [release/support index](https://endoflife.date/api/python.json) |
| windows-sys | 0.61.2 | 2025-10-06 | [crates.io](https://crates.io/api/v1/crates/windows-sys) |
| cargo-llvm-cov | 0.9.1 | 2026-09-06 | [crates.io](https://crates.io/api/v1/crates/cargo-llvm-cov) |
| Gitleaks binary | 8.30.1 | 2026-03-21 | [upstream release](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1) |
| crossterm (local patch) | 0.29.0 | 2025-04-05 | [crates.io](https://crates.io/api/v1/crates/crossterm), [patch provenance](../vendor/crossterm/LOCAL_PATCH.md) |
| pyte | 0.8.2 | 2023-11-12 | [PyPI](https://pypi.org/pypi/pyte/json) |
| Pillow | 13.0.0 | 2026-10-16 | [PyPI](https://pypi.org/pypi/pillow/json) |
| pywinpty | 3.1.0 | 2026-10-15 | [PyPI](https://pypi.org/pypi/pywinpty/json) |
| wcwidth | 0.9.4 | 2026-10-24 | [PyPI release](https://pypi.org/pypi/wcwidth/0.9.4/json) |

pyte remains the upstream stable release despite its age; it is used only as an offline terminal parser, and the terminal regression suites validate its use here. pywinpty 3.1.1 (2026-10-27) is held for the inherited three-day release-age policy. Python test requirements include every transitive dependency and hashes obtained from the selected PyPI releases. pywinpty supplies CPython 3.14 Windows wheels; its published API/source defines the ConPTY test interface.

| GitHub Action | Version | Published |
| --- | --- | --- |
| actions/checkout | 7.0.1 | 2026-07-20 |
| actions/setup-node | 7.1.0 | 2026-10-08 |
| actions/setup-python | 7.1.0 | 2026-10-22 |
| actions/upload-artifact | 7.0.2 | 2026-10-07 |
| actions/download-artifact | 8.0.2 | 2026-10-07 |
| actions/cache | 6.1.0 | 2026-06-26 |
| github/codeql-action bundle | 2.27.3 | 2026-10-20 |
| google/osv-scanner-action | 2.7.0 | 2026-10-12 |
| gitleaks/gitleaks-action | 3.0.0 | 2026-05-30 |
| actions/attest-build-provenance | 4.2.2 | 2026-08-06 |

Action versions/publication dates were checked against each repository's GitHub release metadata; exact commit IDs were resolved from its tag with `git ls-remote` and are pinned in the workflow files. Action inputs were checked in the corresponding `action.yml` or reusable workflow. GitHub runner labels were checked against [runner-images](https://github.com/actions/runner-images).

WinGet uses the published [1.12.0 manifest schemas](https://github.com/microsoft/winget-cli/tree/master/schemas/JSON/manifests/v1.12.0) for zip/portable installers. The generator produces version, defaultLocale and installer files with schema headers, as required by the [community repository](https://github.com/microsoft/winget-pkgs/blob/master/doc/Authoring.md). This is a compatibility schema, not a pin on the latest WinGet client.

# Contributing

Keep changes small and useful for a free terminal application. Linux x86-64 and native Windows x86-64 receive equal release attention. Start with `ttui --mock`; never use a personal account in automated tests.

The Rust toolchain is pinned in `rust-toolchain.toml`. Windows builds use MSVC and its static C runtime. Install the MSVC C++ build tools when building from source. Run:

```sh
cargo fmt --package ttui -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
node --test scripts/package.test.mjs
```

On machines whose rustup default host is GNU, use the explicitly installed MSVC toolchain for Windows (`cargo +1.99.0-x86_64-pc-windows-msvc ...`). Tool versions and publication dates are recorded in [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md).

Install terminal-test dependencies in a virtual environment with `python -m pip install --require-hashes -r scripts/test-requirements.txt`. Linux runs `python scripts/pty_e2e.py --binary target/release/ttui`; Windows runs `python scripts/windows_e2e.py --binary target/release/ttui.exe`. These tests use fictional data and temporary directories. The browser-storage integration test is intentionally ignored unless explicitly requested with `--ignored`.

Add regression cases for meaningful behavior changes, especially failed writes, stale sessions, action acknowledgements and platform differences. Existing synthetic HTTP responses test adapter assumptions; they are not evidence of a current upstream contract. New API behavior requires sanitized observed evidence and its capture date. Never commit account data to fixtures or logs.

Use conventional commit subjects (`fix:`, `feat:`, `docs:`, `ci:`). Describe the reason for the change and its verification. Keep unrelated edits separate. New runtime dependencies, install scripts and services need a clear project-specific reason. The npm package deliberately has no dependencies or lifecycle scripts.

Renovate inherits the maintainer's shared policy from `mikimatsub/.github:renovate-config`. Keep its release-age and review boundaries intact. Verify registry publication dates, dependency relationships and security advisories before changing pins. Do not weaken checks to make a dependency update pass.

CI runs format, strict Clippy, unit/adapter tests, native terminal tests, optimized builds, package installation checks, coverage, secret scanning and dependency scanning. CodeQL runs when the repository is public. All Actions use immutable commit SHAs and minimum job permissions. See [docs/RELEASING.md](docs/RELEASING.md) for publication gates.

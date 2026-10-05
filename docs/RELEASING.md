# Release procedure

GitHub Releases is the primary binary channel. The initial supported targets are Linux x86-64 (glibc, built on Ubuntu 22.04) and Windows x86-64 (MSVC, static C runtime). Native installation requires neither Node nor Rust. ARM and macOS builds are outside the 1.0 support promise.

The npm package `@mikimatsub/ttui` is a convenience for Node users. It contains both native binaries, an argument-preserving launcher, MIT license, README and hashes. It has no dependencies, postinstall script or runtime download. CI packs the actual tarball, checks its complete file list and binary hashes, installs it without scripts in a temporary directory, then runs the installed command on both platforms.

WinGet can distribute the Windows zip as a portable installer with the `ttui` alias. The generator writes an actual archive SHA-256 and the tagged GitHub asset URL. A manifest is prepared only for stable versions. Microsoft validation and community-repository review are separate from GitHub publication; availability cannot be promised before acceptance. crates.io source distribution can be added later; Cargo publishing is deliberately disabled for now.

## One-time setup

1. Make the repository public after reviewing the source and history. Public npm provenance, free OSS CodeQL and community WinGet downloads depend on public access. Configure the `release` GitHub environment for tag-only deployment. The publishing job also verifies that the tag points to a commit on protected `main`.
2. Require CI and security checks on `main`, block force pushes and require a PR. Choose required check names from a completed run. Enable private vulnerability reporting. Install/enable Renovate for this repository; a config file alone does not install the app.
3. Establish ownership of `@mikimatsub/ttui`. The name was absent from npm at review time; availability is not a reservation. Bootstrap the first publication with the maintainer's authenticated npm session if needed. Never store a long-lived publish token in this repository.
4. In npm package settings, configure a GitHub Actions trusted publisher for owner `mikimatsub`, repository `T-Tui`, workflow `release.yml`, environment `release`. Enable the repository variable `NPM_PUBLISH_ENABLED=true` only after this setup is verified. Subsequent publication uses GitHub OIDC and provenance. The workflow uses the npm bundled with the pinned Node LTS. Native GitHub publication runs independently of this one-time npm setup.

## Before stable 1.0

- Green Linux and Windows unit, adapter, lint, terminal, package and security checks for the exact release commit. Review coverage results for gaps; no arbitrary coverage percentage substitutes for behavior checks.
- A maintainer verifies real-account login/import, restart, refresh, inbox/history pagination, incoming updates, photos, sign-out and account switching on both platforms. Store a redacted checklist with date, version and terminal; never store personal payloads.
- Any advertised mutation requires a deliberately authorized live check: sending to a consenting test contact, Like/Pass, profile edits, unmatch and credit-consuming actions. If a workflow is unverified, keep it labeled experimental or remove it from the stable support promise. Automated tests must not perform these actions.
- Verify current Tinder restrictions and decide whether to distribute an unofficial client. Noncommercial status does not grant API permission. Auto-swipe remains outside 1.0.
- Smoke install the release candidate in a clean Windows user profile and clean Linux environment. Verify runtime imports and executable architecture; record any minimum OS/library requirements actually observed.

## Publish

Update `Cargo.toml`, `Cargo.lock`, `npm/package.json` and changelog together. `python scripts/release.py check --tag vVERSION` rejects mismatches. Merge the reviewed change with required checks passing, tag that commit and push the tag. The workflow reruns CI and security from the tag, then enters the protected release environment. No manual dispatch can silently publish the default branch.

The release workflow verifies checksums, attests artifacts, and creates the GitHub release, including the tested npm tarball as a downloadable asset. When `NPM_PUBLISH_ENABLED=true`, a separate job publishes that tarball to npm (`next` for release candidates, `latest` for stable). Otherwise npm publication is explicitly skipped; a GitHub asset is not an npm registry release. Configure native-release prerequisites before pushing a tag. Registry versions are immutable: if publication succeeded, do not try to overwrite that version. If a later step fails, inspect upstream state and recover only the missing operation from the original verified run. Do not rebuild different binaries under an existing tag.

After GitHub publication, the workflow downloads the actual public assets without authentication on both platforms. It verifies the archives and npm tarball against GitHub attestations for the exact release tag, source commit and signing workflow, then runs the native executable and the installed npm shim with isolated offline data. npm publication waits for these checks. Once npm publication is enabled, separately install from the registry on both platforms and inspect npm provenance. Only then call that channel verified.

For WinGet, validate the generated manifest with `winget validate` using the installed command's help. Then test installation from that manifest in an isolated Windows environment and submit it to `microsoft/winget-pkgs`. The GitHub asset must already be publicly downloadable. Do not submit a placeholder URL or hash. The manifest generator is preparation, not evidence of acceptance.

Sources checked 2026-10-05: [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/), [WinGet manifests](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest), [WinGet repository](https://learn.microsoft.com/en-us/windows/package-manager/package/repository), [Cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).

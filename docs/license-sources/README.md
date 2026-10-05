# Supplemental dependency licenses

Some published crates omit the license files from their repository root. These copies come from each crate's exact `.cargo_vcs_info.json` revision, recorded in `sources.json`, and were retrieved on 2026-10-05. Do not replace them with guessed copyright text or licenses from another version.

`scripts/licenses.py` collects license and notice files from the locked dependency sources for both supported targets, including build dependencies. It uses these supplemental files only where the crate contains none, and fails for an unknown missing license. Identical texts are deduplicated in `THIRD_PARTY_NOTICES.txt` with SHA-256 references. The separate `RUST-STDLIB-LICENSE.html` is the standard-library copyright document included in the pinned Rust toolchain.

After changing Cargo dependencies or Rust, run `python scripts/licenses.py`, review new sources or license terms, and commit the regenerated documents. CI checks regeneration on both operating systems. The native archives and npm tarball include both documents.

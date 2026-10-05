# Local patch provenance

Base: crossterm 0.29.0, published 2025-04-05; verified as the current non-yanked release against crates.io on 2026-10-05. Source and MIT license are copied from that registry release. Cargo.toml preserves its original runtime dependencies and omits only upstream development dependencies/examples. No other source edits are intended.

One Windows parser guard prevents surrogate key-up events from consuming a buffered key-down half. This fixes supplementary Unicode input disappearing in ConPTY. The defect was independently reproduced with T-TUI's `scripts/windows_e2e.py`.

References: [upstream issue 1072](https://github.com/crossterm-rs/crossterm/issues/1072), [proposed fix 1073](https://github.com/crossterm-rs/crossterm/pull/1073). The Alt-code branch is deliberately unchanged. The source patch is equivalent to the reviewed upstream guard; retaining the registry source avoids importing unrelated unreleased changes from a fork.

Remove this override after an upstream published release includes the fix and the Windows terminal test passes without it. Review this local source when updating dependencies; registry scanners may not identify advisories for path-patched packages. Preserve the MIT license in redistributions. Do not reformat upstream source as part of application changes.

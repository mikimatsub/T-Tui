#!/usr/bin/env bash
# Install a standalone release binary for the current user.
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
install_dir="${TTUI_INSTALL_DIR:-$HOME/.local/bin}"

cargo build --release --locked --manifest-path "$project_dir/Cargo.toml"
mkdir -p -- "$install_dir"

# Replacing the executable atomically also works while an older copy is open.
staged="$(mktemp "$install_dir/.ttui.XXXXXX")"
cleanup() {
  if [[ -n "$staged" && -e "$staged" ]]; then
    rm -- "$staged"
  fi
}
trap cleanup EXIT
install -m 755 -- "$project_dir/target/release/ttui" "$staged"
mv -- "$staged" "$install_dir/ttui"
staged=""

printf 'Installed %s\n' "$install_dir/ttui"
printf 'Run ttui to open your account, or ttui --mock for the offline demo.\n'

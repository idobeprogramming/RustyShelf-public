#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
flags=()
if [[ -n ${CARGO_ENCODED_RUSTFLAGS:-} ]]; then
    IFS=$'\x1f' read -ra flags <<< "$CARGO_ENCODED_RUSTFLAGS"
else
    read -ra flags <<< "${RUSTFLAGS:-}"
fi
for root in "$HOME" "$PWD" "${CARGO_HOME:-}" "${RUSTUP_HOME:-}" "${CARGO_TARGET_DIR:-}"; do
    [[ -z "$root" ]] || flags+=("--remap-path-prefix=$root=/build")
done
(IFS=$'\x1f'; export CARGO_ENCODED_RUSTFLAGS="${flags[*]}"; cargo build --release)
stage="dist/RustyShelf-linux-x86_64"
mkdir -p "$stage"
install -m 755 target/release/rustyshelf "$stage/rustyshelf"
install -m 644 packaging/rustyshelf.desktop "$stage/rustyshelf.desktop"
install -m 644 README.md "$stage/README.md"
install -m 644 CUSTOM_STYLE.md "$stage/CUSTOM_STYLE.md"
install -m 644 examples/rustyshelf.css "$stage/rustyshelf.css"
tar --owner=0 --group=0 --numeric-owner -C dist -czf dist/RustyShelf-linux-x86_64.tar.gz \
    RustyShelf-linux-x86_64/{rustyshelf,rustyshelf.desktop,README.md,CUSTOM_STYLE.md,rustyshelf.css}
printf 'Package créé : %s\n' "$PWD/dist/RustyShelf-linux-x86_64.tar.gz"

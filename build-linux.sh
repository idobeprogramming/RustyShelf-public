#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
cargo build --release
stage="dist/GameShelf-linux-x86_64"
mkdir -p "$stage"
install -m 755 target/release/gameshelf "$stage/gameshelf"
install -m 644 packaging/gameshelf.desktop "$stage/gameshelf.desktop"
install -m 644 README.md "$stage/README.md"
tar -C dist -czf dist/GameShelf-linux-x86_64.tar.gz GameShelf-linux-x86_64
printf 'Package créé : %s\n' "$PWD/dist/GameShelf-linux-x86_64.tar.gz"

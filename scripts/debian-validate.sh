#!/bin/sh
set -eu
export CARGO_TARGET_DIR=/build/kanlo-target
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
mkdir -p artifacts
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu armv7-unknown-linux-gnueabihf; do
  case "$target" in
    aarch64-*) export CC=aarch64-linux-gnu-gcc ;;
    armv7-*) export CC=arm-linux-gnueabihf-gcc ;;
    *) export CC=gcc ;;
  esac
  cargo build --workspace --locked --target "$target"
  cp "$CARGO_TARGET_DIR/$target/debug/kanlo-server" "artifacts/kanlo-server-debug-$target"
  file "artifacts/kanlo-server-debug-$target"
  sha256sum "artifacts/kanlo-server-debug-$target" > "artifacts/kanlo-server-debug-$target.sha256"
done
export CC=gcc
cargo test --workspace --locked --target x86_64-unknown-linux-gnu

#!/bin/bash
# Translate Rust target triple to Zig triple for zig cc
args=()
for arg in "$@"; do
    case "$arg" in
        --target=armv7-unknown-linux-musleabihf)
            args+=("--target=arm-linux-musleabihf")
            ;;
        *)
            args+=("$arg")
            ;;
    esac
done
exec /usr/bin/zig cc "${args[@]}"

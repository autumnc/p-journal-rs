#!/bin/bash
# Use zig cc as linker, translating args for zig compatibility
args=("-target" "arm-linux-musleabihf")
for arg in "$@"; do
    case "$arg" in
        */crt1.o|*/crti.o|*/crtbegin.o|*/crtend.o|*/crtn.o)
            ;;  # skip: zig provides its own crt objects
        -nostartfiles)
            ;;  # skip: zig cc handles start files itself
        *)
            args+=("$arg")
            ;;
    esac
done
exec /usr/bin/zig cc "${args[@]}"

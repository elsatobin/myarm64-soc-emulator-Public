default:
    @just --list

# format, clippy, test
[group('dev')]
check: fmt_check clippy test

[group('dev')]
fmt:
    cargo fmt

[group('dev')]
fmt_check:
    cargo fmt -- --check

[group('dev')]
clippy:
    cargo clippy --all-targets -- -D warnings

[group('dev')]
test:
    cargo nextest run

# standard cargo test (for coverage, etc)
[group('dev')]
test_cargo:
    cargo test

[group('dev')]
watch:
    bacon

[group('dev')]
watch_test:
    bacon test

[group('run')]
run *ARGS:
    cargo run -- {{ARGS}}

[group('run')]
run_release *ARGS:
    cargo run --release -- {{ARGS}}

[group('build')]
build:
    cargo build

[group('build')]
release:
    cargo build --release

# assemble test.s with zig
[group('build')]
asm:
    zig cc -target aarch64-freestanding-none -nostdlib -Wl,-T,tests/link.ld -o tests/test.elf tests/test.s
    zig objcopy -O binary tests/test.elf tests/test.bin
    rm tests/test.elf

# assemble all .s files in ./bins
[group('build')]
asm_bins:
    #!/usr/bin/env bash
    set -euo pipefail
    for src in bins/*.s; do
        [ -f "$src" ] || continue
        name="${src%.s}"
        echo "assembling $src -> ${name}.bin"
        zig cc -target aarch64-freestanding-none -nostdlib -Wl,-T,tests/link.ld -o "${name}.elf" "$src"
        zig objcopy -O binary "${name}.elf" "${name}.bin"
        rm "${name}.elf"
    done

# show macro expansion
[group('tools')]
expand MODULE:
    cargo expand {{MODULE}}

# binary size breakdown
[group('tools')]
bloat:
    cargo bloat --release

[group('deps')]
outdated:
    cargo outdated

# security audit
[group('deps')]
audit:
    cargo audit

# find unused deps
[group('deps')]
machete:
    cargo machete

[confirm("delete all build artifacts?")]
[group('tools')]
clean:
    cargo clean

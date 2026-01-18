# arm64-soc-emulator

hobby project where i build an arm64 emulator from scratch to learn the architecture at a deeper level. written in rust (told myself i'd stop, relapsing anyway)

i already had a decent understanding of arm64 from a higher level: cache hierarchies, privilege levels, the isa in general. but i wanted to see how it actually gets implemented. how instructions are encoded at the bit level, how the decoder pulls apart opcodes, how execution flows through the pipeline. the stuff you don't really touch when you're just writing code that runs on arm

## what's implemented

- cpu core with instruction decoding and execution
- memory bus and ram
- peripherals: uart, timer, gic
- exception levels and basic exception handling

it can load and run raw binaries. nowhere near complete, but enough to execute simple programs and see what's happening inside

## usage

```sh
cargo run -- bins/add.bin
```

flags:
- `-e 0x40000000` set entry point
- `-r 64` ram size in mb
- `-m 1000` max instructions
- `-d` dump cpu state after

## building test programs

the `bins/` folder has some assembly snippets. build them with:

```sh
just asm_bins
```

uses zig for cross-compilation to aarch64

## dev

```sh
just check   # fmt, clippy, test
just watch   # bacon
```

## docs

arm reference manuals are in `docs/`. the a64 instruction set guide is the most useful one for this project so far tbh

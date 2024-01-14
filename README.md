# Bold

An experimental x86-64 linker.

This is my second linker which I write to understand how linker works.
bold aims to link Rust and C++ programs.


## Status

Implemented features are as follows:
- .o and .a files
- static link (Some relocation types are missing)

# Run

```bash
$ as examples/hello.asm -o hello.o
$ RUST_LOG=debug cargo run hello.o
$ ./a.out
Hello, world
```

## TODO
- Support weak symbols
    - preliminary
- Support special(?) symbols
    - _GLOBAL_OFFSET_TABLE_
    - __start* and __stop*
    - and more?
- Support SHN_COMMON

## References
- https://refspecs.linuxfoundation.org/elf/elf.pdf
- https://github.com/rui314/mold

## Licenses

- gnu.ld: Copyright (C) 2014-2022 Free Software Foundation, Inc.
- Others: MIT

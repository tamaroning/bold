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
$ cargo run <file>...
```

```bash
./tests/hello_nolibc.sh
```

## TODO
- Support weak symbols
    - preliminary
- Support special(?) symbols
    - _GLOBAL_OFFSET_TABLE_
    - __start* and __stop*
    - and more?
- Support SHN_COMMON
- .bss section

## References
- mold, https://github.com/rui314/mold
- ELF spec, https://refspecs.linuxfoundation.org/elf/elf.pdf
- System V ABI spec, https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf
- ELF Handling For Thread-Local-Storage, https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf


## Licenses

- gnu.ld: Copyright (C) 2014-2022 Free Software Foundation, Inc.
- Others: MIT

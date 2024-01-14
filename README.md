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

- Support SHN_ABS and SHN_COMMON

## References
- https://refspecs.linuxfoundation.org/elf/elf.pdf
- https://github.com/rui314/mold

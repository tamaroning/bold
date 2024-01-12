# Bold

An experimental x86-64 linker

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

# Bold

An experimental x86-64 linker

## Status

Static linking partially works

# Run

```bash
$ as examples/hello.asm -o hello.o
$ RUST_LOG=debug cargo run hello.o
$ ./a.out
Hello, world
```

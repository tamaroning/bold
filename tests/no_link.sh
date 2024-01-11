echo '.globl _start; _start: jmp _start' | cc -o %no_link.o -c -x assembler -
cargo run %no_link.o

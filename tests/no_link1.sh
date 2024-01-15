echo '.globl _start; _start: jmp _start' | cc -o %no_link1.o -c -x assembler -
cargo run %no_link1.o

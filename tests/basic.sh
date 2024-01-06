echo '.globl _start; _start: jmp loop' | cc -o %t1.o -c -x assembler -
echo '.globl loop; loop: jmp loop' | cc -o %t2.o -c -x assembler -
cargo run %t1.o %t2.o

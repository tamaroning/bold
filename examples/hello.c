// cc -o hello.o hello.c -nolibc -c
// cargo run hello.o $(gcc -print-libgcc-file-name)

#include <stdio.h>

int main(void) {
    printf("Hello, world!\n");
    return 0;
}

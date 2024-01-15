# https://gist.github.com/carloscarcamo/6833d19b726af698e62b
cat <<EOF | cc -o %hello_nolibc.o -c -x assembler -
.global _start
.text

_start:
    # write (1, msj, 13)
    mov \$1, %rax            # system call 1 is write
    mov \$1, %rdi            # file handler 1 is stdout
    mov \$message, %rsi      # address of string to output
    mov \$13, %rdx           # number of bytes
    syscall

    # exit(0)
    mov \$60, %rax           # system call 60 is exit
    xor %rdi, %rdi          # we want to return code 0
    syscall

message:
    .ascii "Hello, world\n"
EOF

cargo run %hello_nolibc.o

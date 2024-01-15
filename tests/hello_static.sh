# Disable TLS for simplicity.
cat <<EOF | cc -o %hello.o --disable-tls -c -x assembler -
  .text
  .globl main
main:
  lea msg(%rip), %rdi
  xor %rax, %rax
  call printf
  xor %rax, %rax
  ret

  .data
msg:
  .string "Hello world\n"
EOF

cargo run /usr/lib/x86_64-linux-gnu/crt1.o \
  /usr/lib/x86_64-linux-gnu/crti.o \
  /usr/lib/gcc/x86_64-linux-gnu/9/crtbeginT.o \
  /usr/lib/gcc/x86_64-linux-gnu/9/libgcc.a \
  /usr/lib/gcc/x86_64-linux-gnu/9/libgcc_eh.a \
  /usr/lib/x86_64-linux-gnu/libc.a \
  /usr/lib/gcc/x86_64-linux-gnu/9/crtend.o \
  /usr/lib/x86_64-linux-gnu/crtn.o \
  %hello.o

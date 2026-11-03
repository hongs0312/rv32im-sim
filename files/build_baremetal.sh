#!/bin/bash
# files/build_baremetal.sh

echo "컴파일 중: main.c (Baremetal)"

# 1. ELF 파일 생성
riscv64-unknown-elf-gcc -O2 -nostdlib -mabi=ilp32 -march=rv32im -mno-relax -T linker.ld entry.s main.c -o temp.elf

# 2. BIN 파일 추출
riscv64-unknown-elf-objcopy -O binary temp.elf baremetal.bin

# 3. 임시 파일 삭제
# rm temp.elf

echo "빌드 완료: baremetal.bin"
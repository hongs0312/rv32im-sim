#!/bin/bash
set -xue

CC="/usr/bin/clang"
# 추출 도구는 잘 작동했던 GNU 도구를 그대로 사용합니다.
# OBJCOPY="riscv64-unknown-elf-objcopy"
OBJCOPY="/usr/bin/llvm-objcopy"

# 1. Chrome 페이지의 Clang 옵션 + 우분투용 32비트 아키텍처 명시
CFLAGS="-std=c11 -O2 -g3 -gdwarf-4 -Wall -Wextra --target=riscv32-unknown-elf -march=rv32im -mabi=ilp32 -fno-stack-protector -ffreestanding -nostdlib -I."

# 💡 2. 문제 해결의 핵심: Clang이 리눅스의 64비트 GNU 링커를 '32비트 모드(elf32lriscv)'로 호출하도록 강제!
LDFLAGS="--ld-path=riscv64-unknown-elf-ld -Wl,-m,elf32lriscv"

echo "  -> 유저 애플리케이션 검사 및 빌드 중..."
if [ -f "user.ld" ] && [ -f "shell.c" ] && [ -f "user.c" ]; then
    $CC $CFLAGS $LDFLAGS -Wl,-Tuser.ld -Wl,-Map=shell.map -o shell.elf shell.c user.c common.c
    $OBJCOPY --set-section-flags .bss=alloc,contents -O binary shell.elf shell.bin
    $OBJCOPY -I binary -O elf32-littleriscv shell.bin shell.bin.o
    SHELL_OBJ="shell.bin.o"
else
    echo "  -> 유저 파일이 없어 커널만 빌드합니다."
    SHELL_OBJ=""
fi

echo "  -> 커널 빌드 중..."
# 커널 소스 자동 탐색 (user.c, shell.c, mini_sbi.s 제외)
KERNEL_SRCS=$(ls *.c *.s *.S 2>/dev/null | grep -v -E "^(shell\.c|user\.c|mini_sbi\.s)$" | tr '\n' ' ')

$CC $CFLAGS $LDFLAGS -Wl,-Tkernel.ld -Wl,-Map=kernel.map -o kernel.elf $KERNEL_SRCS $SHELL_OBJ

echo "  -> 순수 커널 바이너리 추출 중..."
$OBJCOPY -O binary --set-section-flags .bss=alloc,load,contents kernel.elf kernel.bin

echo "  -> 가상 디스크 이미지 생성 중..."
mkdir -p disk
(cd disk && tar cf ../disk.tar --format=ustar *.txt)

echo "  -> OS 빌드 완료!"
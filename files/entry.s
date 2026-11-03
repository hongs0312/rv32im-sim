.section .text.init
.global _start

_start:
    .option push
    .option norelax

    # S-mode이므로 mtvec이 아니라 stvec을 설정해야 합니다.
    la t0, _start
    csrw stvec, t0

    # 1. 스택 포인터 초기화 (DRAM 상단 16MB 지점 등)
    li sp, 0x81000000

    # 2. C main 함수 호출 (벤치마크 실행)
    call main

    # 3. main의 리턴 값(a0, exit code)을 담고 SBI 셧다운(a7 = 8) 호출
    # mini_sbi.s의 sbi_shutdown 루틴이 a7=8을 감지하여 syscon으로 전원 종료를 수행합니다.
    mv a0, a0     # exit code (main의 리턴값)
    li a7, 8      # SBI shutdown 기능 번호
    ecall

inf_loop:
    j inf_loop
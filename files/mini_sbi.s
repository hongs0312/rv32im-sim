.section .text
.global _start

# SBI 내부 변수 저장용 고정 메모리 주소 (32비트 워드 단위 정렬)
.equ SBI_SCRATCH_BASE, 0x80001000
# 0(SBI_SCRATCH_BASE)  : OS가 등록한 트랩 핸들러 주소 (stvec 역할)
# 4(SBI_SCRATCH_BASE)  : SBI 내부 임시 t1 백업 공간

_start:
    # 1. 초기 펌웨어 스택 포인터 설정 (DRAM 최상단 임시 활용)
    li sp, 0x81000000

    # 2. mscratch 초기화
    csrw mscratch, zero
    # csrw sscratch, zero

    # 3. SBI 트랩 핸들러를 mtvec에 등록 (모든 M-mode 트랩의 진입점)
    la t0, sbi_trap_handler
    csrw mtvec, t0

    # 4. 예외 및 인터럽트 위임(Delegation) 하드웨어 설정
    # S-mode Environment Call (9)을 '제외'한 모든 예외는 하드웨어가 자동으로 S-mode(stvec)로 던지게 만듭니다.
    # medeleg: 9번 비트(S-ecall)만 0으로 두고, 나머지(Instruction/Page Fault, illegal 등)는 1로 설정
    li t0, 0xFFFFFDFF
    csrw medeleg, t0
    
    # mideleg: S-mode 타이머(STIP=5), 소프트웨어(SSIP=1), 외부(SEIP=9) 인터럽트도 커널이 직접 받도록 위임
    li t0, 0x0222
    csrw mideleg, t0

    # 5. mstatus.MPP를 S-Mode(0x0800)로 설정하여 OS를 Supervisor 모드로 부팅 준비
    li t0, 0x1800
    csrc mstatus, t0
    li t0, 0x0800
    csrs mstatus, t0

    # 6. OS 커널 진입점(0x80200000)으로 점프
    li t0, 0x80200000
    csrw mepc, t0
    mret

# ============================================================================
# OpenSBI 핵심 트랩 라우터 (오직 S-mode ecall 및 위임되지 않은 인터럽트만 진입)
# ============================================================================
.align 4
sbi_trap_handler:
    # 레지스터 오염을 100% 방지하기 위해 t0를 mscratch에 대피하고 t1을 고정 메모리에 백업합니다.
    csrw mscratch, t0           
    li t0, SBI_SCRATCH_BASE
    sw t1, 4(t0)           

    # 트랩 원인 분석
    csrr t0, mcause
    bltz t0, handle_m_irq       # 최고위 비트가 1이면 M-mode 인터럽트 처리

    # S-mode가 호출한 ecall은 mcause 코드가 '9'입니다.
    li t1, 9
    beq t0, t1, handle_sbi_ecall

    # 하드웨어 위임 덕분에 일반 예외는 이리로 오지 않지만, 예외적 상황을 위한 무한 루프
unknown_exception:
    j .

# ============================================================================
# [A] SBI 시스템 콜 처리 (mcause == 9)
# ============================================================================
handle_sbi_ecall:
    # 대피했던 t0, t1을 복원하고 SBI 전용 안전 스택 구축
    li t0, SBI_SCRATCH_BASE
    lw t1, 4(t0)
    sw sp, 8(t0)                    # SBI 전용 안전 스택 포인터 저장

    li sp, 0x81000000            # SBI 전용 안전 스택 포인터 설정 (DRAM 최상단 임시 활용)

    csrr t0, mscratch               # t0 복원
    csrw mscratch, zero         

    # SBI 전용 안전 스택 프레임 생성
    addi sp, sp, -16
    sw ra, 0(sp)
    sw t0, 4(sp)
    sw t1, 8(sp)
    sw t2, 12(sp)

    # SBI 기능 분기 (a7 레지스터 기준)
    li t1, 0
    beq a7, t1, sbi_set_timer
    li t1, 1
    beq a7, t1, sbi_putchar
    li t1, 2
    beq a7, t1, sbi_getchar
    li t1, 8
    beq a7, t1, sbi_shutdown
    li t1, 10
    beq a7, t1, sbi_set_trap_handler

    # 정의되지 않은 SBI 호출 시 에러 반환 (a0 = -1)
    li a0, -1
    j sbi_ecall_exit

sbi_set_trap_handler:
    li t0, SBI_SCRATCH_BASE
    sw a0, 0(t0)                # OS 커널 핸들러 주소 저장
    csrw stvec, a0              # 하드웨어 stvec에 동기화
    li a0, 0                    
    li a1, 0
    j sbi_ecall_exit

sbi_set_timer:
    # [RV32I 환경 최적화]
    # RV32I에서 mtimecmp는 32비트 메모리 2개(LO/HI)로 쪼개져 있습니다.
    # a0에 32비트 시간 값이 넘어온다고 가정하고 처리합니다. (a1이 상위 32비트인 경우 상위 비트는 무시하거나 별도 처리)
    li t0, 0x02004000           # CLINT_MTIMECMP_LO 주소
    li t2, -1
    sw t2, 4(t0)                # 상위 32비트(HI)를 일시적으로 최댓값으로 설정해 오동작 방지
    sw a0, 0(t0)                # 하위 32비트(LO)에 타이머 값 설정
    sw zero, 4(t0)              # 상위 32비트(HI)를 0으로 초기화 (시뮬레이터 환경 맞춤)
    
    # S-mode 타이머 인터럽트(STIP) 활성화 요청
    li t0, 0x20                 # STIP 비트 (bit 5)
    csrs mip, t0
    
    li a0, 0
    li a1, 0
    j sbi_ecall_exit

sbi_putchar:
    li t0, 0x10000000           # UART_TX 물리 주소
    sb a0, 0(t0)
    li a0, 0 
    li a1, 0
    j sbi_ecall_exit

sbi_getchar:
    li t0, 0x10000000          # UART_BASE 주소
    lb t1, 5(t0)               # UART_LSR (오프셋 5) 상태 읽기
    andi t1, t1, 1             # Bit 0 (Data Ready) 마스킹
    beqz t1, .Lgetchar_empty   # 0이면 입력된 글자가 없음

    # 글자가 있는 경우
    lb a0, 0(t0)               # UART_RHR (오프셋 0)에서 1바이트 읽기 (반환값 a0)
    j sbi_ecall_exit           # ecall 종료 후 커널로 복귀

.Lgetchar_empty:
    li a0, -1                  # 입력된 글자가 없으면 -1 반환
    j sbi_ecall_exit
    
sbi_shutdown:
    li t0, 0x00100000           # QEMU/시뮬레이터 Poweroff 주소
    beqz a0, shutdown_pass
    li t1, 0x3333
    sw t1, 0(t0)
    j .
shutdown_pass:
    li t1, 0x5555
    sw t1, 0(t0)
    j .

sbi_ecall_exit:
    # ecall 명령어 다음 줄로 돌아가야 하므로 mepc += 4 필수
    csrr t0, mepc
    addi t0, t0, 4
    csrw mepc, t0

sbi_trap_return:
    # 레지스터 복원 및 복귀
    lw ra, 0(sp)
    lw t0, 4(sp)
    lw t1, 8(sp)
    lw t2, 12(sp)
    addi sp, sp, 16

    # 원래 커널/유저의 sp로 원상복구
    csrw mscratch, t0
    li t0, SBI_SCRATCH_BASE
    lw sp, 8(t0)                    # SBI 전용 안전 스택 포인터 복원
    csrr t0, mscratch

    mret

# ============================================================================
# [B] 비동기 M-mode 하드웨어 인터럽트 처리
# ============================================================================
handle_m_irq:
    li t0, SBI_SCRATCH_BASE
    lw t1, 4(t0)
    sw sp, 8(t0)                    # SBI 전용 안전 스택 포인터 저장

    li sp, 0x81000000            # SBI 전용 안전 스택 포인터 설정 (DRAM 최상단 임시 활용)

    # t0 복원
    csrr t0, mscratch
    csrw mscratch, zero

    addi sp, sp, -16
    sw ra, 0(sp)
    sw t0, 4(sp)
    sw t1, 8(sp)

    # 타이머 인터럽트 임시 클리어
    li t0, 0x02004000
    li t1, -1
    sw t1, 0(t0)
    sw t1, 4(t0)

    # 레지스터 복원
    lw ra, 0(sp)
    lw t0, 4(sp)
    lw t1, 8(sp)
    addi sp, sp, 16

    csrw mscratch, t0
    li t0, SBI_SCRATCH_BASE
    lw sp, 8(t0)                    # SBI 전용 안전 스택 포인터 복원
    csrr t0, mscratch               # t0 복원

    # 원래 실행 흐름으로 복귀
    mret



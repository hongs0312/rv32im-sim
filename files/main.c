#include <stdarg.h>

#define N 16

// 가속기 및 캐시 제어용 MMIO (OS 디바이스 드라이버 영역)
#define SYSTOLIC_STATUS (*(volatile unsigned int *)0x40000000)
#define SYSTOLIC_ADDR_A (*(volatile unsigned int *)0x40000004)
#define SYSTOLIC_ADDR_B (*(volatile unsigned int *)0x40000008)
#define SYSTOLIC_ADDR_C (*(volatile unsigned int *)0x4000000C)
#define SYSTOLIC_START (*(volatile unsigned int *)0x40000010)
#define HW_TIMER (*(volatile unsigned int *)0x40000020)
#define DCACHE_FLUSH (*(volatile unsigned int *)0x40000030)

// 💡 SBI 확장 서비스 번호 (RISC-V Legacy SBI 규격 기반)
#define SBI_SET_TIMER 0
#define SBI_CONSOLE_PUTCHAR 1
#define SBI_SHUTDOWN 8
#define SBI_REGISTER_IRQ 9 // OS 타이머 콜백 등록용 커스텀 SBI

// 인터럽트 발생 여부를 확인할 전역 플래그
volatile int timer_interrupt_fired = 0;

// ============================================================================
// 1. SBI 시스템 콜 인터페이스 (OS <-> 펌웨어 표준 관문)
// ============================================================================
static inline int sbi_call(int which, unsigned int arg0, unsigned int arg1, unsigned int arg2) {
    register unsigned int a0 asm("a0") = arg0;
    register unsigned int a1 asm("a1") = arg1;
    register unsigned int a2 asm("a2") = arg2;
    register unsigned int a7 asm("a7") = which;
    asm volatile("ecall"
                 : "=r"(a0)
                 : "r"(a0), "r"(a1), "r"(a2), "r"(a7)
                 : "memory");
    return a0;
}

void sbi_putchar(char c) {
    sbi_call(SBI_CONSOLE_PUTCHAR, (unsigned int)c, 0, 0);
}

void sbi_register_irq_handler(void (*handler)(void)) {
    sbi_call(SBI_REGISTER_IRQ, (unsigned int)handler, 0, 0);
}

void sbi_shutdown(int exit_code) {
    sbi_call(SBI_SHUTDOWN, (unsigned int)exit_code, 0, 0);
    while (1) {} // 하드웨어 전원이 꺼질 때까지 대기
}

// ============================================================================
// 3. 표준 입출력 (Standard I/O) 계층
// ============================================================================
void k_putchar(char c) {
    if (c == '\n') {
        sbi_putchar('\r');
    }
    sbi_putchar(c);
}

void print_int(unsigned int num) {
    char buf[16];
    int i = 0;
    if (num == 0) {
        k_putchar('0');
        return;
    }
    while (num > 0) {
        buf[i++] = (num % 10) + '0';
        num /= 10;
    }
    while (i > 0) {
        k_putchar(buf[--i]);
    }
}

void k_printf(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);

    while (*fmt != '\0') {
        if (*fmt == '%') {
            fmt++;
            if (*fmt == 'd' || *fmt == 'u') {
                unsigned int val = va_arg(args, unsigned int);
                print_int(val);
            }
            else if (*fmt == 's') {
                char *s = va_arg(args, char *);
                while (*s) {
                    k_putchar(*s++);
                }
            }
            else if (*fmt == 'c') {
                char c = (char)va_arg(args, int);
                k_putchar(c);
            }
        }
        else {
            k_putchar(*fmt);
        }
        fmt++;
    }

    va_end(args);
}

// ============================================================================
// 4. 벤치마크 및 메인 커널 루틴
// ============================================================================
void matmul_cpu(unsigned int *A, unsigned int *B, unsigned int *C) {
    for (int i = 0; i < N; i++) {
        for (int j = 0; j < N; j++) {
            unsigned int sum = 0;
            // 기존 코드
            for (int k = 0; k < N; k++) {
                sum += A[i * N + k] * B[k * N + j];
            }

            // 최적화된 코드: 루프 언롤링 적용
            // for (int k = 0; k < 16; k += 4) {
            //     sum += A[i*16 + k] * B[k*16 + j];
            //     sum += A[i*16 + (k+1)] * B[(k+1)*16 + j];
            //     sum += A[i*16 + (k+2)] * B[(k+2)*16 + j];
            //     sum += A[i*16 + (k+3)] * B[(k+3)*16 + j];
            // }
            C[i * N + j] = sum;
        }
    }
}

void matmul_systolic(unsigned int *A, unsigned int *B, unsigned int *C) {
    SYSTOLIC_ADDR_A = (unsigned int)A;
    SYSTOLIC_ADDR_B = (unsigned int)B;
    SYSTOLIC_ADDR_C = (unsigned int)C;
    SYSTOLIC_START = 1;
    while (SYSTOLIC_STATUS != 2)
    {
    }
}

int main() {
    k_printf("\n=========================== RISC-V Booting ===========================\n");
    k_printf("Standard I/O via SBI ecall is working perfectly!\n\n");
    k_printf("\nStarting RISC-V SoC Benchmark...\n");

    unsigned int A[N * N];
    unsigned int B[N * N];
    unsigned int C_cpu[N * N];
    unsigned int C_sys[N * N];

    for (int i = 0; i < N * N; i++) {
        A[i] = i % 10;
        B[i] = (i + 1) % 10;
    }

    unsigned int start_time, end_time;
    unsigned int cpu_cycles, sys_cycles;

    __asm__ __volatile__("fence" ::: "memory"); // 데이터 캐시 플러시

    start_time = HW_TIMER;
    matmul_systolic(A, B, C_sys);
    end_time = HW_TIMER;
    sys_cycles = end_time - start_time;

    __asm__ __volatile__("fence" ::: "memory"); // 💡 추가: C_sys를 읽기 전에 CPU 캐시 비우기!

    // k_printf("향상된 곱셈\n");

    start_time = HW_TIMER;
    matmul_cpu(A, B, C_cpu);
    end_time = HW_TIMER;
    cpu_cycles = end_time - start_time;

    unsigned int has_error = 0;
    for (int i = 0; i < N * N; i++) {
        if (C_cpu[i] != C_sys[i]) {
            has_error = 1;
            break;
        }
    }

    k_printf("======================== RISC-V SoC Benchmark ========================\n");
    k_printf("CPU Cycles : %u cycles\n", cpu_cycles);
    k_printf("SYS Cycles : %u cycles\n", sys_cycles);
    k_printf("Speedup    : %u times Approx. Fast!\n", cpu_cycles / sys_cycles);

    if (has_error != 0) {
        k_printf("\n[ERROR] OS Kernel Halt Reason: has_error = %d\n", has_error);
    } else {
        k_printf("\n[SUCCESS] All tests passed!\n");
    }

    // 💡 SBI 시스템 콜을 통해 우아하게 전원 종료 요청
    sbi_shutdown(has_error);

    return has_error;
}
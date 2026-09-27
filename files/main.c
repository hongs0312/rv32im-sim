#define N 16

#define SYSTOLIC_STATUS (*(volatile unsigned int*)0x80000000)
#define SYSTOLIC_ADDR_A (*(volatile unsigned int*)0x80000004)
#define SYSTOLIC_ADDR_B (*(volatile unsigned int*)0x80000008)
#define SYSTOLIC_ADDR_C (*(volatile unsigned int*)0x8000000C)
#define SYSTOLIC_START  (*(volatile unsigned int*)0x80000010)
#define HW_TIMER        (*(volatile unsigned int*)0x80000020) // 새로 추가한 타이머

void matmul_cpu(unsigned int* A, unsigned int* B, unsigned int* C) {
    for (int i = 0; i < N; i++) {
        for (int j = 0; j < N; j++) {
            unsigned int sum = 0;
            for (int k = 0; k < N; k++) {
                sum += A[i * N + k] * B[k * N + j];
            }
            C[i * N + j] = sum;
        }
    }
}

void matmul_systolic(unsigned int* A, unsigned int* B, unsigned int* C) {
    SYSTOLIC_ADDR_A = (unsigned int)A;
    SYSTOLIC_ADDR_B = (unsigned int)B;
    SYSTOLIC_ADDR_C = (unsigned int)C;
    SYSTOLIC_START = 1;
    while (SYSTOLIC_STATUS != 2) {}
}

int main() {
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

    // 가속기 연산 사이클 측정
    start_time = HW_TIMER;
    matmul_systolic(A, B, C_sys);
    end_time = HW_TIMER;
    sys_cycles = end_time - start_time;

    // MMIO 타이머로 정확한 CPU 연산 사이클 측정
    start_time = HW_TIMER;
    matmul_cpu(A, B, C_cpu);
    end_time = HW_TIMER;
    cpu_cycles = end_time - start_time;

    // 검증 로직
    unsigned int is_correct = 1;
    for (int i = 0; i < N * N; i++) {
        if (C_cpu[i] != C_sys[i]) {
            is_correct = 0; // 틀리면 0
            break;
        }
    }

    // 결과값을 레지스터에 싣고 ecall (is_correct를 a0에 넣어서 최적화 방지)
    asm volatile (
        "li a7, 93\n\t"       
        "mv a1, %0\n\t"       // a1: CPU 클럭
        "mv a2, %1\n\t"       // a2: 가속기 클럭
        "mv a0, %2\n\t"       // a0: 성공 여부 (1이면 성공, 0이면 실패)
        "ecall"
        : 
        : "r" (cpu_cycles), "r" (sys_cycles), "r" (is_correct)
        : "a0", "a1", "a2", "a7"
    );
    
    return 0;
}

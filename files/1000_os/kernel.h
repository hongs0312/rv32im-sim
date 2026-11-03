#pragma once
#include "common.h"

#define PROCS_MAX       8   // 최대 프로세스 개수
#define PROC_UNUSED     0   // 사용되지 않는 프로세스 구조체
#define PROC_RUNNABLE   1   // 실행 가능한(runnable) 프로세스
#define PROC_EXITED     2   // 종료된(exited) 프로세스

#define SATP_SV32 (1u << 31)
#define SSTATUS_SPIE (1 << 5) // sstatus 레지스터의 SPIE 비트 위치
#define SSTATUS_SUM  (1 << 18)

#define SCAUSE_ECALL 8

#define PAGE_V      (1 << 0)    // "Valid" 비트 (엔트리가 유효함을 의미)
#define PAGE_R      (1 << 1)    // 읽기 가능
#define PAGE_W      (1 << 2)    // 쓰기 가능
#define PAGE_X      (1 << 3)    // 실행 가능
#define PAGE_U      (1 << 4)    // 사용자 모드 접근 가능

// 애플리케이션 이미지의 기본 가상 주소입니다. 이는 `user.ld`에 정의된 시작 주소와 일치해야 합니다.
#define USER_BASE 0x1000000
#define FILES_MAX   2
#define DISK_MAX_SIZE     align_up(sizeof(struct file) * FILES_MAX, SECTOR_SIZE)
#define SECTOR_SIZE       512
#define VIRTQ_ENTRY_NUM   16
#define VIRTIO_DEVICE_BLK 2
#define VIRTIO_BLK_PADDR  0x10001000
#define VIRTIO_REG_MAGIC         0x00
#define VIRTIO_REG_VERSION       0x04
#define VIRTIO_REG_DEVICE_ID     0x08
#define VIRTIO_REG_PAGE_SIZE     0x28
#define VIRTIO_REG_QUEUE_SEL     0x30
#define VIRTIO_REG_QUEUE_NUM_MAX 0x34
#define VIRTIO_REG_QUEUE_NUM     0x38
#define VIRTIO_REG_QUEUE_PFN     0x40
#define VIRTIO_REG_QUEUE_READY   0x44
#define VIRTIO_REG_QUEUE_NOTIFY  0x50
#define VIRTIO_REG_DEVICE_STATUS 0x70
#define VIRTIO_REG_DEVICE_CONFIG 0x100
#define VIRTIO_STATUS_ACK       1
#define VIRTIO_STATUS_DRIVER    2
#define VIRTIO_STATUS_DRIVER_OK 4
#define VIRTQ_DESC_F_NEXT          1
#define VIRTQ_DESC_F_WRITE         2
#define VIRTQ_AVAIL_F_NO_INTERRUPT 1
#define VIRTIO_BLK_T_IN  0
#define VIRTIO_BLK_T_OUT 1

struct process {
    int pid;                // 프로세스 ID      
    int state;              // 프로세스 상태: PROC_UNUSED, PROC_RUNNABLE
    vaddr_t sp;             // 스택 포인터
    uint32_t *page_table;   // 페이지 테이블의 물리 주소
    uint8_t stack[8192];    // 커널 스택
};

struct sbiret {
    long error;
    long value;
};

// 트랩 프레임 구조체 정의
struct trap_frame {
    uint32_t ra;
    uint32_t gp;
    uint32_t tp;
    uint32_t t0;
    uint32_t t1;
    uint32_t t2;
    uint32_t t3;
    uint32_t t4;
    uint32_t t5;
    uint32_t t6;
    uint32_t a0;
    uint32_t a1;
    uint32_t a2;
    uint32_t a3;
    uint32_t a4;
    uint32_t a5;
    uint32_t a6;
    uint32_t a7;
    uint32_t s0;
    uint32_t s1;
    uint32_t s2;
    uint32_t s3;
    uint32_t s4;
    uint32_t s5;
    uint32_t s6;
    uint32_t s7;
    uint32_t s8;
    uint32_t s9;
    uint32_t s10;
    uint32_t s11;
    uint32_t sp;
} __attribute__((packed));

// Virtqueue Descriptor Table entry.
struct virtq_desc {
    uint64_t addr;
    uint32_t len;
    uint16_t flags;
    uint16_t next;
} __attribute__((packed));

// Virtqueue Available Ring.
struct virtq_avail {
    uint16_t flags;
    uint16_t index;
    uint16_t ring[VIRTQ_ENTRY_NUM];
} __attribute__((packed));

// Virtqueue Used Ring entry.
struct virtq_used_elem {
    uint32_t id;
    uint32_t len;
} __attribute__((packed));

// Virtqueue Used Ring.
struct virtq_used {
    uint16_t flags;
    uint16_t index;
    struct virtq_used_elem ring[VIRTQ_ENTRY_NUM];
} __attribute__((packed));

// Virtqueue.
struct virtio_virtq {
    struct virtq_desc descs[VIRTQ_ENTRY_NUM];
    struct virtq_avail avail;
    struct virtq_used used __attribute__((aligned(PAGE_SIZE)));
    int queue_index;
    volatile uint16_t *used_index;
    uint16_t last_used_index;
} __attribute__((packed));

// Virtio-blk request.
struct virtio_blk_req {
    uint32_t type;
    uint32_t reserved;
    uint64_t sector;
    uint8_t data[512];
    uint8_t status;
} __attribute__((packed));

struct tar_header {
    char name[100];
    char mode[8];
    char uid[8];
    char gid[8];
    char size[12];
    char mtime[12];
    char checksum[8];
    char type;
    char linkname[100];
    char magic[6];
    char version[2];
    char uname[32];
    char gname[32];
    char devmajor[8];
    char devminor[8];
    char prefix[155];
    char padding[12];
    char data[];      // Array pointing to the data area following the header
                      // (flexible array member)
} __attribute__((packed));

struct file {
    bool in_use;
    char name[100];
    char data[1024];
    size_t size;
};

// CSR 레지스터를 읽는 매크로 정의
#define READ_CSR(reg)                                                          \
    ({                                                                         \
        unsigned long __tmp;                                                   \
        __asm__ __volatile__("csrr %0, " #reg : "=r"(__tmp));                  \
        __tmp;                                                                 \
    })

// CSR 레지스터에 값을 쓰는 매크로 정의
#define WRITE_CSR(reg, value)                                                  \
    do {                                                                       \
        uint32_t __tmp = (value);                                              \
        __asm__ __volatile__("csrw " #reg ", %0" ::"r"(__tmp));                \
    } while (0)

// 커널 패닉 매크로 정의
// do while(0) 구조를 사용하여 매크로가 단일 문장처럼 동작하도록 함
// ##__VA_ARGS__는 가변 인자 매크로에서 인자가 없을 때 발생하는 문제를 방지하기 위해 사용
#define PANIC(fmt, ...)                                                         \
    do {                                                                        \
        printf("PANIC: %s:%d: " fmt "\n", __FILE__, __LINE__, ##__VA_ARGS__);   \
        while (1) {}                                                            \
    } while (0)


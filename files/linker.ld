OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS
{
    /* SBI가 점프하는 도착지 */
    . = 0x80200000;

    .text : {
        *(.text.init)
        
        /* 그 다음 나머지 C 코드들의 기계어를 배치 */
        *(.text .text.*)
    }

    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { 
        *(.bss .bss.*) 
        *(COMMON)
    }
}
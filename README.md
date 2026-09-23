# RV32IM Cycle-Accurate SoC Simulator & Accelerator

A lightweight yet highly precise **RISC-V (RV32IM) System-on-Chip (SoC) simulator** written in Rust.

It automatically compiles C source files using the RISC-V GNU Toolchain, extracts raw binary code, loads it into a simulated 16MB DRAM, and executes instructions with cycle-by-cycle hardware state inspection. 

Unlike simple instruction-level emulators, this project accurately models **microarchitectural behaviors**, including a 5-stage pipeline, L1 Cache state machines, System Bus arbitration, and a custom **16x16 Systolic Array Accelerator** mapped via MMIO.

---

## 🚀 Key Architectural Features

* **Advanced 5-Stage Pipeline Core (IF, ID, EX, MEM, WB)**
  * **Reverse-Latch Update:** Accurately prevents race conditions across pipeline stages using a hardware-true reverse clock update loop.
  * **Robust Hazard Unit:** Handles Load-Use stalls and control hazards smoothly. Jump/Branch signals correctly flush the pipeline and immediately abort ongoing fetch transactions.
* **Realistic Memory Hierarchy & System Bus**
  * **System Bus Arbiter:** CPU (L1 Cache) and Accelerator (DMA) share a single system bus, allowing true hardware **Bus Contention (Memory Wall)** emulation.
  * **L1 Data & Instruction Caches:** Direct-Mapped, Write-Back, Write-Allocate policies with 16-byte block size and structural bus stalling (5-cycle latency per block).
* **Systolic Array Accelerator (Matrix Multiplication)**
  * Integrated via MMIO (`0x8000_0000`), executing 16x16 matrix multiplications using wavefront streaming through 2D Processing Elements (PEs).

---

## 🛠 Prerequisites

Ensure you have the following installed in your environment:

* **Rust** (Cargo)
* **RISC-V GNU Toolchain** (`riscv64-unknown-elf-gcc`, `riscv64-unknown-elf-objcopy`)

---

## 💻 Quick Start & Usage

Run C programs directly on the simulator using `cargo run`:

```bash
cargo run -- --source <PATH_TO_C_FILE> [OPTIONS]
```

Options

| Option | Long Flag     | Description                                         | Default        |
| ------ | ------------- | --------------------------------------------------- | -------------- |
| `-s`   | `--source`    | Path to the target C source file                    | `files/main.c` |
| `-p`   | `--pipeline`  | Enable 5-stage pipelining execution                 | `false`        |
| `-v`   | `--verbose`   | Enable cycle-by-cycle trace log (PC, Inst, sp, a0)  | `false`        |
| `-m`   | `--max-steps` | Maximum instruction limit to prevent infinite loops | `100000`       |

⚡ Systolic Array & Memory Wall

To accelerate matrix multiplications, this simulator includes a custom 16x16
Systolic Array module.

Thanks to the unified System Bus architecture, the DMA Controller must compete
with the CPU for memory bandwidth. This provides a highly realistic benchmark,
perfectly reflecting the Memory Wall latency penalty present in real-world
silicon chips.

  - DMA Controller: Automatically fetches matrix data from DRAM handling bus
    wait cycles and bursts.
  - Performance: Achieves a realistic ~13x speedup on 16x16 matrix
    multiplication compared to pure RV32IM CPU execution (approx. 3,927 cycles
    vs. 52,348 cycles).

MMIO Memory Map

The accelerator is mapped to the >= 0x8000_0000 memory address space.

| Address       | Register | Access | Description                              |
| ------------- | -------- | ------ | ---------------------------------------- |
| `0x8000_0000` | `STATUS` | R/W    | 0: Idle, 1: Running, 2: Done             |
| `0x8000_0004` | `ADDR_A` | W      | Base address of Matrix A in DRAM         |
| `0x8000_0008` | `ADDR_B` | W      | Base address of Matrix B in DRAM         |
| `0x8000_000C` | `ADDR_C` | W      | Base address to store Result Matrix C    |
| `0x8000_0010` | `START`  | W      | Write `1` to trigger DMA and computation |
| `0x8000_0020` | `TIMER`  | R      | Global hardware cycle counter            |

C-Code Example
```C
#define SYSTOLIC_STATUS (*(volatile unsigned int*)0x80000000)
#define SYSTOLIC_ADDR_A (*(volatile unsigned int*)0x80000004)
#define SYSTOLIC_ADDR_B (*(volatile unsigned int*)0x80000008)
#define SYSTOLIC_ADDR_C (*(volatile unsigned int*)0x8000000C)
#define SYSTOLIC_START  (*(volatile unsigned int*)0x80000010)

void matmul_systolic(unsigned int* A, unsigned int* B, unsigned int* C) {
    // Pass matrix pointers via MMIO
    SYSTOLIC_ADDR_A = (unsigned int)A;
    SYSTOLIC_ADDR_B = (unsigned int)B;
    SYSTOLIC_ADDR_C = (unsigned int)C;
    
    // Trigger DMA and compute
    SYSTOLIC_START = 1;
    
    // Wait for the accelerator to finish
    while (SYSTOLIC_STATUS != 2) {} 
}
```

📚 Reference

  - https://github.com/russross/riscv-card


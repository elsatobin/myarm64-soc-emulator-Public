ENTRY(_start)

SECTIONS {
    . = 0x40000000;

    .text : ALIGN(16) { *(.text*) }
    .rodata : ALIGN(16) { *(.rodata*) }
    .data : ALIGN(16) { *(.data*) }

    .bss (NOLOAD) : ALIGN(16) {
        *(.bss*)
        *(COMMON)
    }
}

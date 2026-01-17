.global _start
_start:
    mov     x0, #100
    mov     x1, #23
    add     x2, x0, x1 // x2 = 123

    mov     x3, #0xdead
    movk    x3, #0xbeef, lsl #16 // x3 = 0xbeefdead

    add     x4, x2, x3 // x4 = 123 + 0xbeefdead

    wfi

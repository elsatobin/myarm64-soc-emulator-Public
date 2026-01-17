.global _start
_start:
    mov     x10, #0x4000
    lsl     x10, x10, #16 // x10 = 0x40000000

    mov     w0, #41
    mov     w1, #1
    add     w2, w0, w1 // w2 = 42
    str     w2, [x10] // store 42 to [0x40000000]

    wfi

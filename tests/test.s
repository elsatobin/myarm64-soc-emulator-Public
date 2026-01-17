.global _start
_start:
    mov     x1, #0x0900
    lsl     x1, x1, #16 // x1 = 0x09000000

    // write 'H'
    mov     w0, #'H'
    str     w0, [x1]

    // write 'i'
    mov     w0, #'i'
    str     w0, [x1]

    // write newline
    mov     w0, #'\n'
    str     w0, [x1]

    // halt
    hlt     #0

hang:
    b       hang

; get nth fibonacci number
        DEVICE NOSLOT64K
        ORG $0000

        ld sp, 0xFFFF

        ld a, 6         ; find 7th fibonacci number: 8
        call fib
        halt
fib:

        cp 2            ; n < 2?
        jr c, fib_done  ; then n is base case, so we're done

        push af         ; save n
        dec a
        call fib        ; a now has fib(n-1)
        pop bc          ; restore n to b

        push af;        ; save fib(n-1)
        ld a, b         ; load n
        sub 2
        call  fib       ; fib(n-2)

        pop bc          ; restore result of fib(n-1) to b
        add a, b        ; fib(n-1) + fib(n-2)
fib_done:
        ret

        halt
        SAVEBIN "fib.bin", $0000, $-$0000

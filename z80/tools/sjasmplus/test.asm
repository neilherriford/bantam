; the smallest program
        DEVICE NOSLOT64K
        ORG $0000

        nop
        halt

        SAVEBIN "test.bin", $0000, $-$0000

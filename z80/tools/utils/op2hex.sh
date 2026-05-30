#!/bin/bash
# Given three arguments, two bits, three bits and three bits
# it prints out the hex version of the opcode
#
# Example:
# ./op2hex.sh 2 0 6
# 0x86
opcode=$(( ($1 << 6) | ($2 << 3) | $3 ))
printf "0x%02X\n" $opcode

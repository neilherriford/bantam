use crate::{
    bus::Bus,
    decode,
    flags::{self, bit_is_set, is_set},
    registers::{self, Registers},
    set_bits,
};

fn wrapping_offset_u16(addend: u16, signed_augend: u8) -> u16 {
    let signed_offset = (signed_augend as i8) as i16;
    addend.wrapping_add_signed(signed_offset)
}

pub struct Cpu<B: Bus> {
    pub registers: Registers,
    pub bus: B,
}

impl<B> Cpu<B>
where
    B: Bus,
{
    pub fn new(registers: Registers, bus: B) -> Self {
        Self { registers, bus }
    }

    #[inline]
    fn advance(&mut self) {
        self.registers.increment_pc();
        self.registers.increment_r();
    }

    #[inline]
    fn read_indexed_register(&mut self, index: u8) -> u8 {
        match index {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => self.bus.read8(self.registers.hl()),
            7 => self.registers.a,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn write_indexed_register(&mut self, index: u8, value: u8) {
        match index {
            0 => self.registers.b = value,
            1 => self.registers.c = value,
            2 => self.registers.d = value,
            3 => self.registers.e = value,
            4 => self.registers.h = value,
            5 => self.registers.l = value,
            6 => self.bus.write8(self.registers.hl(), value),
            7 => self.registers.a = value,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn add_u8_and_set_flags(&mut self, augend: u8, addend: u8, use_carry: bool) -> u8 {
        let carry_in: u8 = if use_carry && bit_is_set(self.registers.f, flags::CARRY) {
            1
        } else {
            0
        };

        let full = augend as u16 + addend as u16 + carry_in as u16;
        let sum = full as u8;

        // If the addend and augend signs are the same but the sign of
        // the sum is different, then we overflowed
        let overflow = ((addend ^ sum) & (augend ^ sum) & 0x80) != 0;
        let half_carry = (augend & 0x0F) + (addend & 0x0F) + carry_in > 0x0F;

        self.registers.f = set_bits!(
            self.registers.f,
            flags::CARRY =>  full > 0xFF,
            flags::ADD_SUBTRACT => false,
            flags::PARITY_OVERFLOW => overflow,
            flags::HALF_CARRY => half_carry,
            flags::ZERO => sum == 0,
            flags::SIGN => is_set(sum, 0x80)
        );

        sum
    }

    #[inline]
    fn add_u8_by_index_and_set_flags(
        &mut self,
        augend_index: u8,
        addend_index: u8,
        use_carry: bool,
    ) -> u8 {
        let augend = self.read_indexed_register(augend_index);
        let addend = self.read_indexed_register(addend_index);

        self.add_u8_and_set_flags(augend, addend, use_carry)
    }

    #[inline]
    fn add_u16_and_set_flags(&mut self, augend: u16, addend: u16, use_carry: bool) -> u16 {
        let carry_in: u8 = if use_carry && bit_is_set(self.registers.f, flags::CARRY) {
            1
        } else {
            0
        };

        let full = augend as u32 + addend as u32 + carry_in as u32;
        let sum = full as u16;
        let half_carry = (augend & 0x0FFF) + (addend & 0x0FFF) + (carry_in as u16) > 0x0FFF;

        self.registers.f = set_bits!(
            self.registers.f,
            flags::CARRY => full > 0xFFFF,
            flags::ADD_SUBTRACT => false,
            flags::HALF_CARRY => half_carry,
        );

        sum
    }

    #[inline]
    fn subtract_u8_and_set_flags(&mut self, minuend: u8, subtrahend: u8, use_borrow: bool) -> u8 {
        let borrow_in: u8 = if use_borrow && bit_is_set(self.registers.f, flags::CARRY) {
            1
        } else {
            0
        };

        let full = (minuend as u16).wrapping_sub(subtrahend as u16 + borrow_in as u16);
        let difference = full as u8;

        // If the inputs have different signs, but the difference
        // has the same sign, then we overflowed
        let overflow = ((minuend ^ subtrahend) & (minuend ^ difference) & 0x80) != 0;
        let half_carry = (minuend & 0x0F) < ((subtrahend & 0x0F) + borrow_in);

        self.registers.f = set_bits!(
            self.registers.f,
            flags::CARRY => full > 0xFF,
            flags::ADD_SUBTRACT => true,
            flags::PARITY_OVERFLOW => overflow,
            flags::HALF_CARRY => half_carry,
            flags::ZERO => difference == 0,
            flags::SIGN => is_set(difference, 0x80),
        );

        difference
    }

    #[inline]
    fn subtract_u8_by_index_and_set_flags(
        &mut self,
        minuend_index: u8,
        subtrahend_index: u8,
        use_carry: bool,
    ) -> u8 {
        let minuend = self.read_indexed_register(minuend_index);
        let subtrahend = self.read_indexed_register(subtrahend_index);

        self.subtract_u8_and_set_flags(minuend, subtrahend, use_carry)
    }

    pub fn run(&mut self) {
        while !self.registers.halted {
            self.step();
        }
    }

    pub fn step(&mut self) {
        match decode::into_group_and_operands(self.bus.read8(self.registers.pc)) {
            (0, 0, 0) => {
                // NOP
                self.advance();
            }
            (1, 6, 6) => {
                // HALT
                self.registers.increment_r();
                self.registers.halted = true
            }
            (3, 6, 3) => {
                // DI
                self.advance();
                self.registers.iff1 = false;
                self.registers.iff2 = false;
            }
            (3, 7, 3) => {
                // EI
                self.advance();
                self.registers.iff1 = true;
                self.registers.iff2 = true;
            }
            (1, dest, src) => {
                // LD r, r'
                // LD r, (HL)
                self.advance();
                let value = self.read_indexed_register(src);
                self.write_indexed_register(dest, value);
            }
            (0, register, 6) => {
                // LD r, n
                self.advance();
                let value = self.read_and_advance();
                self.write_indexed_register(register, value);
            }
            (0, op @ 0..=3, 2) => {
                self.advance();
                let operation = op & 1;
                const BC: u8 = 0;
                const WRITE: u8 = 0;

                let address = if op & 2 == BC {
                    self.registers.bc()
                } else {
                    self.registers.de()
                };

                if operation == WRITE {
                    // LD (BC), A
                    // LD (DE), A
                    self.bus.write8(address, self.registers.a);
                } else {
                    // LD A, (BC)
                    // LD A, (DE)
                    self.registers.a = self.bus.read8(address);
                }
            }
            (0, op @ 4..=7, 2) => {
                // LD (nn), HL
                // LD HL, (nn)
                // LD (nn), A
                // LD A, (nn)
                self.advance();
                let operation = op & 1;
                const HL: u8 = 0;
                const WRITE: u8 = 0;

                let low = self.read_and_advance();
                let high = self.read_and_advance();

                let address = (high as u16) << 8 | low as u16;

                if op & 2 == HL {
                    if operation == WRITE {
                        // LD (nn), HL
                        self.bus.write8(address, self.registers.l);
                        self.bus.write8(address.wrapping_add(1), self.registers.h);
                    } else {
                        // LD HL, (nn)
                        self.registers.l = self.bus.read8(address);
                        self.registers.h = self.bus.read8(address.wrapping_add(1));
                    }
                } else if operation == WRITE {
                    // LD (nn), A
                    self.bus.write8(address, self.registers.a);
                } else {
                    //  LD A, (nn)
                    self.registers.a = self.bus.read8(address);
                }
            }
            (0, pair @ (0 | 2 | 4 | 6), 1) => {
                // LD BC, nn
                // LD DE, nn
                // LD HL, nn
                // LD SP, nn
                self.advance();
                let low = self.read_and_advance();
                let high = self.read_and_advance();

                let value = ((high as u16) << 8) | low as u16;
                match pair {
                    0 => self.registers.set_bc(value),
                    2 => self.registers.set_de(value),
                    4 => self.registers.set_hl(value),
                    6 => self.registers.sp = value,
                    _ => unreachable!(),
                }
            }
            (3, pair @ (0 | 2 | 4 | 6), 5) => {
                // PUSH rr
                self.advance();
                let (high, low) = match pair {
                    0 => (self.registers.b, self.registers.c),
                    2 => (self.registers.d, self.registers.e),
                    4 => (self.registers.h, self.registers.l),
                    6 => (self.registers.a, self.registers.f),
                    _ => unreachable!(),
                };

                self.registers.sp = self.registers.sp.wrapping_sub(1);
                self.bus.write8(self.registers.sp, high);
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                self.bus.write8(self.registers.sp, low);
            }
            (3, pair @ (0 | 2 | 4 | 6), 1) => {
                // POP rr
                self.advance();
                let low = self.bus.read8(self.registers.sp);
                self.registers.sp = self.registers.sp.wrapping_add(1);
                let high = self.bus.read8(self.registers.sp);
                self.registers.sp = self.registers.sp.wrapping_add(1);

                let value = ((high as u16) << 8) | low as u16;
                match pair {
                    0 => self.registers.set_bc(value),
                    2 => self.registers.set_de(value),
                    4 => self.registers.set_hl(value),
                    6 => self.registers.set_af(value),
                    _ => unreachable!(),
                }
            }
            (0, register, 4) => {
                // INC r
                self.advance();
                let before = self.read_indexed_register(register);
                let after = before.wrapping_add(1);
                self.write_indexed_register(register, after);

                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::ADD_SUBTRACT => false,
                    flags::PARITY_OVERFLOW => before == 0x7F,
                    flags::HALF_CARRY => is_set(before, 0x0F),
                    flags::ZERO => after == 0,
                    flags::SIGN => is_set(after, 0x80),
                );
            }
            (0, register, 5) => {
                // DEC r
                self.advance();
                let before = self.read_indexed_register(register);
                let after = before.wrapping_sub(1);
                self.write_indexed_register(register, after);

                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::ADD_SUBTRACT => true,
                    flags::PARITY_OVERFLOW => before == 0x80,
                    flags::HALF_CARRY => before & 0x0F == 0x00,
                );

                self.set_zero_and_sign_flags_for_u8(after);
            }
            (2, 0, register) => {
                // ADD A. r
                self.advance();
                let sum = self.add_u8_by_index_and_set_flags(registers::index::A, register, false);
                self.registers.a = sum;
            }
            (2, 1, register) => {
                // ADC A. r
                self.advance();
                let sum = self.add_u8_by_index_and_set_flags(registers::index::A, register, true);
                self.registers.a = sum;
            }
            (3, 0, 6) => {
                // ADD A, n
                self.advance();
                let addend = self.read_and_advance();

                let sum = self.add_u8_and_set_flags(self.registers.a, addend, false);
                self.registers.a = sum
            }
            (3, 1, 6) => {
                // ADC A, n
                self.advance();
                let addend = self.read_and_advance();

                let sum = self.add_u8_and_set_flags(self.registers.a, addend, true);
                self.registers.a = sum
            }
            (2, 2, register) => {
                // SUB A, r
                self.advance();
                let difference =
                    self.subtract_u8_by_index_and_set_flags(registers::index::A, register, false);
                self.registers.a = difference;
            }
            (2, 3, register) => {
                // SBC A, r
                self.advance();
                let difference =
                    self.subtract_u8_by_index_and_set_flags(registers::index::A, register, true);
                self.registers.a = difference;
            }
            (3, 2, 6) => {
                // SUB A, n
                self.advance();
                let subtrahend = self.read_and_advance();

                let difference =
                    self.subtract_u8_and_set_flags(self.registers.a, subtrahend, false);
                self.registers.a = difference
            }
            (3, 3, 6) => {
                // SBC A, n
                self.advance();
                let subtrahend = self.read_and_advance();

                let difference = self.subtract_u8_and_set_flags(self.registers.a, subtrahend, true);
                self.registers.a = difference
            }
            (2, 7, register) => {
                // CP r
                self.advance();
                self.subtract_u8_by_index_and_set_flags(registers::index::A, register, false);
            }
            (3, 7, 6) => {
                // CP n
                self.advance();
                let subtrahend = self.read_and_advance();
                self.subtract_u8_and_set_flags(self.registers.a, subtrahend, false);
            }
            (2, 4, register) => {
                // AND r
                self.advance();
                let value = self.read_indexed_register(register);
                self.registers.a &= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => true);
            }
            (2, 6, register) => {
                // OR r
                self.advance();
                let value = self.read_indexed_register(register);
                self.registers.a |= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => false);
            }
            (2, 5, register) => {
                // XOR r
                self.advance();
                let value = self.read_indexed_register(register);
                self.registers.a ^= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => false);
            }
            (3, 4, 6) => {
                // AND n
                self.advance();
                let value = self.read_and_advance();

                self.registers.a &= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => true);
            }
            (3, 6, 6) => {
                // OR n
                self.advance();
                let value = self.read_and_advance();

                self.registers.a |= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => false);
            }
            (3, 5, 6) => {
                // XOR n
                self.advance();
                let value = self.read_and_advance();

                self.registers.a ^= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.f = set_bits!(self.registers.f, flags::HALF_CARRY => false);
            }
            (0, pair @ (1 | 3 | 5 | 7), 1) => {
                // ADD HL, s
                self.advance();
                let addend = match pair {
                    1 => self.registers.bc(),
                    3 => self.registers.de(),
                    5 => self.registers.hl(),
                    7 => self.registers.sp,
                    _ => unreachable!(),
                };

                let sum = self.add_u16_and_set_flags(self.registers.hl(), addend, false);
                self.registers.set_hl(sum);
            }
            (3, 0, 3) => {
                // JP nn
                self.advance();
                let address = self.read_16_and_advance();
                self.registers.pc = address;
            }
            (0, 3, 0) => {
                // JR e
                self.advance();
                let offset = self.read_and_advance();
                self.registers.pc = wrapping_offset_u16(self.registers.pc, offset);
            }
            (0, 4, 0) => {
                // JR NZ, e
                self.advance();
                let offset = self.read_and_advance();
                if !bit_is_set(self.registers.f, flags::ZERO) {
                    self.registers.pc = wrapping_offset_u16(self.registers.pc, offset);
                }
            }
            (0, 5, 0) => {
                // JR Z, e
                self.advance();
                let offset = self.read_and_advance();
                if bit_is_set(self.registers.f, flags::ZERO) {
                    self.registers.pc = wrapping_offset_u16(self.registers.pc, offset);
                }
            }
            (0, 6, 0) => {
                // JR NC, e
                self.advance();
                let offset = self.read_and_advance();
                if !bit_is_set(self.registers.f, flags::CARRY) {
                    self.registers.pc = wrapping_offset_u16(self.registers.pc, offset);
                }
            }
            (0, 7, 0) => {
                // JR C, e
                self.advance();
                let offset = self.read_and_advance();
                if bit_is_set(self.registers.f, flags::CARRY) {
                    self.registers.pc = wrapping_offset_u16(self.registers.pc, offset);
                }
            }
            (3, 5, 1) => {
                // JP (HL)
                self.advance();
                self.registers.pc = self.registers.hl()
            }
            (3, condition, 2) => {
                // JP c nn
                self.advance();
                let jump_address = self.read_16_and_advance();

                let should_jump = match condition {
                    0 => !bit_is_set(self.registers.f, flags::ZERO),
                    1 => bit_is_set(self.registers.f, flags::ZERO),
                    2 => !bit_is_set(self.registers.f, flags::CARRY),
                    3 => bit_is_set(self.registers.f, flags::CARRY),
                    4 => !bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    5 => bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    6 => !bit_is_set(self.registers.f, flags::SIGN),
                    7 => bit_is_set(self.registers.f, flags::SIGN),
                    _ => unreachable!(),
                };

                if should_jump {
                    self.registers.pc = jump_address;
                }
            }
            (3, 1, 5) => {
                // CALL nn
                self.advance();
                let address = self.read_16_and_advance();
                self.call(address);
            }
            (3, condition, 4) => {
                // CALL c nn
                self.advance();
                let address = self.read_16_and_advance();
                let should_call = match condition {
                    0 => !bit_is_set(self.registers.f, flags::ZERO),
                    1 => bit_is_set(self.registers.f, flags::ZERO),
                    2 => !bit_is_set(self.registers.f, flags::CARRY),
                    3 => bit_is_set(self.registers.f, flags::CARRY),
                    4 => !bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    5 => bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    6 => !bit_is_set(self.registers.f, flags::SIGN),
                    7 => bit_is_set(self.registers.f, flags::SIGN),
                    _ => unreachable!(),
                };

                if should_call {
                    self.call(address);
                }
            }
            (3, 1, 1) => {
                // RET
                self.advance();
                self.ret();
            }
            (3, condition, 0) => {
                // RET c
                self.advance();
                let should_ret = match condition {
                    0 => !bit_is_set(self.registers.f, flags::ZERO),
                    1 => bit_is_set(self.registers.f, flags::ZERO),
                    2 => !bit_is_set(self.registers.f, flags::CARRY),
                    3 => bit_is_set(self.registers.f, flags::CARRY),
                    4 => !bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    5 => bit_is_set(self.registers.f, flags::PARITY_OVERFLOW),
                    6 => !bit_is_set(self.registers.f, flags::SIGN),
                    7 => bit_is_set(self.registers.f, flags::SIGN),
                    _ => unreachable!(),
                };

                if should_ret {
                    self.ret();
                }
            }
            (3, address_shorthand, 7) => {
                // RST p
                self.advance();
                let address = match address_shorthand {
                    0 => 0x00,
                    1 => 0x08,
                    2 => 0x10,
                    3 => 0x18,
                    4 => 0x20,
                    5 => 0x28,
                    6 => 0x30,
                    7 => 0x38,
                    _ => unreachable!(),
                };
                self.call(address);
            }
            (3, 3, 3) => {
                // IN A, (n)
                self.advance();
                let port = (self.registers.a as u16) << 8 | (self.read_and_advance() as u16);
                self.registers.a = self.bus.read_io(port);
            }
            (3, 2, 3) => {
                // OUT n, A
                self.advance();
                let port = (self.registers.a as u16) << 8 | (self.read_and_advance() as u16);
                self.bus.write_io(port, self.registers.a);
            }
            (3, 5, 3) => {
                // EX DE, HL
                self.advance();
                let hl = self.registers.hl();
                self.registers.set_hl(self.registers.de());
                self.registers.set_de(hl);
            }
            (0, 1, 0) => {
                // EX AF, AF'
                self.advance();
                let af = self.registers.af();
                self.registers.set_af(self.registers.af_alt());
                self.registers.set_af_alt(af);
            }
            (3, 3, 1) => {
                // EXX
                self.advance();
                let bc = self.registers.bc();
                let de = self.registers.de();
                let hl = self.registers.hl();

                self.registers.set_bc(self.registers.bc_alt());
                self.registers.set_de(self.registers.de_alt());
                self.registers.set_hl(self.registers.hl_alt());
                self.registers.set_bc_alt(bc);
                self.registers.set_de_alt(de);
                self.registers.set_hl_alt(hl);
            }
            (3, 4, 3) => {
                // EX (SP), HL
                self.advance();
                let l = self.registers.l;
                let h = self.registers.h;

                let high_offset = self.registers.sp.wrapping_add(1);
                self.registers.l = self.bus.read8(self.registers.sp);
                self.registers.h = self.bus.read8(high_offset);

                self.bus.write8(self.registers.sp, l);
                self.bus.write8(high_offset, h);
            }
            (0, 0, 7) => {
                // RLCA
                self.advance();
                self.registers.a = self.registers.a.rotate_left(1);
                self.set_rotate_flags(is_set(self.registers.a, 1));
            }
            (0, 1, 7) => {
                // RRCA
                self.advance();
                self.registers.a = self.registers.a.rotate_right(1);
                self.set_rotate_flags(is_set(self.registers.a, 0x80));
            }
            (0, 2, 7) => {
                // RLA
                self.advance();
                let old_carry = bit_is_set(self.registers.f, flags::CARRY);
                let new_carry = is_set(self.registers.a, 0x80);

                self.registers.a <<= 1;
                self.set_rotate_flags(new_carry);
                if old_carry {
                    self.registers.a |= 1
                }
            }
            (0, 3, 7) => {
                // RRA
                self.advance();
                let old_carry = bit_is_set(self.registers.f, flags::CARRY);
                let new_carry = is_set(self.registers.a, 1);
                self.registers.a >>= 1;
                self.set_rotate_flags(new_carry);
                if old_carry {
                    self.registers.a |= 0x80
                }
            }
            (0, 4, 7) => {
                // DAA
                self.advance();
                let was_subtract = bit_is_set(self.registers.f, flags::ADD_SUBTRACT);
                let diff = self.bcd_difference();
                let new_half_carry = self.bcd_new_half_carry();
                let new_carry = self.bcd_new_carry();

                if was_subtract {
                    self.registers.a = self.registers.a.wrapping_sub(diff);
                } else {
                    self.registers.a = self.registers.a.wrapping_add(diff);
                }

                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::CARRY => new_carry,
                    flags::HALF_CARRY => new_half_carry,
                    flags::PARITY_OVERFLOW => self.registers.a.count_ones().is_multiple_of(2),
                    flags::ZERO => self.registers.a == 0,
                    flags::SIGN => bit_is_set(self.registers.a, 7),
                )
            }
            (0, 5, 7) => {
                // CPL
                self.advance();
                self.registers.a = !self.registers.a;
                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::ADD_SUBTRACT => true,
                    flags::HALF_CARRY => true,
                )
            }
            (0, 6, 7) => {
                // SCF
                self.advance();

                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::ADD_SUBTRACT => false,
                    flags::HALF_CARRY => false,
                    flags::CARRY => true,
                )
            }
            (0, 7, 7) => {
                // CCF
                self.advance();

                let old_carry = bit_is_set(self.registers.f, flags::CARRY);
                self.registers.f = set_bits!(
                    self.registers.f,
                    flags::ADD_SUBTRACT => false,
                    flags::HALF_CARRY => old_carry,
                    flags::CARRY => !old_carry,
                )
            }
            (0, 2, 0) => {
                // DJNZ
                self.advance();
                let offset = self.read_and_advance();
                self.registers.b = self.registers.b.wrapping_sub(1);
                if self.registers.b != 0 {
                    self.registers.pc =
                        self.registers.pc.wrapping_add_signed((offset as i8) as i16);
                }
            }
            (3, 1, 3) => {
                // CB Prefix
                self.advance();
                let opcode = self.read_and_advance();

                match decode::into_group_and_operands(opcode) {
                    (0, 0, register) => {
                        // RLC r
                        let value = self.read_indexed_register(register);
                        let high_bit = bit_is_set(value, 7);
                        let rotated = value.rotate_left(1);
                        self.write_indexed_register(register, rotated);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(rotated, 7),
                            flags::ZERO => rotated == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => rotated.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => high_bit,
                        );
                    }
                    (0, 1, register) => {
                        // RRC r
                        let value = self.read_indexed_register(register);
                        let low_bit = bit_is_set(value, 0);
                        let rotated = value.rotate_right(1);
                        self.write_indexed_register(register, rotated);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(rotated, 7),
                            flags::ZERO => rotated == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => rotated.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => low_bit,
                        );
                    }
                    (0, 2, register) => {
                        // RL r
                        let value = self.read_indexed_register(register);
                        let high_bit = bit_is_set(value, 7);
                        let new_low_bit = if bit_is_set(self.registers.f, flags::CARRY) {
                            1
                        } else {
                            0
                        };
                        let rotated = (value << 1) | new_low_bit;
                        self.write_indexed_register(register, rotated);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(rotated, 7),
                            flags::ZERO => rotated == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => rotated.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => high_bit,
                        );
                    }
                    (0, 3, register) => {
                        // RR r
                        let value = self.read_indexed_register(register);
                        let low_bit = bit_is_set(value, 0);
                        let new_high_bit = if bit_is_set(self.registers.f, flags::CARRY) {
                            0x80
                        } else {
                            0
                        };
                        let rotated = (value >> 1) | new_high_bit;
                        self.write_indexed_register(register, rotated);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(rotated, 7),
                            flags::ZERO => rotated == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => rotated.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => low_bit,
                        );
                    }
                    (0, 4, register) => {
                        // SLA r
                        let value = self.read_indexed_register(register);
                        let high_bit = bit_is_set(value, 7);
                        let shifted = value << 1;
                        self.write_indexed_register(register, shifted);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(shifted, 7),
                            flags::ZERO => shifted == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => shifted.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => high_bit,
                        );
                    }
                    (0, 5, register) => {
                        // SRA r
                        let value = self.read_indexed_register(register);
                        let high_bit = 0x80 & value;
                        let low_bit = bit_is_set(value, 0);
                        let shifted = (value >> 1) | high_bit;
                        self.write_indexed_register(register, shifted);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => bit_is_set(shifted, 7),
                            flags::ZERO => shifted == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => shifted.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => low_bit,
                        );
                    }
                    (0, 7, register) => {
                        // SRL r
                        let value = self.read_indexed_register(register);
                        let low_bit = bit_is_set(value, 0);
                        let shifted = value >> 1;
                        self.write_indexed_register(register, shifted);
                        self.registers.f = set_bits!(
                            self.registers.f,
                            flags::SIGN => false,
                            flags::ZERO => shifted == 0,
                            flags::HALF_CARRY => false,
                            flags::PARITY_OVERFLOW => shifted.count_ones().is_multiple_of(2),
                            flags::ADD_SUBTRACT => false,
                            flags::CARRY => low_bit,
                        );
                    }
                    _ => panic!("CB Unsupported instruction"),
                }
            }
            _ => panic!("Unsupported instruction"),
        }
    }

    fn set_rotate_flags(&mut self, new_carry: bool) {
        self.registers.f = set_bits!(
            self.registers.f,
            flags::CARRY => new_carry,
            flags::HALF_CARRY => false,
            flags::ADD_SUBTRACT => false,
        );
    }

    #[inline]
    fn set_boolean_operator_flags(&mut self, value: u8) {
        self.set_zero_and_sign_flags_for_u8(value);
        self.registers.f = set_bits!(
            self.registers.f,
            flags::CARRY => false,
            flags::ADD_SUBTRACT => false,
            flags::PARITY_OVERFLOW => value.count_ones().is_multiple_of(2),
        );
    }

    #[inline]
    fn set_zero_and_sign_flags_for_u8(&mut self, value: u8) {
        self.registers.f = set_bits!(
            self.registers.f,
            flags::ZERO => value == 0,
            flags::SIGN => is_set(value, 0x80),
        );
    }

    #[inline]
    fn read_and_advance(&mut self) -> u8 {
        let result = self.bus.read8(self.registers.pc);
        self.registers.increment_pc();
        result
    }

    #[inline]
    fn read_16_and_advance(&mut self) -> u16 {
        let low_byte = self.read_and_advance() as u16;
        let high_byte = self.read_and_advance() as u16;
        (high_byte << 8) | low_byte
    }

    #[inline]
    fn stack_push(&mut self, value: u16) {
        let [high_byte, low_byte] = value.to_be_bytes();

        self.registers.sp = self.registers.sp.wrapping_sub(2);
        self.bus.write8(self.registers.sp, low_byte);
        self.bus
            .write8(self.registers.sp.wrapping_add(1), high_byte);
    }

    #[inline]
    fn call(&mut self, address: u16) {
        self.stack_push(self.registers.pc);
        self.registers.pc = address;
    }

    #[inline]
    fn ret(&mut self) {
        let low_byte = self.bus.read8(self.registers.sp) as u16;
        let high_byte = self.bus.read8(self.registers.sp.wrapping_add(1)) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(2);
        self.registers.pc = (high_byte << 8) | low_byte
    }

    #[inline]
    fn bcd_difference(&mut self) -> u8 {
        // From The Undocumented Z80 Documented, by Sean Young
        // Version 0.91, 18th September, 2005:
        //     high       low
        // C  nibble  H  nibble  diff
        // ▔  ▔▔▔▔▔▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔
        // 0  0-9     0  0-9     00
        // 0  0-9     1  0-9     06
        // 0  0-8     *  a-f     06
        // 0  a-f     0  0-9     60
        // 1  *       0  0-9     60
        // 1  *       1  0-9     66
        // 1  *       *  a-f     66
        // 0  9-f     *  a-f     66
        // 0  a-f     1  0-9     66
        //
        // OK so this simplification took me a while to
        // understand; the key insight is to split the problem
        // into nibbles:
        //
        //        high    low
        // C  H  nibble  nibble  dh  dl
        // ▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔  ▔▔
        // 0  0  0-9     0-9     0   0
        // 0  1  0-9     0-9     0   6
        // 0  *  0-8     a-f     0   6
        // 0  0  a-f     0-9     6   0
        // 1  0  *       0-9     6   0
        // 1  1  *       0-9     6   6
        // 1  *  *       a-f     6   6
        // 0  *  9-f     a-f     6   6
        // 0  1  a-f     0-9     6   6
        //
        // Now there's two problems, solve for when DH is set to
        // 6, and when DL is set to six, by sorting the C and H
        // flags:
        //
        //        high     low                  high     low                high     low
        // C  H  nibble  nibble  dl      C  H  nibble  nibble  dl    C  H  nibble  nibble  dl
        // ▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔      ▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔    ▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔
        // 0  0  0-8     a-f      6┬─────0  0          a-f     6──┬──               a-f     6
        // 0  0  9-f     a-f      6┘┌────0  1                  6─╴│╶┬   1                   6
        //                          │ ┌──1  0          a-f     6──┘ │
        // 0  1  0-9     0-9      6┐│ │ ┌1  1                  6────┘
        // 0  1  a-f     0-9      6├┘ │ │
        // 0  1  0-8     a-f      6│  │ │
        // 0  1  9-f     a-f      6┘  │ │
        //                            │ │
        // 1  0  *       a-f      6───┘ │
        //                              │
        // 1  1  *       0-9      6┬────┘
        // 1  1  *       a-f      6┘
        //
        // so this yields H || ln >= 10. OK.  Next, the high
        // diff:
        //        high    low                  high    low
        //  C  H nibble  nibble  dh      C  H nibble  nibble  dh
        //  ▔  ▔ ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔      ▔  ▔ ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔
        //  0  0 9-f     a-f      6┬─────0    9-f     a-f      6
        //  0  1 9-f     a-f      6┘ ┌───0    a-f     0-9      6
        //                           │ ┌─1
        //  0  0 a-f     0-9      6┬─┘ │
        //  0  1 a-f     0-9      6┘   │
        //                             │
        //  1  1 *       a-f      6┐   │
        //  1  0 *       0-9      6├───┘
        //  1  1 *       0-9      6│
        //  1  0 *       a-f      6┘
        //
        // so this yields
        //  C
        //  || (h == 9 && l >= 10)
        //  || (h >= 10 && l <=10)
        // The trick is realizing that this can be simplified
        // if you look at the whole byte: the six is set when
        // a > 99, i.e.
        //   condition two     a >= 9A && a <= 9F
        //   condition three   a >= A0 && a <= A9
        // there's an overlap there, so we can simplify
        let had_carry = bit_is_set(self.registers.f, flags::CARRY);
        let had_half_carry = bit_is_set(self.registers.f, flags::HALF_CARRY);
        let a = self.registers.a;
        let low = a & 0xF;

        let low_diff = if had_half_carry || low >= 10 {
            0x06u8
        } else {
            0x00
        };
        let high_diff = if had_carry || a > 0x99 { 0x60u8 } else { 0x00 };
        low_diff | high_diff
    }

    #[inline]
    fn bcd_new_half_carry(&self) -> bool {
        // This also needs a little break down, the trick here is that
        // rows two and three are in conflict, so expanding rows one
        // and two with respect to three gets us the solution
        //            low
        //   NF  HF  nibble  HF’
        //   ▔▔  ▔▔  ▔▔▔▔▔▔  ▔▔▔
        //  ┌0   *   0-9     0
        //  │0   *   a-f     1───┬───conflict
        //  ├0   1   6-f     0───┘
        //  │1   1   0-5     1
        //  │
        //  └──HF  nibble  HF'
        //     ▔▔  ▔▔▔▔▔▔  ▔▔▔
        //     0   0-9     0───────row 1
        //     0   A-F     1───────row 2
        //     1   0-9     0───────row 1
        //     1   A-F     0───────row 3 overrides row 2
        //
        // H' = (N && H && L <= 5) || (!N && !H && L >= 10)

        let had_half_carry = bit_is_set(self.registers.f, flags::HALF_CARRY);
        let low = self.registers.a & 0xF;

        if bit_is_set(self.registers.f, flags::ADD_SUBTRACT) {
            had_half_carry && low <= 5
        } else {
            !had_half_carry && low >= 0xA
        }
    }

    #[inline]
    fn bcd_new_carry(&self) -> bool {
        // Same story here as the diff -- recognize that when the carry
        // flag is set, the output is always high, or, looking at the
        // full byte if the value is more than 0x99.
        //
        //      high    low
        // CF  nibble  nibble  CF’
        // ▔▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔▔
        // 0   0-9     0-9     0
        // 0   0-8     a-f     0
        // 0   9-f     a-f     1
        // 0   a-f     0-9     1
        // 1     *       *     1

        let had_carry = bit_is_set(self.registers.f, flags::CARRY);
        had_carry || self.registers.a > 0x99
    }
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    static REG_A: u8 = 7;
    static REG_B: u8 = 0;
    static REG_C: u8 = 1;
    static REG_D: u8 = 2;
    static REG_E: u8 = 3;
    static REG_H: u8 = 4;
    static REG_L: u8 = 5;
    static REG_HL: u8 = 6;

    static REG_A_SRC: u8 = REG_A;
    static REG_B_SRC: u8 = REG_B;
    static REG_C_SRC: u8 = REG_C;
    static REG_D_SRC: u8 = REG_D;
    static REG_E_SRC: u8 = REG_E;
    static REG_H_SRC: u8 = REG_H;
    static REG_L_SRC: u8 = REG_L;
    static REG_HL_SRC: u8 = REG_HL;

    static REG_A_DEST: u8 = REG_A << 3;
    static REG_B_DEST: u8 = REG_B << 3;
    static REG_C_DEST: u8 = REG_C << 3;
    static REG_D_DEST: u8 = REG_D << 3;
    static REG_E_DEST: u8 = REG_E << 3;
    static REG_H_DEST: u8 = REG_H << 3;
    static REG_L_DEST: u8 = REG_L << 3;
    static REG_HL_DEST: u8 = REG_HL << 3;

    mod cpu {
        use crate::{
            bus::{Bus, TestBus},
            cpu::Cpu,
            registers::Registers,
        };

        #[test]
        fn read_indexed_registers() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.registers.c = 3;
            cpu.registers.d = 4;
            cpu.registers.e = 5;
            cpu.registers.h = 6;
            cpu.registers.l = 7;
            cpu.bus.write8(6 << 8 | 7, 8);

            assert_eq!(1, cpu.read_indexed_register(7));
            assert_eq!(2, cpu.read_indexed_register(0));
            assert_eq!(3, cpu.read_indexed_register(1));
            assert_eq!(4, cpu.read_indexed_register(2));
            assert_eq!(5, cpu.read_indexed_register(3));
            assert_eq!(6, cpu.read_indexed_register(4));
            assert_eq!(7, cpu.read_indexed_register(5));
            assert_eq!(8, cpu.read_indexed_register(6));
        }

        #[test]
        fn write_indexed_registers() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.registers.c = 3;
            cpu.registers.d = 4;
            cpu.registers.e = 5;
            cpu.registers.h = 6;
            cpu.registers.l = 7;

            cpu.write_indexed_register(7, 1);
            cpu.write_indexed_register(0, 2);
            cpu.write_indexed_register(1, 3);
            cpu.write_indexed_register(2, 4);
            cpu.write_indexed_register(3, 5);
            cpu.write_indexed_register(4, 6);
            cpu.write_indexed_register(5, 7);
            cpu.write_indexed_register(6, 8);

            assert_eq!(1, cpu.registers.a);
            assert_eq!(2, cpu.registers.b);
            assert_eq!(3, cpu.registers.c);
            assert_eq!(4, cpu.registers.d);
            assert_eq!(5, cpu.registers.e);
            assert_eq!(6, cpu.registers.h);
            assert_eq!(7, cpu.registers.l);
            assert_eq!(8, cpu.bus.read8(6 << 8 | 7));
        }

        mod add_u8_by_index_and_set_flags {
            use crate::{
                bus::{Bus, TestBus},
                cpu::Cpu,
                flags::{self, bit_is_set, is_set},
                registers::{self, Registers},
                set_bits,
            };

            #[test]
            fn should_set_sign_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!bit_is_set(cpu.registers.f, flags::SIGN));

                cpu.registers.b = 1;
                cpu.registers.c = 0x7f;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            }

            #[test]
            fn should_set_zero_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!bit_is_set(cpu.registers.f, flags::ZERO));

                cpu.registers.b = 1;
                cpu.registers.c = 0xFF;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            }

            #[test]
            fn should_set_half_carry_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));

                cpu.registers.b = 1;
                cpu.registers.c = 0x0F;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            }

            #[test]
            fn should_set_overflow() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));

                cpu.registers.b = 1;
                cpu.registers.c = 0x7F;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            }

            #[test]
            fn should_reset_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);

                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_use_carry_in() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                let actual = cpu.add_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    true,
                );
                assert_eq!(4, actual);

                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
                let actual = cpu.add_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    true,
                );
                assert_eq!(3, actual);
            }
        }

        mod subtract_u8_and_set_flags {
            use crate::{
                bus::{Bus, TestBus},
                cpu::Cpu,
                flags::{self, bit_is_set, is_set},
                registers::{self, Registers},
                set_bits,
            };

            #[test]
            fn should_set_sign_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 3;
                cpu.registers.c = 2;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(!bit_is_set(cpu.registers.f, flags::SIGN));

                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            }

            #[test]
            fn should_set_zero_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(!bit_is_set(cpu.registers.f, flags::ZERO));

                cpu.registers.b = 1;
                cpu.registers.c = 1;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            }

            #[test]
            fn should_set_half_carry_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 0x0F;
                cpu.registers.c = 0x0F;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));

                cpu.registers.b = 0x0E;
                cpu.registers.c = 0x0F;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            }

            #[test]
            fn should_set_overflow() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 3;
                cpu.registers.c = 2;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));

                cpu.registers.b = 0x50;
                cpu.registers.c = 0xB0;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            }

            #[test]
            fn should_set_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => false);

                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_use_borrow_in() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                let actual = cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    true,
                );
                assert_eq!(0xFE, actual);

                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
                let actual = cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    true,
                );
                assert_eq!(0xFF, actual);
            }
        }

        mod set_boolean_operator_flags {
            use crate::{
                bus::{Bus, TestBus},
                cpu::Cpu,
                flags::{self, bit_is_set, is_set},
                registers::{self, Registers},
                set_bits,
            };

            #[test]
            fn should_set_zero() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(0);
                assert!(bit_is_set(cpu.registers.f, flags::ZERO));
                cpu.set_boolean_operator_flags(1);
                assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            }

            #[test]
            fn should_set_sign() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(0x80);
                assert!(bit_is_set(cpu.registers.f, flags::SIGN));
                cpu.set_boolean_operator_flags(0x7F);
                assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            }

            #[test]
            fn should_set_carry() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
                cpu.set_boolean_operator_flags(0);
                assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            }

            #[test]
            fn should_set_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);
                cpu.set_boolean_operator_flags(0);
                assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_set_parity() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(3);
                assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));

                cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => false);
                cpu.set_boolean_operator_flags(1);
                assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            }
        }

        mod add_u16_and_set_flags {
            use crate::{
                bus::{Bus, TestBus},
                cpu::Cpu,
                flags::{self, bit_is_set, is_set},
                registers::{self, Registers},
                set_bits,
            };

            #[test]
            fn should_set_carry() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.add_u16_and_set_flags(0xFFFF, 1, false);
                assert!(bit_is_set(cpu.registers.f, flags::CARRY));
            }

            #[test]
            fn should_set_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);
                cpu.add_u16_and_set_flags(0xFFFF, 1, false);
                assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_set_half_carry() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.add_u16_and_set_flags(0x0FFF, 0x001, false);
                assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
                cpu.add_u16_and_set_flags(1, 1, false);
                assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            }
        }
    }
    mod instructions {
        use crate::{
            bus::{Bus, TestBus},
            cpu::{
                Cpu,
                tests::{REG_C_DEST, REG_E_SRC, REG_HL_DEST, REG_HL_SRC},
            },
            flags::{self, bit_is_set, is_set},
            registers::Registers,
            set_bits,
        };

        #[test]
        fn nop_advances_pc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());

            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 0);

            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn halt_stops() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());

            cpu.bus.write8(0, 0x76);
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 0);
            assert!(!cpu.registers.halted);

            cpu.step();
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 1);
            assert!(cpu.registers.halted);
        }

        #[test]
        fn di_disables_iff() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());

            cpu.registers.iff1 = true;
            cpu.registers.iff2 = true;
            cpu.bus.write8(0, 0xF3);
            cpu.bus.write8(1, 0xF3);
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 0);

            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!cpu.registers.iff1);
            assert!(!cpu.registers.iff2);

            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 2);
            assert!(!cpu.registers.iff1);
            assert!(!cpu.registers.iff2);
        }

        #[test]
        fn ei_enables_iff() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());

            cpu.registers.iff1 = false;
            cpu.registers.iff2 = false;
            cpu.bus.write8(0, 0xFB);
            cpu.bus.write8(1, 0xFB);
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 0);

            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(cpu.registers.iff1);
            assert!(cpu.registers.iff2);

            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 2);
            assert!(cpu.registers.iff1);
            assert!(cpu.registers.iff2);
        }

        #[test]
        fn should_load_e_alt_into_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.e = 42;
            cpu.registers.c = 13;
            // ld c e
            cpu.bus.write8(0, 1 << 6 | REG_C_DEST | REG_E_SRC);
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.e, 42);
            assert_eq!(cpu.registers.c, 42);
        }

        #[test]
        fn should_load_hl_alt_into_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.h = 0xBE;
            cpu.registers.l = 0xEF;
            cpu.bus.write8(0xBEEF, 42);
            cpu.registers.c = 13;
            cpu.bus.write8(0, 1 << 6 | REG_C_DEST | REG_HL_SRC);
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.c, 42);
        }

        #[test]
        fn should_ld_r_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, REG_C_DEST | 6);
            cpu.bus.write8(1, 42);
            // ld r, nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.c, 42);
        }

        #[test]
        fn should_ld_r_n_to_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, REG_HL_DEST | 6);
            cpu.bus.write8(1, 42);
            // ld r, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xBEEF), 42);
        }

        #[test]
        fn should_load_bc_into_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_bc(0xBEEF);
            cpu.bus.write8(0xBEEF, 42);
            cpu.bus.write8(0, 1 << 3 | 2);
            // ld a, (BC)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 42);
        }

        #[test]
        fn should_load_a_into_bc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_bc(0xBEEF);
            cpu.registers.a = 42;
            cpu.bus.write8(0, 2);
            // ld (BC), a
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xBEEF), 42);
        }

        #[test]
        fn should_load_de_into_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_de(0xBEEF);
            cpu.bus.write8(0xBEEF, 42);
            cpu.bus.write8(0, 3 << 3 | 2);
            // ld a, (DE)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 42);
        }

        #[test]
        fn should_load_a_into_de() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_de(0xBEEF);
            cpu.registers.a = 42;
            cpu.bus.write8(0, 2 << 3 | 2);
            // ld (DE), a
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xBEEF), 42);
        }

        #[test]
        fn should_load_hl_into_nn() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 4 << 3 | 2);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD (nn), HL
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.l, cpu.bus.read8(0xB0A));
            assert_eq!(cpu.registers.h, cpu.bus.read8(0xB0B));
        }

        #[test]
        fn should_load_address_of_nn_into_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 5 << 3 | 2);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            cpu.bus.write8(0xB0A, 13);
            cpu.bus.write8(0xB0B, 42);
            // LD HL, (nn)
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.h, 42);
            assert_eq!(cpu.registers.l, 13);
        }

        #[test]
        fn should_load_a_into_nn() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.registers.a = 42;
            cpu.bus.write8(0, 6 << 3 | 2);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD (nn), A
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, cpu.bus.read8(0xB0A));
        }

        #[test]
        fn should_load_nn_into_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 7 << 3 | 2);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            cpu.bus.write8(0xB0A, 42);
            // LD A, (nn)
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 42);
        }

        #[test]
        fn should_load_nn_into_bc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 1);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD BC, (nn)
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.bc(), 0xB0A);
        }

        #[test]
        fn should_load_nn_into_de() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 2 << 3 | 1);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD BC, nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.de(), 0xB0A);
        }

        #[test]
        fn should_load_of_nn_into_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 1);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD HL, nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.hl(), 0xB0A);
        }

        #[test]
        fn should_load_of_nn_into_sp() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 6 << 3 | 1);
            cpu.bus.write8(1, 0xA);
            cpu.bus.write8(2, 0xB);
            // LD HL, nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 0xB0A);
        }

        #[test]
        fn should_push_bc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_bc(0xBEEF);
            cpu.registers.sp = 100;
            cpu.bus.write8(0, 3 << 6 | 5);
            // PUSH BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 98);
            assert_eq!(cpu.bus.read8(99), 0xBE);
            assert_eq!(cpu.bus.read8(98), 0xEF);
        }

        #[test]
        fn should_push_de() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_de(0xBEEF);
            cpu.registers.sp = 100;
            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 5);
            // PUSH DE
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 98);
            assert_eq!(cpu.bus.read8(99), 0xBE);
            assert_eq!(cpu.bus.read8(98), 0xEF);
        }

        #[test]
        fn should_push_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.registers.sp = 100;
            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 5);
            // PUSH HL
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 98);
            assert_eq!(cpu.bus.read8(99), 0xBE);
            assert_eq!(cpu.bus.read8(98), 0xEF);
        }

        #[test]
        fn should_push_af() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_af(0xBEEF);
            cpu.registers.sp = 100;
            cpu.bus.write8(0, 3 << 6 | 6 << 3 | 5);
            // PUSH AF
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 98);
            assert_eq!(cpu.bus.read8(99), 0xBE);
            assert_eq!(cpu.bus.read8(98), 0xEF);
        }

        #[test]
        fn should_pop_bc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(99, 0xBE);
            cpu.bus.write8(98, 0xEF);
            cpu.registers.sp = 98;

            cpu.bus.write8(0, 3 << 6 | 1);
            // POP BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 100);
            assert_eq!(cpu.registers.bc(), 0xBEEF);
        }

        #[test]
        fn should_pop_de() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(99, 0xBE);
            cpu.bus.write8(98, 0xEF);
            cpu.registers.sp = 98;

            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 1);
            // POP BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 100);
            assert_eq!(cpu.registers.de(), 0xBEEF);
        }

        #[test]
        fn should_pop_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(99, 0xBE);
            cpu.bus.write8(98, 0xEF);
            cpu.registers.sp = 98;

            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 1);
            // POP BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 100);
            assert_eq!(cpu.registers.hl(), 0xBEEF);
        }

        #[test]
        fn should_pop_af() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(99, 0xBE);
            cpu.bus.write8(98, 0xEF);
            cpu.registers.sp = 98;

            cpu.bus.write8(0, 3 << 6 | 6 << 3 | 1);
            // POP BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.sp, 100);
            assert_eq!(cpu.registers.af(), 0xBEEF);
        }

        #[test]
        fn should_inc_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
        }

        #[test]
        fn should_inc_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4);
            // INC B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.b, 1);
        }

        #[test]
        fn should_inc_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 1 << 3 | 4);
            // INC C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.c, 1);
        }

        #[test]
        fn should_inc_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 2 << 3 | 4);
            // INC D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.d, 1);
        }

        #[test]
        fn should_inc_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 3 | 4);
            // INC E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.e, 1);
        }

        #[test]
        fn should_inc_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 4);
            // INC H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.h, 1);
        }

        #[test]
        fn should_inc_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 5 << 3 | 4);
            // INC H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.l, 1);
        }

        #[test]
        fn should_inc_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 6 << 3 | 4);
            // INC (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xBEEF), 1);
        }

        #[test]
        fn should_set_sign_flag_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x7F;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_set_zero_flag_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xFF;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_set_half_carry_flag_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x0F;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_set_overflow_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x7F;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
        }

        #[test]
        fn should_reset_add_subtract_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);
            cpu.registers.a = 0xAB;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
        }

        #[test]
        fn should_dec_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 2;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
        }

        #[test]
        fn should_dec_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.b = 2;
            cpu.bus.write8(0, 5);
            // DEC B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.b, 1);
        }

        #[test]
        fn should_dec_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.c = 2;
            cpu.bus.write8(0, 1 << 3 | 5);
            // DEC C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.c, 1);
        }

        #[test]
        fn should_dec_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.d = 2;
            cpu.bus.write8(0, 2 << 3 | 5);
            // DEC D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.d, 1);
        }

        #[test]
        fn should_dec_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.e = 2;
            cpu.bus.write8(0, 3 << 3 | 5);
            // DEC E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.e, 1);
        }

        #[test]
        fn should_dec_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.h = 2;
            cpu.bus.write8(0, 4 << 3 | 5);
            // DEC H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.h, 1);
        }

        #[test]
        fn should_dec_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.l = 2;
            cpu.bus.write8(0, 5 << 3 | 5);
            // DEC H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.l, 1);
        }

        #[test]
        fn should_dec_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0xBEEF, 2);
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 6 << 3 | 5);
            // DEC (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xBEEF), 1);
        }

        #[test]
        fn should_set_sign_flag_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x0;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_set_zero_flag_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x1;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_set_half_carry_flag_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x10;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_set_overflow_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0x80;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
        }

        #[test]
        fn should_reset_add_subtract_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);
            cpu.registers.a = 0xAB;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
        }

        #[test]
        fn should_add_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 2 << 6 | 7);
            // ADD A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 2);
        }

        #[test]
        fn should_add_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.bus.write8(0, 2 << 6);
            // ADD A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.c = 2;
            cpu.bus.write8(0, 2 << 6 | 1);
            // ADD A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.d = 2;
            cpu.bus.write8(0, 2 << 6 | 2);
            // ADD A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.e = 2;
            cpu.bus.write8(0, 2 << 6 | 3);
            // ADD A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.h = 2;
            cpu.bus.write8(0, 2 << 6 | 4);
            // ADD A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.bus.write8(0, 2 << 6 | 5);
            // ADD A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_add_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 2 << 6 | 6);
            cpu.bus.write8(0xBEEF, 2);
            // ADD A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_adc_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 7);
            // ADC A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_adc_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3);
            // ADC A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.c = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 1);
            // ADC A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.d = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 2);
            // ADC A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.e = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 3);
            // ADC A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.h = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 4);
            // ADC A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 5);
            // ADC A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_adc_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.registers.set_hl(0xBEEF);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 1 << 3 | 6);
            cpu.bus.write8(0xBEEF, 2);
            // ADC A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_add_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 3 << 6 | 6);
            cpu.bus.write8(1, 2);
            // ADD A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
        }

        #[test]
        fn should_adc_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 3 << 6 | 1 << 3 | 6);
            cpu.bus.write8(1, 2);
            // ADD A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 4);
        }

        #[test]
        fn should_sub_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 7);
            // SUB A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0);
        }

        #[test]
        fn should_sub_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3);
            // SUB A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.c = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 1);
            // SUB A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.d = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 2);
            // SUB A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.e = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 3);
            // SUB A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.h = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 4);
            // SUB A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 5);
            // SUB A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sub_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0xBEEF, 2);
            cpu.bus.write8(0, 2 << 6 | 2 << 3 | 6);
            // SUB A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sbc_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 7);
            // SUB A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sbc_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3);
            // SUB A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.c = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 1);
            // SUB A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.d = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 2);
            // SUB A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.e = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 3);
            // SUB A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.h = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 4);
            // SUB A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 2;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 5);
            // SUB A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sbc_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0xBEEF, 2);
            cpu.bus.write8(0, 2 << 6 | 3 << 3 | 6);
            // SUB A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_sub_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 6);
            cpu.bus.write8(1, 2);
            // SUB A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
        }

        #[test]
        fn should_sbc_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0, 3 << 6 | 3 << 3 | 6);
            cpu.bus.write8(1, 2);
            // SBC A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFE);
        }

        #[test]
        fn should_cp_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 7);
            // CP A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.b = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3);
            // CP A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.c = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 1);
            // CP A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.d = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 2);
            // CP A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.e = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 3);
            // CP A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.h = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 4);
            // CP A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.l = 1;
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 5);
            // CP A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0xBEEF, 1);
            cpu.bus.write8(0, 2 << 6 | 7 << 3 | 6);
            // CP A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_cp_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 1;
            cpu.bus.write8(0, 3 << 6 | 7 << 3 | 6);
            cpu.bus.write8(1, 1);
            // CP A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
        }

        #[test]
        fn should_and_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 7);
            cpu.bus.write8(1, 1);
            // AND A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xAA);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.b = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3);
            cpu.bus.write8(1, 1);
            // AND A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.c = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 1);
            cpu.bus.write8(1, 1);
            // AND A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.d = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 2);
            // AND A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.e = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 3);
            // AND A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.h = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 4);
            // AND A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.l = 0x0F;
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 5);
            // AND A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 2 << 6 | 4 << 3 | 6);
            cpu.bus.write8(0xBEEF, 0x0F);
            // AND A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 7);
            cpu.bus.write8(1, 1);
            // OR A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xAA);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.b = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3);
            cpu.bus.write8(1, 1);
            // OR A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.c = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 1);
            cpu.bus.write8(1, 1);
            // OR A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.d = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 2);
            // OR A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.e = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 3);
            // OR A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.h = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 4);
            // OR A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.l = 0x55;
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 5);
            // OR A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 2 << 6 | 6 << 3 | 6);
            cpu.bus.write8(0xBEEF, 0x55);
            // OR A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 7);
            cpu.bus.write8(1, 1);
            // XOR A, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_b() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.b = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3);
            cpu.bus.write8(1, 1);
            // XOR A, B
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.c = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 1);
            cpu.bus.write8(1, 1);
            // XOR A, C
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_d() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.d = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 2);
            // XOR A, D
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_e() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.e = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 3);
            // XOR A, E
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_h() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.h = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 4);
            // XOR A, H
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_l() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.l = 0xFF;
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 5);
            // XOR A, L
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 2 << 6 | 5 << 3 | 6);
            cpu.bus.write8(0xBEEF, 0xFF);
            // XOR A, (HL)
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_and_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 6);
            cpu.bus.write8(1, 0x0F);
            // AND A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0A);
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_or_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 3 << 6 | 6 << 3 | 6);
            cpu.bus.write8(1, 0x55);
            // OR A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0xFF);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_xor_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xAA;
            cpu.bus.write8(0, 3 << 6 | 5 << 3 | 6);
            cpu.bus.write8(1, 0xFF);
            // OR A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_add_hl_bc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0x5F78);
            cpu.registers.set_bc(0x5F77);
            cpu.bus.write8(0, 1 << 3 | 1);

            // ADD HL BC
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.hl(), 0xBEEF);
        }

        #[test]
        fn should_add_hl_de() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0x5F78);
            cpu.registers.set_de(0x5F77);
            cpu.bus.write8(0, 3 << 3 | 1);

            // ADD HL DE
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.hl(), 0xBEEF);
        }

        #[test]
        fn should_add_hl_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0x657F);
            cpu.bus.write8(0, 5 << 3 | 1);

            // ADD HL HL
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.hl(), 0xCAFE);
        }

        #[test]
        fn should_add_hl_sp() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0x5F78);
            cpu.registers.sp = 0x5F77;
            cpu.bus.write8(0, 7 << 3 | 1);

            // ADD HL SP
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.hl(), 0xBEEF);
        }

        #[test]
        fn should_jp_nn() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 3);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);

            // JP nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jr_e_forward() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 3);
            cpu.bus.write8(1, 40);

            // JR e
            cpu.step();
            assert_eq!(cpu.registers.pc, 42);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jr_e_backwards() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 3);
            cpu.bus.write8(1, 0xFD);

            // JR e
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xFFFF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_hl(0xBEEF);
            cpu.bus.write8(0, 3 << 6 | 5 << 3 | 1);

            // JP HL
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_z_n_if_z_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 5 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);

            // JP Z n
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xFFFF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_z_n_if_z_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 5 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);

            // JP Z n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_nz_n_if_z_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);

            // JP NZ n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_nz_n_if_z_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);

            // JP NZ n
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xFFFF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_c_n_if_c_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // JP C n
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xFFFF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_c_n_if_c_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // JP C n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_nc_n_if_c_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 6 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // JP NC n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_nc_n_if_c_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 6 << 3);
            cpu.bus.write8(1, 0xFD);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // JP NC n
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xFFFF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_nz_nn_if_z_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);

            // JP NZ nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_nz_nn_if_z_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);

            // JP NZ nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_z_nn_if_z_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 1 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);

            // JP NZ nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_z_nn_if_z_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 1 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);

            // JP NZ nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_nc_nn_if_c_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // JP NC nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_nc_nn_if_c_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // JP NC nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_c_nn_if_c_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 3 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // JP C nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_c_nn_if_c_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 3 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // JP C nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_no_nn_if_o_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);

            // JP NO nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_no_nn_if_o_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);

            // JP NO nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_o_nn_if_o_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 5 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);

            // JP O nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_o_nn_if_o_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 5 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);

            // JP O nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_ns_nn_if_s_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 6 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);

            // JP NS nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_ns_nn_if_s_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 6 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);

            // JP NS nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_jp_s_nn_if_s_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 7 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);

            // JP S nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 3);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_jp_s_nn_if_s_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 6 | 7 << 3 | 2);
            cpu.bus.write8(1, 0xEF);
            cpu.bus.write8(2, 0xBE);
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);

            // JP S nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_nn() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3 | 5);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_nz_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_nz_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_z_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_z_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_nc_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 2 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nc nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_nc_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 2 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL nc nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_c_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 3 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL c nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_c_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 3 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL c nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_np_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL np nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_np_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL np nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_p_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 5 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL p nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_p_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 5 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL p nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_ns_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 6 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL ns nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_ns_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 6 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL ns nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_call_s_nn_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL s nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0E1);
            assert_eq!(cpu.registers.sp, 0xDEAF);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_call_s_nn_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7 << 3 | 4);
            cpu.bus.write8(0xC0DF, 0xEF);
            cpu.bus.write8(0xC0E0, 0xBE);

            // CALL s nn
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xE1);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);
            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3 | 1);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_nz_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);
            cpu.bus.write8(0xC0DE, 3 << 6);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET nz
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_nz_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);
            cpu.bus.write8(0xC0DE, 3 << 6);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET nz
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_z_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET z
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_z_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ZERO => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET z
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_nc_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 2 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET nc
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_nc_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 2 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET nc
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_c_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 3 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET c
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_c_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 3 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET c
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_np_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET np
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_np_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET np
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_p_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 5 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET p
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_p_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::PARITY_OVERFLOW => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 5 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET p
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_c_ns_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 6 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET ns
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_c_ns_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 6 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET ns
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_not_ret_d_s_when_unset() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => false);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET s
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xC0DF);
            assert_eq!(cpu.registers.sp, 0xDEAB);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_ret_d_s_when_set() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAB;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7 << 3);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RET s
            cpu.step();
            assert_eq!(cpu.registers.pc, 0xBEEF);
            assert_eq!(cpu.registers.sp, 0xDEAD);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_0() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 0
            cpu.step();
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_1() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 1 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x8);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_2() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 2 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x10);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_3() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 3 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x18);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_4() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 4 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x20);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_5() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 5 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x28);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_6() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 6 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x30);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_rst_7() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.sp = 0xDEAF;
            cpu.registers.pc = 0xC0DE;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::SIGN => true);
            cpu.bus.write8(0xC0DE, 3 << 6 | 7 << 3 | 7);
            cpu.bus.write8(cpu.registers.sp, 0xEF);
            cpu.bus.write8(cpu.registers.sp + 1, 0xBE);

            // RST 1
            cpu.step();
            assert_eq!(cpu.registers.pc, 0x38);
            assert_eq!(cpu.registers.sp, 0xDEAD);
            assert_eq!(cpu.bus.read8(cpu.registers.sp), 0xDF);
            assert_eq!(cpu.bus.read8(cpu.registers.sp + 1), 0xC0);

            assert_eq!(cpu.registers.r, 1);
        }

        #[test]
        fn should_in_a_n() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 0xFF;
            cpu.bus.write_io(1, 123);
            cpu.bus.write8(0, 3 << 6 | 3 << 3 | 3);
            cpu.bus.write8(1, 1);

            // IN A, n
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 123);
        }

        #[test]
        fn should_out_n_a() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = 42;
            cpu.bus.write_io(1, 123);
            cpu.bus.write8(0, 3 << 6 | 2 << 3 | 3);
            cpu.bus.write8(1, 1);

            // OUT n, A
            cpu.step();
            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 42);
            assert_eq!(cpu.bus.read_io(1), 42);
        }

        #[test]
        fn should_ex_de_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_de(0xC0DE);
            cpu.registers.set_hl(0xF00D);
            cpu.bus.write8(0, 3 << 6 | 5 << 3 | 3);

            // EX DE, HL
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.de(), 0xF00D);
            assert_eq!(cpu.registers.hl(), 0xC0DE);
        }

        #[test]
        fn should_ex_af_af_alt() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_af(0xCAFE);
            cpu.registers.set_af_alt(0xD00D);
            cpu.bus.write8(0, 1 << 3);

            // EX AF, AF'
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.af(), 0xD00D);
            assert_eq!(cpu.registers.af_alt(), 0xCAFE);
        }

        #[test]
        fn should_exx() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_bc(0xDEAD);
            cpu.registers.set_de(0xBEEF);
            cpu.registers.set_hl(0xCAFE);

            cpu.registers.set_bc_alt(0xFEED);
            cpu.registers.set_de_alt(0xFACE);
            cpu.registers.set_hl_alt(0xF00D);

            cpu.bus.write8(0, 3 << 6 | 3 << 3 | 1);

            // EXX
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);

            assert_eq!(cpu.registers.bc(), 0xFEED);
            assert_eq!(cpu.registers.de(), 0xFACE);
            assert_eq!(cpu.registers.hl(), 0xF00D);

            assert_eq!(cpu.registers.bc_alt(), 0xDEAD);
            assert_eq!(cpu.registers.de_alt(), 0xBEEF);
            assert_eq!(cpu.registers.hl_alt(), 0xCAFE);
        }

        #[test]
        fn should_ex_sp_hl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0xC0DE, 0xEF);
            cpu.bus.write8(0xC0DF, 0xBE);
            cpu.registers.sp = 0xC0DE;
            cpu.registers.set_hl(0xADDE);
            cpu.bus.write8(0, 3 << 6 | 4 << 3 | 3);

            // EX (SP) HL
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.bus.read8(0xC0DE), 0xDE);
            assert_eq!(cpu.bus.read8(0xC0DF), 0xAD);
            assert_eq!(cpu.registers.hl(), 0xBEEF);
        }

        #[test]
        fn should_rlca() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7);
            cpu.registers.a = 0x80;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // RLCA
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rrca() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 1 << 3 | 7);
            cpu.registers.a = 3;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => false);

            // RRCA
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x81);
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rla() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 2 << 3 | 7);
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // RLA
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 3);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rra() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 3 << 3 | 7);
            cpu.registers.a = 1;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::CARRY => true);

            // RRA
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x80);
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_calculate_diff_for_daa() {
            // Version 0.91, 18th September, 2005:
            //           high    low
            //    C  H  nibble  nibble  diff
            //    ▔  ▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔▔▔
            // Ⅰ  0  0  0-9     0-9     00
            // Ⅱ  0  1  0-9     0-9     06
            // Ⅲ  0  *  0-8     a-f     06
            // Ⅳ  0  0  a-f     0-9     60
            // Ⅴ  1  0  *       0-9     60
            // Ⅵ  1  1  *       0-9     66
            // Ⅶ  1  *  *       a-f     66
            // Ⅷ  0  *  9-f     a-f     66
            // Ⅸ  0  1  a-f     0-9     66

            assert_daa_bcd_difference(false, false, 0, 0, 0); //       Ⅰ
            assert_daa_bcd_difference(false, true, 0, 0, 0x06); //     Ⅱ
            assert_daa_bcd_difference(false, true, 8, 0xA, 0x06); //   Ⅲa
            assert_daa_bcd_difference(false, false, 8, 0xA, 0x06); //  Ⅲb
            assert_daa_bcd_difference(false, false, 0xA, 9, 0x60); //  Ⅳ
            assert_daa_bcd_difference(true, false, 0, 9, 0x60); //     Ⅴa
            assert_daa_bcd_difference(true, false, 0x10, 9, 0x60); //  Ⅴb
            assert_daa_bcd_difference(true, true, 0, 9, 0x66); //      Ⅵa
            assert_daa_bcd_difference(true, true, 0x10, 9, 0x66); //   Ⅵb
            assert_daa_bcd_difference(true, false, 0, 0xA, 0x66); //   Ⅶa
            assert_daa_bcd_difference(true, false, 0xA, 0xA, 0x66); // Ⅶb
            assert_daa_bcd_difference(true, true, 0, 0xA, 0x66); //    Ⅶc
            assert_daa_bcd_difference(true, true, 0xA, 9, 0x66); //    Ⅶd
            assert_daa_bcd_difference(false, false, 9, 0xA, 0x66); //  Ⅷa
            assert_daa_bcd_difference(false, true, 9, 0xA, 0x66); //   Ⅷb
            assert_daa_bcd_difference(false, true, 0xA, 9, 0x66); //   Ⅸ
        }

        fn assert_daa_bcd_difference(
            carry_flag: bool,
            half_cary_flag: bool,
            high_nibble: u8,
            low_nibble: u8,
            expected: u8,
        ) {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = high_nibble << 4 | low_nibble;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => carry_flag,
                flags::HALF_CARRY => half_cary_flag
            );
            assert_eq!(cpu.bcd_difference(), expected)
        }

        #[test]
        fn should_calculate_bcd_new_half_carry_daa() {
            //             low
            //    NF  HF  nibble  HF’
            //    ▔▔  ▔▔  ▔▔▔▔▔▔  ▔▔▔
            // Ⅰ  0   *   0-9     0
            // Ⅱ  0   *   a-f     1
            // Ⅲ  0   1   6-f     0
            // Ⅳ  1   1   0-5     1

            assert_bcd_new_half_carry(false, false, 9, false); //  Ⅰa
            assert_bcd_new_half_carry(false, true, 9, false); //   Ⅰb
            assert_bcd_new_half_carry(false, false, 0xA, true); // Ⅱa
            assert_bcd_new_half_carry(false, true, 0xA, false); // Ⅱa
            assert_bcd_new_half_carry(false, true, 6, false); //   Ⅲ
            assert_bcd_new_half_carry(true, true, 0, true); //     Ⅳ
            assert_bcd_new_half_carry(true, false, 0, false); //   Ⅴa
            assert_bcd_new_half_carry(true, true, 5, true); //     Ⅴb
            assert_bcd_new_half_carry(true, true, 6, false); //    Ⅴc
        }

        fn assert_bcd_new_half_carry(
            was_subtraction: bool,
            half_cary_flag: bool,
            low_nibble: u8,
            expected: bool,
        ) {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = low_nibble & 0xF;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::ADD_SUBTRACT => was_subtraction,
                flags::HALF_CARRY => half_cary_flag
            );
            assert_eq!(cpu.bcd_new_half_carry(), expected)
        }

        #[test]
        fn should_calculate_bcd_bcd_new_carry_daa() {
            //         high    low
            //    CF  nibble  nibble  CF’
            //    ▔▔  ▔▔▔▔▔▔  ▔▔▔▔▔▔  ▔▔▔
            // Ⅰ  0   0-9     0-9     0
            // Ⅱ  0   0-8     a-f     0
            // Ⅲ  0   9-f     a-f     1
            // Ⅳ  0   a-f     0-9     1
            // Ⅴ  1     *       *     1

            assert_bcd_new_carry(false, 0, 0, false); //   Ⅰ
            assert_bcd_new_carry(false, 8, 0xA, false); // Ⅱ
            assert_bcd_new_carry(false, 9, 0xA, true); //  Ⅲ
            assert_bcd_new_carry(false, 0xA, 9, true); //  Ⅳ
            assert_bcd_new_carry(true, 0, 0, true); //     Ⅴa
            assert_bcd_new_carry(true, 0, 0xA, true); //   Ⅴb
            assert_bcd_new_carry(true, 0xA, 0, true); //   Ⅴc
            assert_bcd_new_carry(true, 0xA, 0xA, true); // Ⅴd
        }

        fn assert_bcd_new_carry(carry_flag: bool, high_nibble: u8, low_nibble: u8, expected: bool) {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.a = (high_nibble << 4) | low_nibble;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => carry_flag,
            );
            assert_eq!(cpu.bcd_new_carry(), expected)
        }

        #[test]
        fn should_daa() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            // Example from the documentation:
            // base10   BCD h   BCD l
            //   15     0001    0101
            // + 27   + 0010    0111
            // ▔▔▔▔   ▔▔▔▔▔▔▔▔▔▔▔▔▔▔
            //   42     0011    1100  3C ──DAA──→ 0100 0010
            cpu.registers.a = 0x3C;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x42);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_for_negative() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0x81;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x81);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_and_set_carry_flag_after_add() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0xA1;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 1);
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_and_set_half_carry_flag_after_add() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0x0A;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x10);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_and_set_carry_flag_after_subtract() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0xF9;
            cpu.registers.f = set_bits!(cpu.registers.f, flags::ADD_SUBTRACT => true);
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x99);
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_and_set_half_carry_flag_after_subtract() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0x13;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::ADD_SUBTRACT => true,
                flags::HALF_CARRY => true
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 13);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_daa_and_unset_half_carry_flag_after_subtract() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 4 << 3 | 7);
            cpu.registers.a = 0xC;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::ADD_SUBTRACT => true,
                flags::HALF_CARRY => true
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 6);
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
        }

        #[test]
        fn should_cpl() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 5 << 3 | 7);
            cpu.registers.a = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.a, 0x55);
            assert!(bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
        }

        #[test]
        fn should_scf() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 6 << 3 | 7);
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_ccf_set_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7 << 3 | 7);
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::HALF_CARRY => true,
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_ccf_unset_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0, 7 << 3 | 7);
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => true,
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn shoulld_djnz_if_b_becomes_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.b = 1;
            cpu.bus.write8(0, 2 << 3);
            cpu.bus.write8(1, 0x0B);

            cpu.step();

            assert_eq!(cpu.registers.pc, 2);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.b, 0);
        }

        #[test]
        fn shoulld_djnz_forward_if_b_becomes_non_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.b = 2;
            cpu.bus.write8(0, 2 << 3);
            cpu.bus.write8(1, 0x0B);

            cpu.step();

            assert_eq!(cpu.registers.pc, 0x0D);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.b, 1);
        }
        #[test]
        fn shoulld_djnz_backward_if_b_becomes_non_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.b = 2;
            cpu.registers.pc = 0xF0;
            cpu.bus.write8(0xF0, 2 << 3);
            cpu.bus.write8(0xF1, 0xF5); // -0x0B

            cpu.step();

            assert_eq!(cpu.registers.pc, 0xE7);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.b, 1);
        }
    }

    mod phase1_tests {
        use crate::{
            bus::{Bus, TestBus},
            cpu::Cpu,
            registers::Registers,
        };

        #[test]
        fn should_calculate_6th_fib_number() {
            // this program calculates the Nth number of the Fibonacci
            // number, assuming that the 0th element is 0.
            // #  Fibonacci Number
            // ▔  ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔
            // 0  0
            // 1  1
            // 2  1
            // 3  2
            // 4  3
            // 5  5
            // 6  8

            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            const ROM: &[u8] = include_bytes!("../tools/sjasmplus/fib.bin");
            cpu.bus.load(ROM);
            cpu.run();

            assert_eq!(8, cpu.registers.a);
        }
    }

    mod cb_prefix {
        use crate::{
            bus::{Bus, TestBus},
            cpu::Cpu,
            flags::{self, bit_is_set},
            registers::Registers,
            set_bits,
        };

        #[test]
        fn should_rlc_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x00);
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rlc_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x00);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rlc_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x00);
            cpu.registers.b = 0x01;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x02);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rrc_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x08);
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rrc_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x08);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rrc_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x08);
            cpu.registers.b = 0x01;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x80);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rl_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x10);
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => true,
            );
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rl_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x10);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rl_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x10);
            cpu.registers.b = 0x01;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => true,
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x03);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rr_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x18);
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => true,
            );
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0xD5);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rr_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x18);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_rr_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x18);
            cpu.registers.b = 0x01;
            cpu.registers.f = set_bits!(
                cpu.registers.f,
                flags::CARRY => true,
            );
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x80);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sla_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x20);
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x54);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sla_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x20);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sla_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x20);
            cpu.registers.b = 0x01;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x02);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sra_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x28);
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0xD5);
            assert!(bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(!bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sra_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x28);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_sra_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x28);
            cpu.registers.b = 0x01;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x00);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_srl_r() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x38);
            cpu.registers.b = 0xAA;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x55);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(!bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_srl_r_for_zero() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x38);
            cpu.registers.b = 0x0;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x0);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(!bit_is_set(cpu.registers.f, flags::CARRY));
        }

        #[test]
        fn should_srl_r_for_one() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.bus.write8(0x00, 0xCB);
            cpu.bus.write8(0x01, 0x38);
            cpu.registers.b = 0x01;
            cpu.step();

            assert_eq!(cpu.registers.pc, 0x02);
            assert_eq!(cpu.registers.b, 0x00);
            assert!(!bit_is_set(cpu.registers.f, flags::SIGN));
            assert!(bit_is_set(cpu.registers.f, flags::ZERO));
            assert!(!bit_is_set(cpu.registers.f, flags::HALF_CARRY));
            assert!(bit_is_set(cpu.registers.f, flags::PARITY_OVERFLOW));
            assert!(!bit_is_set(cpu.registers.f, flags::ADD_SUBTRACT));
            assert!(bit_is_set(cpu.registers.f, flags::CARRY));
        }
    }
}

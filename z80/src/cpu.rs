use crate::{
    bus::Bus,
    decode,
    flags::{self, is_set},
    registers::{self, Registers},
};

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
        let carry_in: u8 = if use_carry && self.registers.flag(flags::CARRY) {
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

        self.registers.set_flag(flags::CARRY, full > 0xFF);
        self.registers.set_flag(flags::ADD_SUBTRACT, false);
        self.registers.set_flag(flags::PARITY_OVERFLOW, overflow);
        self.registers.set_flag(flags::HALF_CARRY, half_carry);
        self.registers.set_flag(flags::ZERO, sum == 0);
        self.registers.set_flag(flags::SIGN, is_set(sum, 0x80));

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
    fn subtract_u8_and_set_flags(&mut self, minuend: u8, subtrahend: u8, use_borrow: bool) -> u8 {
        let borrow_in: u8 = if use_borrow && self.registers.flag(flags::CARRY) {
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

        self.registers.set_flag(flags::CARRY, full > 0xFF);
        self.registers.set_flag(flags::ADD_SUBTRACT, true);
        self.registers.set_flag(flags::PARITY_OVERFLOW, overflow);
        self.registers.set_flag(flags::HALF_CARRY, half_carry);
        self.registers.set_flag(flags::ZERO, difference == 0);
        self.registers
            .set_flag(flags::SIGN, is_set(difference, 0x80));

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

    pub fn step(&mut self) {
        match decode::into_group_and_operands(self.bus.read8(self.registers.pc)) {
            (0, 0, 0) => {
                // NOP
                self.registers.increment_pc();
                self.registers.increment_r();
            }
            (1, 6, 6) => {
                // HALT
                self.registers.increment_r();
                self.registers.halted = true
            }
            (3, 6, 3) => {
                // DI
                self.registers.increment_pc();
                self.registers.increment_r();
                self.registers.iff1 = false;
                self.registers.iff2 = false;
            }
            (3, 7, 3) => {
                // EI
                self.registers.increment_pc();
                self.registers.increment_r();
                self.registers.iff1 = true;
                self.registers.iff2 = true;
            }
            (1, dest, src) => {
                // LD r, r'
                // LD r, (HL)
                self.registers.increment_pc();
                self.registers.increment_r();
                let value = self.read_indexed_register(src);
                self.write_indexed_register(dest, value);
            }
            (0, register, 6) => {
                // LD r, n
                self.registers.increment_pc();
                self.registers.increment_r();
                let value = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();
                self.write_indexed_register(register, value);
            }
            (0, op @ 0..=3, 2) => {
                self.registers.increment_pc();
                self.registers.increment_r();

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
                self.registers.increment_pc();
                self.registers.increment_r();

                let operation = op & 1;
                const HL: u8 = 0;
                const WRITE: u8 = 0;

                let low = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();
                let high = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

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
                self.registers.increment_pc();
                self.registers.increment_r();

                let low = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();
                let high = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

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
                self.registers.increment_pc();
                self.registers.increment_r();

                let (high, low) = match pair {
                    0 => (self.registers.b, self.registers.c),
                    2 => (self.registers.d, self.registers.e),
                    4 => (self.registers.h, self.registers.l),
                    6 => (self.registers.a, self.registers.f),
                    _ => unreachable!(),
                };

                self.registers.decrement_sp();
                self.bus.write8(self.registers.sp, high);
                self.registers.decrement_sp();
                self.bus.write8(self.registers.sp, low);
            }
            (3, pair @ (0 | 2 | 4 | 6), 1) => {
                // POP rr
                self.registers.increment_pc();
                self.registers.increment_r();

                let low = self.bus.read8(self.registers.sp);
                self.registers.increment_sp();
                let high = self.bus.read8(self.registers.sp);
                self.registers.increment_sp();

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
                self.registers.increment_pc();
                self.registers.increment_r();

                let before = self.read_indexed_register(register);
                let after = before.wrapping_add(1);
                self.write_indexed_register(register, after);

                self.registers.set_flag(flags::ADD_SUBTRACT, false);
                self.registers
                    .set_flag(flags::PARITY_OVERFLOW, before == 0x7F);
                self.registers
                    .set_flag(flags::HALF_CARRY, is_set(before, 0x0F));
                self.registers.set_flag(flags::ZERO, after == 0);
                self.registers.set_flag(flags::SIGN, is_set(after, 0x80));
            }
            (0, register, 5) => {
                // DEC r
                self.registers.increment_pc();
                self.registers.increment_r();

                let before = self.read_indexed_register(register);
                let after = before.wrapping_sub(1);
                self.write_indexed_register(register, after);

                self.registers.set_flag(flags::ADD_SUBTRACT, true);
                self.registers
                    .set_flag(flags::PARITY_OVERFLOW, before == 0x80);
                self.registers
                    .set_flag(flags::HALF_CARRY, before & 0x0F == 0x00);
                self.set_zero_and_sign_flags_for_u8(after);
            }
            (2, 0, register) => {
                // ADD A. r
                self.registers.increment_pc();
                self.registers.increment_r();

                let sum = self.add_u8_by_index_and_set_flags(registers::index::A, register, false);
                self.registers.a = sum;
            }
            (2, 1, register) => {
                // ADC A. r
                self.registers.increment_pc();
                self.registers.increment_r();

                let sum = self.add_u8_by_index_and_set_flags(registers::index::A, register, true);
                self.registers.a = sum;
            }
            (3, 0, 6) => {
                // ADD A, n
                self.registers.increment_pc();
                self.registers.increment_r();

                let addend = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                let sum = self.add_u8_and_set_flags(self.registers.a, addend, false);
                self.registers.a = sum
            }
            (3, 1, 6) => {
                // ADC A, n
                self.registers.increment_pc();
                self.registers.increment_r();

                let addend = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                let sum = self.add_u8_and_set_flags(self.registers.a, addend, true);
                self.registers.a = sum
            }
            (2, 2, register) => {
                // SUB A, r
                self.registers.increment_pc();
                self.registers.increment_r();

                let difference =
                    self.subtract_u8_by_index_and_set_flags(registers::index::A, register, false);
                self.registers.a = difference;
            }
            (2, 3, register) => {
                // SBC A, r
                self.registers.increment_pc();
                self.registers.increment_r();

                let difference =
                    self.subtract_u8_by_index_and_set_flags(registers::index::A, register, true);
                self.registers.a = difference;
            }
            (3, 2, 6) => {
                // SUB A, n
                self.registers.increment_pc();
                self.registers.increment_r();

                let subtrahend = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                let difference =
                    self.subtract_u8_and_set_flags(self.registers.a, subtrahend, false);
                self.registers.a = difference
            }
            (3, 3, 6) => {
                // SBC A, n
                self.registers.increment_pc();
                self.registers.increment_r();

                let subtrahend = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                let difference = self.subtract_u8_and_set_flags(self.registers.a, subtrahend, true);
                self.registers.a = difference
            }
            (2, 7, register) => {
                // CP r
                self.registers.increment_pc();
                self.registers.increment_r();

                self.subtract_u8_by_index_and_set_flags(registers::index::A, register, false);
            }
            (3, 7, 6) => {
                // CP n
                self.registers.increment_pc();
                self.registers.increment_r();

                let subtrahend = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();
                self.subtract_u8_and_set_flags(self.registers.a, subtrahend, false);
            }
            (2, 4, register) => {
                // AND r
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.read_indexed_register(register);
                self.registers.a &= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, true);
            }
            (2, 6, register) => {
                // OR r
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.read_indexed_register(register);
                self.registers.a |= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, false);
            }
            (2, 5, register) => {
                // XOR r
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.read_indexed_register(register);
                self.registers.a ^= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, false);
            }
            (3, 4, 6) => {
                // AND n
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                self.registers.a &= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, true);
            }
            (3, 6, 6) => {
                // OR n
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                self.registers.a |= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, false);
            }
            (3, 5, 6) => {
                // XOR n
                self.registers.increment_pc();
                self.registers.increment_r();

                let value = self.bus.read8(self.registers.pc);
                self.registers.increment_pc();

                self.registers.a ^= value;
                self.set_boolean_operator_flags(self.registers.a);
                self.registers.set_flag(flags::HALF_CARRY, false);
            }
            _ => panic!("Unsupported instruction"),
        }
    }

    fn set_boolean_operator_flags(&mut self, value: u8) {
        self.set_zero_and_sign_flags_for_u8(value);
        self.registers.set_flag(flags::CARRY, false);
        self.registers.set_flag(flags::ADD_SUBTRACT, false);
        self.registers
            .set_flag(flags::PARITY_OVERFLOW, value.count_ones().is_multiple_of(2));
    }

    fn set_zero_and_sign_flags_for_u8(&mut self, value: u8) {
        self.registers.set_flag(flags::ZERO, value == 0);
        self.registers.set_flag(flags::SIGN, is_set(value, 0x80));
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
                flags,
                registers::{self, Registers},
            };

            #[test]
            fn should_set_sign_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!cpu.registers.flag(flags::SIGN));

                cpu.registers.b = 1;
                cpu.registers.c = 0x7f;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(cpu.registers.flag(flags::SIGN));
            }

            #[test]
            fn should_set_zero_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!cpu.registers.flag(flags::ZERO));

                cpu.registers.b = 1;
                cpu.registers.c = 0xFF;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(cpu.registers.flag(flags::ZERO));
            }

            #[test]
            fn should_set_half_carry_flag() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!cpu.registers.flag(flags::HALF_CARRY));

                cpu.registers.b = 1;
                cpu.registers.c = 0x0F;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(cpu.registers.flag(flags::HALF_CARRY));
            }

            #[test]
            fn should_set_overflow() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!cpu.registers.flag(flags::PARITY_OVERFLOW));

                cpu.registers.b = 1;
                cpu.registers.c = 0x7F;
                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(cpu.registers.flag(flags::PARITY_OVERFLOW));
            }

            #[test]
            fn should_reset_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::ADD_SUBTRACT, true);

                cpu.add_u8_by_index_and_set_flags(registers::index::B, registers::index::C, false);
                assert!(!cpu.registers.flag(flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_use_carry_in() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::CARRY, true);
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
                cpu.registers.set_flag(flags::CARRY, false);
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
                flags,
                registers::{self, Registers},
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
                assert!(!cpu.registers.flag(flags::SIGN));

                cpu.registers.b = 1;
                cpu.registers.c = 2;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(cpu.registers.flag(flags::SIGN));
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
                assert!(!cpu.registers.flag(flags::ZERO));

                cpu.registers.b = 1;
                cpu.registers.c = 1;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(cpu.registers.flag(flags::ZERO));
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
                assert!(!cpu.registers.flag(flags::HALF_CARRY));

                cpu.registers.b = 0x0E;
                cpu.registers.c = 0x0F;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(cpu.registers.flag(flags::HALF_CARRY));
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
                assert!(!cpu.registers.flag(flags::PARITY_OVERFLOW));

                cpu.registers.b = 0x50;
                cpu.registers.c = 0xB0;
                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(cpu.registers.flag(flags::PARITY_OVERFLOW));
            }

            #[test]
            fn should_set_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::ADD_SUBTRACT, false);

                cpu.subtract_u8_by_index_and_set_flags(
                    registers::index::B,
                    registers::index::C,
                    false,
                );
                assert!(cpu.registers.flag(flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_use_borrow_in() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::CARRY, true);
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
                cpu.registers.set_flag(flags::CARRY, false);
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
                flags,
                registers::{self, Registers},
            };

            #[test]
            fn should_set_zero() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(0);
                assert!(cpu.registers.flag(flags::ZERO));
                cpu.set_boolean_operator_flags(1);
                assert!(!cpu.registers.flag(flags::ZERO));
            }

            #[test]
            fn should_set_sign() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(0x80);
                assert!(cpu.registers.flag(flags::SIGN));
                cpu.set_boolean_operator_flags(0x7F);
                assert!(!cpu.registers.flag(flags::SIGN));
            }

            #[test]
            fn should_set_carry() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::CARRY, true);
                cpu.set_boolean_operator_flags(0);
                assert!(!cpu.registers.flag(flags::CARRY));
            }

            #[test]
            fn should_set_add_subtract() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.registers.set_flag(flags::ADD_SUBTRACT, true);
                cpu.set_boolean_operator_flags(0);
                assert!(!cpu.registers.flag(flags::ADD_SUBTRACT));
            }

            #[test]
            fn should_set_parity() {
                let mut cpu = Cpu::new(Registers::new(), TestBus::new());
                cpu.set_boolean_operator_flags(3);
                assert!(cpu.registers.flag(flags::PARITY_OVERFLOW));

                cpu.registers.set_flag(flags::ADD_SUBTRACT, false);
                cpu.set_boolean_operator_flags(1);
                assert!(!cpu.registers.flag(flags::PARITY_OVERFLOW));
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
            flags::{self, is_bit_set, is_set},
            registers::Registers,
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
            assert!(cpu.registers.flag(flags::SIGN));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::PARITY_OVERFLOW));
        }

        #[test]
        fn should_reset_add_subtract_on_inc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_flag(flags::ADD_SUBTRACT, true);
            cpu.registers.a = 0xAB;
            cpu.bus.write8(0, 7 << 3 | 4);
            // INC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(!cpu.registers.flag(flags::ADD_SUBTRACT));
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
            assert!(cpu.registers.flag(flags::SIGN));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::PARITY_OVERFLOW));
        }

        #[test]
        fn should_reset_add_subtract_on_dec() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.set_flag(flags::ADD_SUBTRACT, true);
            cpu.registers.a = 0xAB;
            cpu.bus.write8(0, 7 << 3 | 5);
            // DEC A
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert!(cpu.registers.flag(flags::ADD_SUBTRACT));
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            cpu.bus.write8(0, 3 << 6 | 2 << 3  | 6);
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
            cpu.registers.set_flag(flags::CARRY, true);
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::ZERO));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
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
            assert!(!cpu.registers.flag(flags::HALF_CARRY));
        }
    }
}

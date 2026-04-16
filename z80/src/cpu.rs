use crate::{bus::Bus, decode, registers::Registers};

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
            _ => panic!("Unsupported instruction"),
        }
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
    }

    mod instructions {
        use crate::{
            bus::{Bus, TestBus},
            cpu::{
                Cpu,
                tests::{REG_C_DEST, REG_E_SRC, REG_HL_DEST, REG_HL_SRC},
            },
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
    }
}

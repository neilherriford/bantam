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
                self.registers.increment_pc();
                self.registers.increment_r();
                let value = self.read_indexed_register(src);
                self.write_indexed_register(dest, value);
            }
            _ => panic!("Unsupported instruction"),
        }
    }
}

#[cfg(test)]
mod tests {
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
            cpu::Cpu,
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
            cpu.bus.write8(0, 0x4B);
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.e, 42);
            assert_eq!(cpu.registers.c, 42);
        }

        #[test]
        fn should_load_hl_alt_into_c() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            cpu.registers.h = 1;
            cpu.registers.l = 2;
            cpu.bus.write8(1 << 8 | 2, 42);
            cpu.registers.c = 13;
            // ld c hl
            cpu.bus.write8(0, 0x4E);
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
            assert_eq!(cpu.registers.h, 1);
            assert_eq!(cpu.registers.l, 2);
            assert_eq!(cpu.registers.c, 42);
        }
    }
}

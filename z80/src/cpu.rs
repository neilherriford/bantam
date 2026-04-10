use crate::{bus::Bus, registers::Registers};
use num_enum::{FromPrimitive, IntoPrimitive};

pub struct Cpu<B: Bus> {
    pub registers: Registers,
    pub bus: B,
}

#[derive(FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum Instructions {
    NOP = 0,
    HALT = 0x76,
    DI = 0xF3,
    EI = 0xFB,
    #[num_enum(default)]
    Unknown,
}

impl<B> Cpu<B>
where
    B: Bus,
{
    pub fn new(registers: Registers, bus: B) -> Self {
        Self { registers, bus }
    }

    pub fn step(&mut self) {
        match Instructions::from(self.bus.read8(self.registers.pc)) {
            Instructions::NOP => {
                self.registers.increment_pc();
                self.registers.increment_r();
            }
            Instructions::HALT => {
                self.registers.increment_r();
                self.registers.halted = true
            }
            Instructions::DI => {
                self.registers.increment_pc();
                self.registers.increment_r();
                self.registers.iff1 = false;
                self.registers.iff2 = false;
            }
            Instructions::EI => {
                self.registers.increment_pc();
                self.registers.increment_r();
                self.registers.iff1 = true;
                self.registers.iff2 = true;
            }

            Instructions::Unknown => panic!("Unsupported instruction"),
        }
    }
}

#[cfg(test)]
mod tests {
    mod instructions {
        use crate::{
            bus::{Bus, TestBus},
            cpu::{Cpu, Instructions},
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

            cpu.bus.write8(0, Instructions::HALT.into());
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
            cpu.bus.write8(0, Instructions::DI.into());
            cpu.bus.write8(1, Instructions::DI.into());
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
            cpu.bus.write8(0, Instructions::EI.into());
            cpu.bus.write8(1, Instructions::EI.into());
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
    }
}

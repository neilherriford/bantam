use crate::{bus::Bus, registers::Registers};

pub struct Cpu<B: Bus> {
    pub registers: Registers,
    pub bus: B,
}

#[repr(u8)]
pub enum Instructions {
    NOP = 0,
    Unknown,
}

impl From<u8> for Instructions {
    fn from(value: u8) -> Self {
        match value {
            0 => Instructions::NOP,
            _ => Instructions::Unknown,
        }
    }
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
                self.registers.pc += 1;
                self.registers.r =
                    self.registers.r & 0x80 | (self.registers.r.wrapping_add(1) & 0x7f);
            }

            Instructions::Unknown => panic!("Unsupported instruction"),
        }
    }
}

#[cfg(test)]
mod tests {
    mod instructions {
        use crate::{bus::TestBus, cpu::Cpu, registers::Registers};

        #[test]
        fn nop_advances_pc() {
            let mut cpu = Cpu::new(Registers::new(), TestBus::new());
            assert_eq!(cpu.registers.pc, 0);
            assert_eq!(cpu.registers.r, 0);
            cpu.step();
            assert_eq!(cpu.registers.pc, 1);
            assert_eq!(cpu.registers.r, 1);
        }
    }
}

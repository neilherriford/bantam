use core::panic;

pub trait Bus {
    fn read8(&mut self, address: u16) -> u8;
    fn read16(&mut self, address: u16) -> u16;
    fn write8(&mut self, address: u16, value: u8);
    fn write16(&mut self, address: u16, value: u16);
    fn read_io(&mut self, port: u16) -> u8;
    fn write_io(&mut self, port: u16, value: u8);
    fn load(&mut self, data: &[u8]);
}

pub struct TestBus {
    data: [u8; 65536],
    io: [u8; 256],
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            data: [0; 65536],
            io: [0; 256],
        }
    }
}

impl Default for TestBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for TestBus {
    fn read8(&mut self, address: u16) -> u8 {
        self.data[address as usize]
    }
    fn read16(&mut self, address: u16) -> u16 {
        let low_byte = self.data[address as usize] as u16;
        let high_byte = self.data[address.wrapping_add(1) as usize] as u16;

        high_byte << 8 | low_byte
    }

    fn write8(&mut self, address: u16, value: u8) {
        self.data[address as usize] = value
    }

    fn write16(&mut self, address: u16, value: u16) {
        let high_byte = ((value >> 8) & 0xFF) as u8;
        let low_byte = (value & 0xFF) as u8;

        self.write8(address, low_byte);
        self.write8(address.wrapping_add(1), high_byte);
    }

    fn read_io(&mut self, port: u16) -> u8 {
        self.io[(port & 0xFF) as usize]
    }

    fn write_io(&mut self, port: u16, value: u8) {
        self.io[(port & 0xFF) as usize] = value;
    }

    fn load(&mut self, data: &[u8]) {
        if data.len() > self.data.len() {
            panic!("Too much data!");
        }
        self.data[..data.len()].copy_from_slice(data);
    }
}

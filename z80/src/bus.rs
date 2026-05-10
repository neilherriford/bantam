use core::panic;

pub trait Bus {
    fn read8(&mut self, address: u16) -> u8;
    fn write8(&mut self, address: u16, value: u8);
    fn read_io(&mut self, port: u16) -> u8;
    fn write_io(&mut self, port: u16, value: u8);
    fn load(&mut self, data: &[u8]);
}

pub struct TestBus {
    data: [u8; 65536],
    io: [u8; 258],
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            data: [0; 65536],
            io: [0; 258],
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

    fn write8(&mut self, address: u16, value: u8) {
        self.data[address as usize] = value
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

pub trait Bus {
    fn read8(&mut self, address: u16) -> u8;
    fn write8(&mut self, address: u16, value: u8);
}

pub struct TestBus {
    data: [u8; 65536],
}

impl TestBus {
    pub fn new() -> Self {
        Self { data: [0; 65536] }
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
}

pub struct Registers {
    pub w: u8, // Temporary
    pub z: u8, // Temporary
    pub a: u8, // Accumuator
    pub f: u8, // Flags
    pub b: u8, //
    pub c: u8, //
    pub d: u8, //
    pub e: u8, //
    pub h: u8, // Accumulator
    pub l: u8, // Accumulator

    pub a_alt: u8, // Accumuator
    pub f_alt: u8, // Flags
    pub b_alt: u8, //
    pub c_alt: u8, //
    pub d_alt: u8, //
    pub e_alt: u8, //
    pub h_alt: u8, // Accumulator
    pub l_alt: u8, // Accumulator

    // Special
    pub pc: u16, // Program Counter
    pub sp: u16, // Stack Pointer
    pub ix: u16, // Index X
    pub iy: u16, // Index Y
    pub i: u8,   // Interrupt vector
    pub r: u8,   // Refresh counter

    pub iff1: bool, // interrupt flags
    pub iff2: bool, // interrupt flags
    pub im: u8,     // interrupt mode

    pub halted: bool,
}

impl Registers {
    pub fn new() -> Self {
        Self {
            w: 0,
            z: 0,
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_alt: 0,
            f_alt: 0,
            b_alt: 0,
            c_alt: 0,
            d_alt: 0,
            e_alt: 0,
            h_alt: 0,
            l_alt: 0,
            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,
            i: 0,
            r: 0,
            iff1: false,
            iff2: false,
            im: 0,
            halted: false,
        }
    }

    pub fn wz(&self) -> u16 {
        ((self.w as u16) << 8) | (self.z as u16)
    }
    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }
    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }
    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    pub fn set_wz(&mut self, value: u16) {
        self.w = (value >> 8) as u8;
        self.z = (value & 0xFF) as u8;
    }
    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value & 0xFF) as u8;
    }
    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }
    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }
    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

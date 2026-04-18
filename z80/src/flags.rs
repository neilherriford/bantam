pub const CARRY: u8 = 0;
pub const ADD_SUBTRACT: u8 = 1;
pub const PARITY_OVERFLOW: u8 = 2;
pub const HALF_CARRY: u8 = 4;
pub const ZERO: u8 = 6;
pub const SIGN: u8 = 7;

pub fn is_set(value: u8, test: u8) -> bool {
    (value & test) == test
}

pub fn is_bit_set(value: u8, index: u8) -> bool {
    is_set(value, 1 << index)
}

pub fn get_bit(value: u8, bit: u8) -> bool {
    is_set(value, 1 << bit)
}

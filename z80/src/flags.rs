pub const CARRY: u8 = 0;
pub const ADD_SUBTRACT: u8 = 1;
pub const PARITY_OVERFLOW: u8 = 2;
pub const HALF_CARRY: u8 = 4;
pub const ZERO: u8 = 6;
pub const SIGN: u8 = 7;

pub fn is_set(value: u8, test: u8) -> bool {
    (value & test) == test
}

pub fn bit_is_set(value: u8, index: u8) -> bool {
    is_set(value, 1 << index)
}

pub fn get_bit(value: u8, bit: u8) -> bool {
    is_set(value, 1 << bit)
}

#[macro_export]
macro_rules! set_bits {
    ($val:expr, $($bit:expr => $flag:expr),* ,) => {
        set_bits!($val, $($bit => $flag),*)
    };
    ($val:expr, $($bit:expr => $flag:expr),*) => {{
        let mut result = $val;
        $(
            if $flag {
                result |= 1 << $bit;
            } else {
                result &= !(1 << $bit);
            }
        )*
        result
    }};
}

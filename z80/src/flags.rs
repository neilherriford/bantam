pub const CARRY: u8 = 0;
pub const ADD_SUBTRACT: u8 = 1;
pub const PARITY_OVERFLOW: u8 = 2;
pub const HALF_CARRY: u8 = 4;
pub const ZERO: u8 = 6;
pub const SIGN: u8 = 7;

use core::ops::BitAnd;

pub fn is_set<T: BitAnd<Output = T> + PartialEq + Copy>(value: T, test: T) -> bool {
    (value & test) == test
}

pub fn bit_is_set<T>(value: T, index: u8) -> bool
where
    T: core::ops::BitAnd<Output = T> + core::ops::Shl<u8, Output = T> + PartialEq + Copy + From<u8>,
{
    let mask = T::from(1u8) << index;
    is_set(value, mask)
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

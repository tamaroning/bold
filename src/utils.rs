#[macro_export]
/// Create a zero-cleared value of a given type.
macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

// e.g. padding(0x1234, 0x1000) == 0x2000 - 0x1234
// padding(0x1000, 0x100) = 0x1000 - 0x1000
pub fn padding(val: u64, align: u64) -> u64 {
    if val % align == 0 {
        0
    } else {
        align - (val % align)
    }
}

#[macro_export]
/// Create a zero-cleared value of a given type.
macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

pub fn align_to(val: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    return (val + align - 1) & !(align - 1);
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

pub fn write_to<T>(buf: &mut [u8], offset: usize, data: &T) -> usize {
    let size = std::mem::size_of::<T>();
    let view = data as *const _ as *const u8;
    let slice = unsafe { std::slice::from_raw_parts(view, size) };
    buf[offset..offset + size].copy_from_slice(slice);
    size
}

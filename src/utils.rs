#[macro_export]
/// Create a zero-cleared value of a given type.
macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

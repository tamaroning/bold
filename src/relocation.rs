use elf::{abi, relocation::Rela};

// https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/mold.h#L312
#[derive(Debug)]
pub enum RelType {
    None,
    /// L + A - P
    Pc,
    /// L + A
    Abs,
}

impl RelType {
    pub fn from(r_type: u32) -> Self {
        match r_type {
            abi::R_X86_64_NONE => RelType::None,
            abi::R_X86_64_PLT32 => RelType::Pc,
            abi::R_X86_64_8 | abi::R_X86_64_16 | abi::R_X86_64_32 | abi::R_X86_64_32S => {
                RelType::Abs
            }
            _ => panic!("TODO: relocation type: {}", r_type),
        }
    }
}

pub fn relocation_value(symbol_addr: u64, isec_addr: u64, rela: &Rela) -> Option<u64> {
    let s = symbol_addr;
    let a = rela.r_addend;
    let p = isec_addr + rela.r_offset;
    match RelType::from(rela.r_type) {
        RelType::None => None,
        RelType::Pc => Some((s as i64 + a - p as i64) as u64),
        RelType::Abs => Some((s as i64 + a) as u64),
        _ => todo!(),
    }
}

use elf::{abi, relocation::Rela};

#[derive(Debug)]
pub struct RelValue {
    pub file_ofs: usize,
    pub value: u64,
    pub size: usize,
}

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
            abi::R_X86_64_PC32 | abi::R_X86_64_PLT32 => RelType::Pc,
            abi::R_X86_64_8
            | abi::R_X86_64_16
            | abi::R_X86_64_32
            | abi::R_X86_64_32S
            | abi::R_X86_64_64 => RelType::Abs,
            _ => todo!("r_type: {} is not supported", r_type_as_str(r_type)),
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

pub fn relocation_size(rela: &Rela) -> usize {
    match rela.r_type {
        abi::R_X86_64_NONE => 0,
        abi::R_X86_64_8 => 1,
        abi::R_X86_64_16 => 2,
        abi::R_X86_64_32 => 4,
        abi::R_X86_64_32S => 4,
        abi::R_X86_64_64 => 8,
        abi::R_X86_64_PC32 => 4,
        abi::R_X86_64_GOT32 => 4,
        abi::R_X86_64_PLT32 => 4,
        _ => todo!("r_type: {} is not supported", r_type_as_str(rela.r_type)),
    }
}

fn r_type_as_str(r_type: u32) -> &'static str {
    match r_type {
        abi::R_X86_64_NONE => "R_X86_64_NONE",
        abi::R_X86_64_64 => "R_X86_64_64",
        abi::R_X86_64_PC32 => "R_X86_64_PC32",
        abi::R_X86_64_GOT32 => "R_X86_64_GOT32",
        abi::R_X86_64_PLT32 => "R_X86_64_PLT32",
        abi::R_X86_64_COPY => "R_X86_64_COPY",
        abi::R_X86_64_GLOB_DAT => "R_X86_64_GLOB_DAT",
        abi::R_X86_64_JUMP_SLOT => "R_X86_64_JUMP_SLOT",
        abi::R_X86_64_RELATIVE => "R_X86_64_RELATIVE",
        abi::R_X86_64_32 => "R_X86_64_32",
        // 10
        abi::R_X86_64_16 => "R_X86_64_16",
        abi::R_X86_64_8 => "R_X86_64_8",
        abi::R_X86_64_PC8 => "R_X86_64_PC8",
        abi::R_X86_64_PC16 => "R_X86_64_PC16",
        abi::R_X86_64_32S => "R_X86_64_32S",
        abi::R_X86_64_PC64 => "R_X86_64_PC64",
        abi::R_X86_64_TLSGD => "R_X86_64_TLSGD",
        abi::R_X86_64_TLSLD => "R_X86_64_TLSLD",
        // 20
        abi::R_X86_64_DTPOFF32 => "R_X86_64_DTPOFF32",
        abi::R_X86_64_GOTTPOFF => "R_X86_64_GOTTPOFF",
        abi::R_X86_64_TPOFF32 => "R_X86_64_TPOFF32",
        abi::R_X86_64_GOTOFF64 => "R_X86_64_GOTOFF64",
        abi::R_X86_64_GOTPC32 => "R_X86_64_GOTPC32",
        abi::R_X86_64_GOT64 => "R_X86_64_GOT64",
        abi::R_X86_64_GOTPCREL64 => "R_X86_64_GOTPCREL64",
        abi::R_X86_64_GOTPC64 => "R_X86_64_GOTPC64",
        abi::R_X86_64_PLTOFF64 => "R_X86_64_PLTOFF64",
        abi::R_X86_64_SIZE32 => "R_X86_64_SIZE32",
        abi::R_X86_64_SIZE64 => "R_X86_64_SIZE64",
        abi::R_X86_64_GOTPC32_TLSDESC => "R_X86_64_GOTPC32_TLSDESC",
        abi::R_X86_64_TLSDESC_CALL => "R_X86_64_TLSDESC_CALL",
        abi::R_X86_64_TLSDESC => "R_X86_64_TLSDESC",
        abi::R_X86_64_IRELATIVE => "R_X86_64_IRELATIVE",
        abi::R_X86_64_RELATIVE64 => "R_X86_64_RELATIVE64",
        abi::R_X86_64_GOTPCRELX => "R_X86_64_GOTPCRELX",
        abi::R_X86_64_REX_GOTPCRELX => "R_X86_64_REX_GOTPCRELX",
        _ => panic!("TODO: relocation type: {}", r_type),
    }
}

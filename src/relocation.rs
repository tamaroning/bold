use elf::{abi, relocation::Rela};

#[derive(Debug)]
pub struct RelValue {
    pub file_ofs: usize,
    pub value: u64,
    pub size: usize,
}

pub fn relocation_value(symbol_addr: u64, isec_addr: u64, rela: &Rela) -> Option<u64> {
    let s = symbol_addr;
    let a = rela.r_addend;
    let p = isec_addr + rela.r_offset;

    match rela.r_type {
        abi::R_X86_64_NONE => None,
        abi::R_X86_64_PC32 | abi::R_X86_64_PLT32 => Some((s as i64 + a - p as i64) as u64),
        abi::R_X86_64_8
        | abi::R_X86_64_16
        | abi::R_X86_64_32
        | abi::R_X86_64_32S
        | abi::R_X86_64_64 => Some((s as i64 + a) as u64),
        abi::R_X86_64_GOTTPOFF | abi::R_X86_64_GOTPCRELX => {
            log::warn!("{} is not supported, ignored", r_type_as_str(rela.r_type));
            Some(0)
        }
        _ => todo!("r_type: {} is not supported", r_type_as_str(rela.r_type)),
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
        // FIXME: Not sure
        abi::R_X86_64_GOTTPOFF => 4,
        // FIXME: Not sure
        abi::R_X86_64_GOTPCRELX => 4,
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
        abi::R_X86_64_GOTPCREL => "R_X86_64_GOTPCREL",
        abi::R_X86_64_32 => "R_X86_64_32",
        abi::R_X86_64_16 => "R_X86_64_16",
        abi::R_X86_64_8 => "R_X86_64_8",
        abi::R_X86_64_PC8 => "R_X86_64_PC8",
        abi::R_X86_64_PC16 => "R_X86_64_PC16",
        abi::R_X86_64_32S => "R_X86_64_32S",
        abi::R_X86_64_PC64 => "R_X86_64_PC64",
        abi::R_X86_64_TLSGD => "R_X86_64_TLSGD",
        abi::R_X86_64_TLSLD => "R_X86_64_TLSLD",
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

/*
static u32 relax_gottpoff(u8 *loc) {
    switch ((loc[0] << 16) | (loc[1] << 8) | loc[2]) {
    case 0x488b05: return 0x48c7c0; // mov 0(%rip), %rax -> mov $0, %rax
    case 0x488b0d: return 0x48c7c1; // mov 0(%rip), %rcx -> mov $0, %rcx
    case 0x488b15: return 0x48c7c2; // mov 0(%rip), %rdx -> mov $0, %rdx
    case 0x488b1d: return 0x48c7c3; // mov 0(%rip), %rbx -> mov $0, %rbx
    case 0x488b25: return 0x48c7c4; // mov 0(%rip), %rsp -> mov $0, %rsp
    case 0x488b2d: return 0x48c7c5; // mov 0(%rip), %rbp -> mov $0, %rbp
    case 0x488b35: return 0x48c7c6; // mov 0(%rip), %rsi -> mov $0, %rsi
    case 0x488b3d: return 0x48c7c7; // mov 0(%rip), %rdi -> mov $0, %rdi
    case 0x4c8b05: return 0x49c7c0; // mov 0(%rip), %r8  -> mov $0, %r8
    case 0x4c8b0d: return 0x49c7c1; // mov 0(%rip), %r9  -> mov $0, %r9
    case 0x4c8b15: return 0x49c7c2; // mov 0(%rip), %r10 -> mov $0, %r10
    case 0x4c8b1d: return 0x49c7c3; // mov 0(%rip), %r11 -> mov $0, %r11
    case 0x4c8b25: return 0x49c7c4; // mov 0(%rip), %r12 -> mov $0, %r12
    case 0x4c8b2d: return 0x49c7c5; // mov 0(%rip), %r13 -> mov $0, %r13
    case 0x4c8b35: return 0x49c7c6; // mov 0(%rip), %r14 -> mov $0, %r14
    case 0x4c8b3d: return 0x49c7c7; // mov 0(%rip), %r15 -> mov $0, %r15
    }
    return 0;
  }

  */
fn relax_gottpoff(loc: &[u8]) -> u32 {
    match (loc[0] as u32) << 16 | (loc[1] as u32) << 8 | loc[2] as u32 {
        0x488b05 => 0x48c7c0, // mov 0(%rip), %rax -> mov $0, %rax
        0x488b0d => 0x48c7c1, // mov 0(%rip), %rcx -> mov $0, %rcx
        0x488b15 => 0x48c7c2, // mov 0(%rip), %rdx -> mov $0, %rdx
        0x488b1d => 0x48c7c3, // mov 0(%rip), %rbx -> mov $0, %rbx
        0x488b25 => 0x48c7c4, // mov 0(%rip), %rsp -> mov $0, %rsp
        0x488b2d => 0x48c7c5, // mov 0(%rip), %rbp -> mov $0, %rbp
        0x488b35 => 0x48c7c6, // mov 0(%rip), %rsi -> mov $0, %rsi
        0x488b3d => 0x48c7c7, // mov 0(%rip), %rdi -> mov $0, %rdi
        0x4c8b05 => 0x49c7c0, // mov 0(%rip), %r8  -> mov $0, %r8
        0x4c8b0d => 0x49c7c1, // mov 0(%rip), %r9  -> mov $0, %r9
        0x4c8b15 => 0x49c7c2, // mov 0(%rip), %r10 -> mov $0, %r10
        0x4c8b1d => 0x49c7c3, // mov 0(%rip), %r11 -> mov $0, %r11
        0x4c8b25 => 0x49c7c4, // mov 0(%rip), %r12 -> mov $0, %r12
        0x4c8b2d => 0x49c7c5, // mov 0(%rip), %r13 -> mov $0, %r13
        0x4c8b35 => 0x49c7c6, // mov 0(%rip), %r14 -> mov $0, %r14
        0x4c8b3d => 0x49c7c7, // mov 0(%rip), %r15 -> mov $0, %r15
        _ => panic!(),
    }
}

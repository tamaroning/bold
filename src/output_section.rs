use elf::{
    abi::{SHF_ALLOC, SHT_STRTAB},
    file::Elf64_Ehdr,
    section::Elf64_Shdr,
    segment::Elf64_Phdr,
    symbol::Elf64_Sym,
};

use crate::{
    context::{Context, COMMON_SECTION_NAMES},
    dummy,
    input_section::InputSectionId,
};

pub enum OutputChunk {
    Ehdr(OutputEhdr),
    Shdr(OutputShdr),
    Phdr(OutputPhdr),
    Section(OutputSectionId),
    Strtab(Strtab),
    Symtab(Symtab),
    Shstrtab(Shstrtab),
}

impl OutputChunk {
    pub fn get_common<'a>(&'a self, ctx: &'a Context) -> &'a ChunkInfo {
        match self {
            OutputChunk::Ehdr(chunk) => &chunk.common,
            OutputChunk::Shdr(chunk) => &chunk.common,
            OutputChunk::Phdr(chunk) => &chunk.common,
            OutputChunk::Section(osec_id) => {
                let osec = ctx.get_output_section(*osec_id);
                osec.get_common()
            }
            OutputChunk::Strtab(chunk) => &chunk.common,
            OutputChunk::Symtab(chunk) => &chunk.common,
            OutputChunk::Shstrtab(chunk) => &chunk.common,
        }
    }

    pub fn get_common_mut<'a>(&'a mut self, ctx: &'a mut Context) -> &'a mut ChunkInfo {
        match self {
            OutputChunk::Ehdr(chunk) => &mut chunk.common,
            OutputChunk::Shdr(chunk) => &mut chunk.common,
            OutputChunk::Phdr(chunk) => &mut chunk.common,
            OutputChunk::Section(osec_id) => {
                let osec = ctx.get_output_section_mut(*osec_id);
                osec.get_common_mut()
            }
            OutputChunk::Strtab(chunk) => &mut chunk.common,
            OutputChunk::Symtab(chunk) => &mut chunk.common,
            OutputChunk::Shstrtab(chunk) => &mut chunk.common,
        }
    }

    pub fn get_section_name(&self, ctx: &Context) -> String {
        match self {
            OutputChunk::Ehdr(_) => panic!(),
            OutputChunk::Shdr(_) => panic!(),
            OutputChunk::Phdr(_) => panic!(),
            OutputChunk::Section(osec_id) => {
                let osec = ctx.get_output_section(*osec_id);
                osec.get_name()
            }
            OutputChunk::Strtab(_) => ".strtab".to_owned(),
            OutputChunk::Symtab(_) => ".symtab".to_owned(),
            OutputChunk::Shstrtab(_) => ".shstrtab".to_owned(),
        }
    }

    pub fn set_offset(&mut self, ctx: &mut Context, mut offset: u64) {
        match self {
            OutputChunk::Ehdr(chunk) => chunk.common.shdr.sh_offset = offset,
            OutputChunk::Shdr(chunk) => chunk.common.shdr.sh_offset = offset,
            OutputChunk::Phdr(chunk) => chunk.common.shdr.sh_offset = offset,
            OutputChunk::Section(osec_id) => {
                let osec = ctx.get_output_section_mut(*osec_id);
                let offset_start = offset;
                osec.common.shdr.sh_offset = offset;

                for input_section in osec.sections.clone() {
                    let input_section = ctx.get_input_section_mut(input_section);
                    input_section.set_offset(offset);
                    offset += input_section.get_size();
                }

                let osec = ctx.get_output_section_mut(*osec_id);
                osec.common.shdr.sh_size = offset - offset_start;
            }
            OutputChunk::Strtab(chunk) => chunk.common.shdr.sh_offset = offset,
            OutputChunk::Symtab(chunk) => chunk.common.shdr.sh_offset = offset,
            OutputChunk::Shstrtab(chunk) => chunk.common.shdr.sh_offset = offset,
        }
    }

    pub fn is_header(&self) -> bool {
        matches!(
            self,
            OutputChunk::Ehdr(_) | OutputChunk::Shdr(_) | OutputChunk::Phdr(_)
        )
    }

    pub fn as_string(&self, ctx: &Context) -> String {
        (match self {
            OutputChunk::Ehdr(_) => "Ehdr ".to_owned(),
            OutputChunk::Shdr(_) => "Shdr ".to_owned(),
            OutputChunk::Phdr(_) => "Phdr ".to_owned(),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section(*chunk);
                chunk.as_string()
            }
            OutputChunk::Strtab(_) => "Strtab ".to_owned(),
            OutputChunk::Symtab(_) => "Symtab ".to_owned(),
            OutputChunk::Shstrtab(_) => "Shstrtab ".to_owned(),
        }) + &self.get_common(ctx).as_string()
    }
}

#[derive(Debug)]
pub struct ChunkInfo {
    pub shdr: Elf64_Shdr,
    pub shndx: Option<usize>,
}

impl ChunkInfo {
    pub fn new() -> ChunkInfo {
        let mut shdr: Elf64_Shdr = dummy!(Elf64_Shdr);
        shdr.sh_addralign = 1;
        ChunkInfo { shdr, shndx: None }
    }

    pub fn as_string(&self) -> String {
        format!(
            "(sh_type={}, sh_flags={}, sh_offset={}, sh_size={}, sh_name={})",
            self.shdr.sh_type,
            self.shdr.sh_flags,
            self.shdr.sh_offset,
            self.shdr.sh_size,
            self.shdr.sh_name
        )
    }
}

pub struct OutputEhdr {
    common: ChunkInfo,
}

impl OutputEhdr {
    pub fn new() -> OutputEhdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        common.shdr.sh_size = std::mem::size_of::<Elf64_Ehdr>() as u64;
        OutputEhdr { common }
    }
}

impl OutputEhdr {
    #[allow(clippy::too_many_arguments)]
    pub fn copy_buf(
        &self,
        buf: &mut [u8],
        e_entry: u64,
        e_phoff: u64,
        e_shoff: u64,
        e_phnum: u16,
        e_shnum: u16,
        e_shstrndx: u16,
    ) {
        use elf::abi::*;

        let mut ehdr: Elf64_Ehdr = dummy!(Elf64_Ehdr);
        ehdr.e_ident[EI_MAG0] = ELFMAG0;
        ehdr.e_ident[EI_MAG1] = ELFMAG1;
        ehdr.e_ident[EI_MAG2] = ELFMAG2;
        ehdr.e_ident[EI_MAG3] = ELFMAG3;
        ehdr.e_ident[EI_CLASS] = ELFCLASS64;
        ehdr.e_ident[EI_DATA] = ELFDATA2LSB;
        ehdr.e_ident[EI_VERSION] = EV_CURRENT;
        ehdr.e_type = ET_EXEC; // FIXME: PIE
        ehdr.e_machine = EM_X86_64;
        ehdr.e_version = EV_CURRENT as u32;
        ehdr.e_entry = e_entry;
        ehdr.e_phoff = e_phoff;
        ehdr.e_shoff = e_shoff;
        ehdr.e_ehsize = std::mem::size_of::<Elf64_Ehdr>() as u16;
        ehdr.e_phentsize = std::mem::size_of::<Elf64_Phdr>() as u16;
        ehdr.e_phnum = e_phnum;
        ehdr.e_shentsize = std::mem::size_of::<Elf64_Shdr>() as u16;
        ehdr.e_shnum = e_shnum;
        ehdr.e_shstrndx = e_shstrndx;

        let view = &ehdr as *const _ as *const u8;
        let offset = self.common.shdr.sh_offset as usize;
        let size = std::mem::size_of::<Elf64_Ehdr>();
        let data = unsafe { std::slice::from_raw_parts(view, size) };
        buf[offset..offset + size].copy_from_slice(data);
    }
}

pub struct OutputShdr {
    pub common: ChunkInfo,
}

impl OutputShdr {
    pub fn new() -> OutputShdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        OutputShdr { common }
    }

    pub fn update_shdr(&mut self, n: usize) {
        self.common.shdr.sh_size = (n * std::mem::size_of::<Elf64_Shdr>()) as u64;
    }
}

pub struct OutputPhdr {
    common: ChunkInfo,
}

impl OutputPhdr {
    pub fn new() -> OutputPhdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        OutputPhdr { common }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct OutputSectionId {
    private: usize,
}

fn get_next_output_section_id() -> OutputSectionId {
    static mut OUTPUT_SECTION_ID: usize = 0;
    let id = unsafe { OUTPUT_SECTION_ID };
    unsafe { OUTPUT_SECTION_ID += 1 };
    OutputSectionId { private: id }
}

#[derive(Debug)]
pub struct OutputSection {
    id: OutputSectionId,
    common: ChunkInfo,
    name: String,
    pub sections: Vec<InputSectionId>,
}

impl OutputSection {
    pub fn new(name: String, sh_type: u32, sh_flags: u64) -> OutputSection {
        let mut common = ChunkInfo::new();
        common.shdr.sh_type = sh_type;
        common.shdr.sh_flags = sh_flags;
        OutputSection {
            id: get_next_output_section_id(),
            common,
            name,
            sections: vec![],
        }
    }

    pub fn get_id(&self) -> OutputSectionId {
        self.id
    }

    pub fn get_common(&self) -> &ChunkInfo {
        &self.common
    }

    pub fn get_common_mut(&mut self) -> &mut ChunkInfo {
        &mut self.common
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn copy_buf(&self, ctx: &Context, buf: &mut [u8]) {
        for input_section in self.sections.iter() {
            let input_section = ctx.get_input_section(*input_section);
            input_section.copy_buf(buf);
        }
    }

    fn as_string(&self) -> String {
        format!(
            "OutputSection \"{}\" (containing {} sections)",
            self.name,
            self.sections.len()
        )
    }
}

pub fn get_output_section_name(input_section: &String) -> String {
    for common_section_name in &COMMON_SECTION_NAMES {
        if *input_section == **common_section_name
            || input_section.starts_with(&format!("{common_section_name}."))
        {
            return common_section_name.to_string();
        }
    }
    panic!("Unknown section: \"{}\"", input_section);
}

pub struct Shstrtab {
    pub common: ChunkInfo,
}

impl Shstrtab {
    pub fn new() -> Shstrtab {
        let mut common = ChunkInfo::new();
        common.shdr.sh_type = SHT_STRTAB;
        Shstrtab { common }
    }

    pub fn update_shdr(&mut self, shstrtab_size: u64) {
        self.common.shdr.sh_size = shstrtab_size;
    }

    pub fn copy_buf(&self, buf: &mut [u8], data: &[u8]) {
        let offset = self.common.shdr.sh_offset as usize;
        buf[offset..offset + data.len()].copy_from_slice(data);
    }
}

pub struct Symtab {
    pub common: ChunkInfo,
}

impl Symtab {
    pub fn new() -> Symtab {
        let mut common = ChunkInfo::new();
        common.shdr.sh_type = elf::abi::SHT_SYMTAB;
        common.shdr.sh_entsize = std::mem::size_of::<elf::symbol::Elf64_Sym>() as u64;
        common.shdr.sh_addralign = 8;
        // NULL symbol
        common.shdr.sh_size = std::mem::size_of::<elf::symbol::Elf64_Sym>() as u64;
        Symtab { common }
    }

    pub fn update_shdr(&mut self, num_sym: u64) {
        self.common.shdr.sh_size = num_sym * std::mem::size_of::<elf::symbol::Elf64_Sym>() as u64;
    }

    pub fn copy_buf(&self, buf: &mut [u8], data: &[Elf64_Sym]) {
        let mut offset = self.common.shdr.sh_offset as u64;
        // TODO: NULL symbol
        for sym in data {
            let size = std::mem::size_of::<Elf64_Sym>();
            let view = sym as *const _ as *const u8;
            let slice = unsafe { std::slice::from_raw_parts(view, size) };
            buf[offset as usize..offset as usize + size].copy_from_slice(slice);
            offset += size as u64;
        }
    }
}

pub struct Strtab {
    pub common: ChunkInfo,
}

impl Strtab {
    pub fn new() -> Strtab {
        let mut common = ChunkInfo::new();
        common.shdr.sh_type = elf::abi::SHT_STRTAB;
        Strtab { common }
    }

    pub fn update_shdr(&mut self, strtab_size: u64) {
        self.common.shdr.sh_size = strtab_size;
    }

    pub fn copy_buf(&self, buf: &mut [u8], data: &[u8]) {
        let offset = self.common.shdr.sh_offset as usize;
        buf[offset..offset + data.len()].copy_from_slice(data);
    }
}

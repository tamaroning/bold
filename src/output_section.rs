use std::{cell::RefCell, sync::Arc};

use elf::{abi::SHF_ALLOC, file::Elf64_Ehdr, section::Elf64_Shdr, segment::Elf64_Phdr};

use crate::{
    context::{Context, COMMON_SECTION_NAMES},
    dummy,
    input_section::InputSectionId,
};

// TODO: We need to add some sort of Elf_Shdr to this
// https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/mold.h#L417

pub enum OutputChunk {
    Ehdr(OutputEhdr),
    Shdr(OutputShdr),
    Phdr(OutputPhdr),
    Section(OutputSectionId),
}

impl OutputChunk {
    pub fn is_header(&self) -> bool {
        match self {
            OutputChunk::Ehdr(_) | OutputChunk::Shdr(_) | OutputChunk::Phdr(_) => true,
            _ => false,
        }
    }

    pub fn get_offset(&self, ctx: &Context) -> usize {
        match self {
            OutputChunk::Ehdr(chunk) => chunk.offset.unwrap(),
            OutputChunk::Shdr(chunk) => chunk.offset.unwrap(),
            OutputChunk::Phdr(chunk) => chunk.offset.unwrap(),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section(*chunk);
                chunk.offset.unwrap()
            }
        }
    }

    pub fn set_offset(&mut self, ctx: &mut Context, mut offset: usize) {
        match self {
            OutputChunk::Ehdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Shdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Phdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Section(osec_id) => {
                let osec = ctx.get_output_section_mut(*osec_id);
                let offset_start = offset;
                osec.offset = Some(offset);

                for input_section in osec.sections.clone() {
                    let input_section = ctx.get_input_section_mut(input_section);
                    input_section.set_offset(offset);
                    offset += input_section.get_size();
                }

                let osec = ctx.get_output_section_mut(*osec_id);
                osec.size = Some(offset - offset_start);
            }
        }
    }

    pub fn get_size(&self, ctx: &Context) -> usize {
        match self {
            OutputChunk::Ehdr(_) => OutputEhdr::get_size(),
            OutputChunk::Shdr(chunk) => chunk.get_size(),
            OutputChunk::Phdr(chunk) => chunk.get_size(),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section(*chunk);
                chunk.get_size()
            }
        }
    }

    pub fn as_string(&self, ctx: &Context) -> String {
        match self {
            OutputChunk::Ehdr(_) => "Ehdr".to_owned(),
            OutputChunk::Shdr(_) => "Shdr".to_owned(),
            OutputChunk::Phdr(_) => "Phdr".to_owned(),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section(*chunk);
                chunk.as_string()
            }
        }
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
}

pub struct OutputEhdr {
    common: ChunkInfo,
    offset: Option<usize>,
}

impl OutputEhdr {
    pub fn new() -> OutputEhdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        common.shdr.sh_size = Self::get_size() as u64;
        OutputEhdr {
            common,
            offset: None,
        }
    }
}

impl OutputEhdr {
    pub fn copy_to(&self, buf: &mut [u8]) {
        use elf::abi::*;

        let mut ehdr: Elf64_Ehdr = dummy!(Elf64_Ehdr);
        ehdr.e_ident[EI_CLASS] = ELFCLASS64;
        ehdr.e_ident[EI_DATA] = ELFDATA2LSB;
        ehdr.e_ident[EI_VERSION] = EV_CURRENT;
        ehdr.e_type = ET_EXEC; // TODO: PIE
        ehdr.e_machine = EM_X86_64;
        ehdr.e_version = EV_CURRENT as u32;
        ehdr.e_entry = 0x400000; // TODO: entry point
                                 // TODO: rest of the fields

        let view = &ehdr as *const _ as *const u8;
        let offset = self.offset.unwrap();
        let size = Self::get_size();
        let data = unsafe { std::slice::from_raw_parts(view, size) };
        buf[offset..offset + size].copy_from_slice(data);
    }

    fn get_size() -> usize {
        std::mem::size_of::<Elf64_Ehdr>()
    }
}

pub struct OutputShdr {
    common: ChunkInfo,
    offset: Option<usize>,

    // TODO: remove
    shdrs: Vec<Elf64_Shdr>,
}

impl OutputShdr {
    pub fn new() -> OutputShdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        OutputShdr {
            common,
            offset: None,
            shdrs: vec![],
        }
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        if self.shdrs.is_empty() {
            return;
        }
        let view = &self.shdrs[0] as *const _ as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(view, self.get_size()) };
        buf.copy_from_slice(slice);
    }

    pub fn update_shdr(&mut self, chunks: &Vec<Arc<RefCell<OutputChunk>>>) {
        let mut num_section = 0;
        for chunk in chunks.iter() {
            if let Ok(chunk) = chunk.try_borrow() {
                if !chunk.is_header() {
                    num_section += 1;
                }
            }
        }
    }

    fn get_size(&self) -> usize {
        self.shdrs.len() * std::mem::size_of::<Elf64_Shdr>()
    }
}

pub struct OutputPhdr {
    common: ChunkInfo,
    offset: Option<usize>,
    phdrs: Vec<Elf64_Phdr>,
}

impl OutputPhdr {
    pub fn new() -> OutputPhdr {
        let mut common = ChunkInfo::new();
        common.shdr.sh_flags = SHF_ALLOC as u64;
        OutputPhdr {
            common,
            offset: None,
            phdrs: vec![],
        }
    }

    pub fn add_phdr(&mut self, phdr: Elf64_Phdr) {
        self.phdrs.push(phdr);
    }

    fn get_size(&self) -> usize {
        self.phdrs.len() * std::mem::size_of::<Elf64_Phdr>()
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        if self.phdrs.is_empty() {
            return;
        }
        let view = &self.phdrs[0] as *const _ as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(view, self.get_size()) };
        buf.copy_from_slice(slice);
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
    offset: Option<usize>,
    size: Option<usize>,
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
            offset: None,
            size: None,
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

    fn get_size(&self) -> usize {
        self.size.unwrap()
    }

    pub fn copy_to(&self, ctx: &Context, buf: &mut [u8]) {
        for input_section in self.sections.iter() {
            let input_section = ctx.get_input_section(*input_section);
            input_section.copy_to(buf);
        }
    }

    fn as_string(&self) -> String {
        format!(
            "OutputSection \"{}\" (sh_type={}, sh_flags={}, containing {} sections)",
            self.name,
            self.common.shdr.sh_type,
            self.common.shdr.sh_flags,
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

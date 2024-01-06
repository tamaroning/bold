use std::{
    cell::RefCell,
    sync::{Arc, RwLock},
};

use elf::{file::Elf64_Ehdr, section::Elf64_Shdr, segment::Elf64_Phdr};

use crate::{dummy, input_section::InputSection};

pub enum OutputChunk {
    Ehdr(OutputEhdr),
    Shdr(OutputShdr),
    Phdr(OutputPhdr),
    Section(Arc<RefCell<OutputSection>>),
}

impl Chunk for OutputChunk {
    fn get_name(&self) -> String {
        match self {
            OutputChunk::Ehdr(ehdr) => ehdr.get_name(),
            OutputChunk::Shdr(shdr) => shdr.get_name(),
            OutputChunk::Phdr(phdr) => phdr.get_name(),
            OutputChunk::Section(section) => {
                let section = section.borrow();
                section.get_name()
            }
        }
    }

    fn get_size(&self) -> usize {
        match self {
            OutputChunk::Ehdr(ehdr) => ehdr.get_size(),
            OutputChunk::Shdr(shdr) => shdr.get_size(),
            OutputChunk::Phdr(phdr) => phdr.get_size(),
            OutputChunk::Section(section) => {
                let section = section.borrow();
                section.get_size()
            }
        }
    }

    fn get_offset(&self) -> usize {
        match self {
            OutputChunk::Ehdr(ehdr) => ehdr.get_offset(),
            OutputChunk::Shdr(shdr) => shdr.get_offset(),
            OutputChunk::Phdr(phdr) => phdr.get_offset(),
            OutputChunk::Section(section) => {
                let section = section.borrow();
                section.get_offset()
            }
        }
    }

    fn set_offset(&mut self, offset: usize) {
        match self {
            OutputChunk::Ehdr(ehdr) => ehdr.set_offset(offset),
            OutputChunk::Shdr(shdr) => shdr.set_offset(offset),
            OutputChunk::Phdr(phdr) => phdr.set_offset(offset),
            OutputChunk::Section(section) => {
                let mut section = section.borrow_mut();
                section.set_offset(offset);
            }
        }
    }

    fn as_string(&self) -> String {
        match self {
            OutputChunk::Ehdr(ehdr) => ehdr.as_string(),
            OutputChunk::Shdr(shdr) => shdr.as_string(),
            OutputChunk::Phdr(phdr) => phdr.as_string(),
            OutputChunk::Section(section) => {
                let section = section.borrow();
                section.as_string()
            }
        }
    }
}

pub trait Chunk {
    fn get_name(&self) -> String;
    fn get_size(&self) -> usize;
    fn get_offset(&self) -> usize;
    /// Set size and offset
    fn set_offset(&mut self, offset: usize);
    fn as_string(&self) -> String;
}

pub struct OutputEhdr {
    offset: Option<usize>,
}

impl OutputEhdr {
    pub fn new() -> OutputEhdr {
        OutputEhdr { offset: None }
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
        let offset = self.get_offset();
        let size = self.get_size();
        let data = unsafe { std::slice::from_raw_parts(view, size) };
        buf[offset..offset + size].copy_from_slice(data);
    }
}

impl Chunk for OutputEhdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_size(&self) -> usize {
        std::mem::size_of::<Elf64_Ehdr>()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = Some(offset);
    }

    fn as_string(&self) -> String {
        format!("OutputEhdr")
    }
}

pub struct OutputShdr {
    offset: Option<usize>,
    shdrs: Vec<Elf64_Shdr>,
}

impl OutputShdr {
    pub fn new() -> OutputShdr {
        OutputShdr {
            offset: None,
            shdrs: vec![],
        }
    }

    pub fn add_shdr(&mut self, shdr: Elf64_Shdr) {
        self.shdrs.push(shdr);
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        if self.shdrs.is_empty() {
            return;
        }
        let view = &self.shdrs[0] as *const _ as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(view, self.get_size()) };
        buf.copy_from_slice(slice);
    }
}

impl Chunk for OutputShdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_size(&self) -> usize {
        self.shdrs.len() * std::mem::size_of::<Elf64_Shdr>()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = Some(offset);
    }

    fn as_string(&self) -> String {
        format!("OutputShdr")
    }
}

pub struct OutputPhdr {
    offset: Option<usize>,
    phdrs: Vec<Elf64_Phdr>,
}

impl OutputPhdr {
    pub fn new() -> OutputPhdr {
        OutputPhdr {
            offset: None,
            phdrs: vec![],
        }
    }

    pub fn add_phdr(&mut self, phdr: Elf64_Phdr) {
        self.phdrs.push(phdr);
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

impl Chunk for OutputPhdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_size(&self) -> usize {
        self.phdrs.len() * std::mem::size_of::<Elf64_Phdr>()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = Some(offset);
    }

    fn as_string(&self) -> String {
        format!("OutputPhdr")
    }
}

#[derive(Debug, Clone)]
pub struct OutputSection {
    name: String,
    pub sections: Vec<Arc<RwLock<InputSection>>>,
    offset: Option<usize>,
    size: Option<usize>,
}

const COMMON_SECTION_NAMES: [&str; 12] = [
    ".text",
    ".data",
    ".data.rel.ro",
    ".rodata",
    ".bss",
    ".bss.rel.ro",
    ".ctors",
    ".dtors",
    ".init_array",
    ".fini_array",
    ".tbss",
    ".tdata",
];

pub struct OutputSectionInstance {
    sections: Vec<Arc<RefCell<OutputSection>>>,
}

impl OutputSectionInstance {
    pub fn new() -> OutputSectionInstance {
        OutputSectionInstance {
            sections: COMMON_SECTION_NAMES
                .iter()
                .map(|name| OutputSection::new(name.to_string()))
                .map(RefCell::new)
                .map(Arc::new)
                .collect(),
        }
    }

    pub fn get_section_by_name(&self, name: &String) -> Arc<RefCell<OutputSection>> {
        for section_ref in self.sections.iter() {
            let section = section_ref.borrow();
            if section.name == *name {
                return Arc::clone(section_ref);
            }
        }
        panic!()
    }
}

impl OutputSection {
    fn new(name: String) -> OutputSection {
        OutputSection {
            name,
            sections: vec![],
            offset: None,
            size: None,
        }
    }

    pub fn get_output_name(input_section: &String) -> String {
        for common_section_name in &COMMON_SECTION_NAMES {
            if *input_section == **common_section_name
                || input_section.starts_with(&format!("{common_section_name}."))
            {
                return common_section_name.to_string();
            }
        }
        panic!("Unknown section: \"{}\"", input_section);
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        for input_section in self.sections.iter() {
            let input_section = input_section.read().unwrap();
            input_section.copy_to(buf);
        }
    }
}

impl Chunk for OutputSection {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_size(&self) -> usize {
        self.size.unwrap()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    fn set_offset(&mut self, mut offset: usize) {
        // TODO: alignment?
        let offset_start = offset;
        self.offset = Some(offset);
        for input_section in self.sections.iter() {
            let mut input_section = input_section.write().unwrap();
            input_section.set_offset(offset);
            offset += input_section.get_size();
        }
        self.size = Some(offset - offset_start);
    }

    fn as_string(&self) -> String {
        let input_sections_str = self
            .sections
            .iter()
            .map(|input_section| {
                let input_section = input_section.read().unwrap();
                format!("\"{}\"", input_section.elf_section.name.clone())
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("OutputSection \"{}\" [{}]", self.name, input_sections_str)
    }
}

pub struct Shstrtab {
    offset: Option<usize>,
    strings: Vec<String>,
}

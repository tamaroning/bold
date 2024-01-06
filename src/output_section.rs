use std::sync::{Arc, OnceLock, RwLock};

use elf::{file::Elf64_Ehdr, section::Elf64_Shdr, segment::Elf64_Phdr};

use crate::input_section::InputSection;

pub trait OutputChunk {
    fn get_name(&self) -> String;
    fn get_kind(&self) -> ChunkKind;
    fn get_size(&self) -> usize;
    fn get_offset(&self) -> usize;
    /// Set size and offset
    fn set_offset(&mut self, offset: usize);
    //fn update_shdr(&mut self);
    fn copy_to(&self, buf: &mut [u8]);
    fn relocate(&mut self, relocs: &[u8]);
    fn as_string(&self) -> String;
}

pub enum ChunkKind {
    Header,
    Regular,
    Synthetic,
}

pub struct OutputEhdr {
    offset: Option<usize>,
}

impl OutputEhdr {
    pub fn new() -> OutputEhdr {
        OutputEhdr { offset: None }
    }
}

impl OutputChunk for OutputEhdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_kind(&self) -> ChunkKind {
        ChunkKind::Header
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

    fn copy_to(&self, _buf: &mut [u8]) {
        // Do nothing
        // Copy is done in relocate()
    }

    fn relocate(&mut self, _relocs: &[u8]) {
        todo!()
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
}

impl OutputChunk for OutputShdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_kind(&self) -> ChunkKind {
        ChunkKind::Header
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

    fn copy_to(&self, buf: &mut [u8]) {
        return;
        // TODO:
        assert!(self.shdrs.len() > 0);
        let view = &self.shdrs[0] as *const _ as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(view, self.get_size()) };
        buf.copy_from_slice(slice);
    }

    fn relocate(&mut self, _relocs: &[u8]) {
        // Do nothing
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
}

impl OutputChunk for OutputPhdr {
    fn get_name(&self) -> String {
        "".to_owned()
    }

    fn get_kind(&self) -> ChunkKind {
        ChunkKind::Header
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

    fn copy_to(&self, buf: &mut [u8]) {
        // TODO:
        return;
        assert!(self.phdrs.len() > 0);
        let view = &self.phdrs[0] as *const _ as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(view, self.get_size()) };
        buf.copy_from_slice(slice);
    }

    fn relocate(&mut self, _relocs: &[u8]) {
        // Do nothing
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

impl OutputSection {
    fn new(name: String) -> OutputSection {
        OutputSection {
            name,
            sections: vec![],
            offset: None,
            size: None,
        }
    }

    pub fn from_section_name(section_name: &String) -> Arc<RwLock<OutputSection>> {
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
        static COMMON_SECTIONS: OnceLock<Vec<Arc<RwLock<OutputSection>>>> = OnceLock::new();
        let common_sections = COMMON_SECTIONS.get_or_init(|| {
            COMMON_SECTION_NAMES
                .iter()
                .map(|name| OutputSection::new(name.to_string()))
                .map(RwLock::new)
                .map(Arc::new)
                .collect()
        });

        for common_section_ref in common_sections.iter() {
            let common_section = common_section_ref.read().unwrap();
            if *section_name == common_section.name
                || section_name.starts_with(&format!("{section_name}."))
            {
                return Arc::clone(common_section_ref);
            }
        }
        panic!("Unknown section: \"{}\"", section_name);
    }
}

impl OutputChunk for OutputSection {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_kind(&self) -> ChunkKind {
        ChunkKind::Regular
    }

    fn get_size(&self) -> usize {
        self.size.unwrap()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    fn set_offset(&mut self, mut offset: usize) {
        let offset_start = offset;
        self.offset = Some(offset);
        for input_section in self.sections.iter() {
            let mut input_section = input_section.write().unwrap();
            input_section.set_offset(offset);
            offset += input_section.get_size();
        }
        self.size = Some(offset - offset_start);
    }

    fn copy_to(&self, buf: &mut [u8]) {
        for input_section in self.sections.iter() {
            let input_section = input_section.read().unwrap();
            input_section.copy_to(buf);
        }
    }

    fn relocate(&mut self, _relocs: &[u8]) {
        todo!()
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

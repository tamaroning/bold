use std::sync::{Arc, OnceLock, RwLock};

use elf::file::Elf64_Ehdr;

use crate::input_section::InputSection;

pub trait OutputChunk {
    fn get_size(&self) -> usize;
    fn get_offset(&self) -> usize;
    /// Set size and offset
    fn set_offset(&mut self, offset: usize);
    fn copy_to(&self, buf: &mut [u8]);
    fn relocate(&mut self, relocs: &[u8]);
    fn as_string(&self) -> String;
}

pub struct OutputEhdr {
    offset: usize,
}

impl OutputChunk for OutputEhdr {
    fn get_size(&self) -> usize {
        std::mem::size_of::<Elf64_Ehdr>()
    }

    fn get_offset(&self) -> usize {
        self.offset
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    fn copy_to(&self, _buf: &mut [u8]) {}

    fn relocate(&mut self, _relocs: &[u8]) {
        todo!()
    }

    fn as_string(&self) -> String {
        format!("OutputEhdr")
    }
}

#[derive(Debug, Clone)]
pub struct OutputSection {
    name: String,
    pub sections: Vec<Arc<RwLock<InputSection>>>,
    offset: usize,
    size: usize,
}

impl OutputSection {
    fn new(name: String) -> OutputSection {
        OutputSection {
            name,
            sections: vec![],
            offset: 0,
            size: 0,
        }
    }

    pub fn get_name(&self) -> &String {
        &self.name
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
    fn get_size(&self) -> usize {
        self.size
    }

    fn get_offset(&self) -> usize {
        self.offset
    }

    fn set_offset(&mut self, mut offset: usize) {
        let offset_start = offset;
        self.offset = offset;
        for input_section in self.sections.iter() {
            let mut input_section = input_section.write().unwrap();
            input_section.set_offset(offset);
            offset += input_section.get_size();
        }
        self.size = offset - offset_start;
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

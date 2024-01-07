use std::{cell::RefCell, sync::Arc};

use elf::{file::Elf64_Ehdr, section::Elf64_Shdr, segment::Elf64_Phdr};

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
                chunk.get_offset()
            }
        }
    }

    pub fn set_offset(&mut self, ctx: &mut Context, offset: usize) {
        match self {
            OutputChunk::Ehdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Shdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Phdr(chunk) => chunk.offset = Some(offset),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section_mut(*chunk);
                chunk.set_offset(ctx, offset);
            }
        }
    }

    pub fn get_size(&self, ctx: &Context) -> usize {
        match self {
            OutputChunk::Ehdr(chunk) => chunk.get_size(),
            OutputChunk::Shdr(chunk) => chunk.get_size(),
            OutputChunk::Phdr(chunk) => chunk.get_size(),
            OutputChunk::Section(chunk) => {
                let chunk = ctx.get_output_section(*chunk);
                chunk.size.unwrap()
            }
        }
    }
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
        let offset = self.offset.unwrap();
        let size = self.get_size();
        let data = unsafe { std::slice::from_raw_parts(view, size) };
        buf[offset..offset + size].copy_from_slice(data);
    }

    fn get_size(&self) -> usize {
        std::mem::size_of::<Elf64_Ehdr>()
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

#[derive(Debug, Clone)]
pub struct OutputSection {
    id: OutputSectionId,
    name: String,
    pub sections: Vec<InputSectionId>,
    private_offset: Option<usize>,
    size: Option<usize>,
}

impl OutputSection {
    pub fn new(name: String) -> OutputSection {
        OutputSection {
            id: get_next_output_section_id(),
            name,
            sections: vec![],
            private_offset: None,
            size: None,
        }
    }

    pub fn get_id(&self) -> OutputSectionId {
        self.id
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
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

    fn get_offset(&self) -> usize {
        self.private_offset.unwrap()
    }

    fn set_offset(&mut self, ctx: &mut Context, mut offset: usize) {
        // TODO: alignment?
        let offset_start = offset;
        self.private_offset = Some(offset);
        for input_section in self.sections.iter() {
            let input_section = ctx.get_input_section_mut(*input_section);
            input_section.set_offset(offset);
            offset += input_section.get_size();
        }
        self.size = Some(offset - offset_start);
    }

    pub fn copy_to(&self, ctx: &Context, buf: &mut [u8]) {
        for input_section in self.sections.iter() {
            let input_section = ctx.get_input_section(*input_section);
            input_section.copy_to(buf);
        }
    }

    fn as_string(&self, ctx: &Context) -> String {
        let input_sections_str = self
            .sections
            .iter()
            .map(|input_section| {
                let input_section = ctx.get_input_section(*input_section);
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

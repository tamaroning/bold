use std::sync::{Arc, RwLock};

use crate::{context::ObjectId, output_section::OutputSection};
use elf::{endian::AnyEndian, section::SectionHeader, symbol::Symbol as ElfSymbolData, ElfBytes};

macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

pub struct ObjectFile {
    file_name: String,
    // TODO: archive file
    data: Vec<u8>,

    // Elf sections and symbols
    elf_symtab: SectionHeader,
    first_global: usize,
    elf_sections: Vec<Arc<ElfSection>>,
    elf_symbols: Vec<ElfSymbol>,

    input_sections: Vec<Option<Arc<RwLock<InputSection>>>>,
    symbols: Vec<Option<Symbol>>,
}

impl ObjectFile {
    pub fn read_from(file_name: String) -> ObjectFile {
        // TODO: We should use mmap here
        let data = std::fs::read(file_name.clone()).unwrap();
        ObjectFile {
            file_name,
            data,
            elf_symtab: dummy!(SectionHeader),
            first_global: 0,
            elf_sections: Vec::new(),
            elf_symbols: Vec::new(),
            input_sections: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn get_file_name(&self) -> &str {
        &self.file_name
    }

    pub fn get_elf_sections(&self) -> &[Arc<ElfSection>] {
        &self.elf_sections
    }

    pub fn get_elf_symbols(&self) -> &[ElfSymbol] {
        &self.elf_symbols
    }

    pub fn get_input_sections(&self) -> &[Option<Arc<RwLock<InputSection>>>] {
        &self.input_sections
    }

    pub fn get_symbols(&self) -> &[Option<Symbol>] {
        &self.symbols
    }

    pub fn parse(&mut self) {
        let file = ElfBytes::<AnyEndian>::minimal_parse(&self.data).expect("Open ELF file failed");

        let shstrtab_shdr = file.section_header_by_name(".shstrtab").unwrap().unwrap();
        let shstrtab = file.section_data_as_strtab(&shstrtab_shdr).unwrap();
        let section_headers = file.section_headers().unwrap();
        // Arrange elf_sections
        for shdr in section_headers.into_iter() {
            let name = shstrtab.get(shdr.sh_name as usize).unwrap();
            // TODO: remove clone()
            self.elf_sections.push(Arc::new(ElfSection {
                name: name.to_string(),
                header: shdr.clone(),
                data: file.section_data(&shdr).unwrap().0.to_vec(),
            }));
        }

        // Arrange elf_symbols
        let (symtab_sec, strtab_sec) = file.symbol_table().unwrap().unwrap();
        // TODO: Use .dsymtab instead of .symtab for dso
        let symtab_shdr = file.section_header_by_name(".symtab").unwrap().unwrap();
        for sym in symtab_sec {
            let name = strtab_sec.get(sym.st_name as usize).unwrap();
            self.elf_symbols.push(ElfSymbol {
                name: name.to_string(),
                sym,
            });
        }

        self.elf_symtab = symtab_shdr;
        self.first_global = symtab_shdr.sh_info as usize;

        self.initialize_sections();
        self.initialize_symbols();
    }

    fn initialize_sections(&mut self) {
        self.input_sections.resize(self.elf_sections.len(), None);
        for (i, elf_section) in self.elf_sections.iter().enumerate() {
            match elf_section.header.sh_type {
                elf::abi::SHT_NULL
                | elf::abi::SHT_REL
                | elf::abi::SHT_RELA
                | elf::abi::SHT_SYMTAB
                | elf::abi::SHT_STRTAB => {
                    // Nothing to do
                }
                elf::abi::SHT_SYMTAB_SHNDX => panic!("SHT_SYMTAB_SHNDX is not supported"),
                elf::abi::SHT_GROUP => {
                    todo!("TODO:")
                }
                _ => {
                    let input_section = InputSection::new(Arc::clone(elf_section));
                    self.input_sections[i] = Some(Arc::new(RwLock::new(input_section)));
                }
            }
        }
    }

    fn initialize_symbols(&mut self) {
        self.symbols.resize(self.elf_symbols.len(), None);
        for (i, elf_symbol) in self.elf_symbols.iter().enumerate() {
            // Skip until reaching the first global
            if i < self.first_global as usize {
                continue;
            }
            self.symbols[i] = Some(Symbol {
                name: elf_symbol.name.clone(),
                file: None,
            });
        }
    }

    pub fn register_defined_symbols(&mut self, this_file_id: ObjectId) {
        for (i, symbol) in self.symbols.iter_mut().enumerate() {
            let esym = &self.elf_symbols[i];
            if esym.sym.is_undefined() {
                continue;
            }
            let Some(symbol) = symbol else {
                continue;
            };

            symbol.file = Some(this_file_id);
            // TODO: visibility
        }
    }

    pub fn register_undefined_symbols(&mut self) {
        for (esym, symbol) in self.elf_symbols.iter().zip(self.symbols.iter()) {
            if esym.sym.is_undefined() {
                continue;
            }
            let Some(symbol) = symbol else {
                continue;
            };

            // TODO: do something for an archive file
        }
    }
}

pub struct ElfSection {
    pub name: String,
    pub header: SectionHeader,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for ElfSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Section").field("name", &self.name).finish()
    }
}

#[derive(Debug, Clone)]
pub struct InputSection {
    pub elf_section: Arc<ElfSection>,
    pub output_section_name: String,
    offset: Option<usize>,
}

impl InputSection {
    fn new(elf_section: Arc<ElfSection>) -> InputSection {
        let output_section_name = OutputSection::get_output_name(&elf_section.name);
        InputSection {
            elf_section,
            output_section_name,
            offset: None,
        }
    }

    pub fn get_size(&self) -> usize {
        self.elf_section.data.len()
    }

    fn get_offset(&self) -> usize {
        self.offset.unwrap()
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = Some(offset);
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        let offset = self.get_offset();
        let size = self.get_size();
        let data = &self.elf_section.data;
        buf[offset..offset + size].copy_from_slice(data);
    }
}

#[derive(Clone)]
pub struct ElfSymbol {
    name: String,
    sym: ElfSymbolData,
}

impl std::fmt::Debug for ElfSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Symbol").field("name", &self.name).finish()
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub file: Option<ObjectId>,
}

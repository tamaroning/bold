use std::sync::Arc;

use crate::{context::Context, dummy};
use elf::{endian::AnyEndian, section::SectionHeader, symbol::Symbol as ElfSymbolData, ElfBytes};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct ObjectId {
    private: usize,
}

fn get_next_object_file_id() -> ObjectId {
    static mut OBJECT_FILE_ID: usize = 0;
    let id = unsafe { OBJECT_FILE_ID };
    unsafe { OBJECT_FILE_ID += 1 };
    ObjectId { private: id }
}

pub struct ObjectFile {
    id: ObjectId,
    file_name: String,
    // TODO: archive file
    data: Vec<u8>,

    // Elf sections and symbols
    elf_symtab: SectionHeader,
    first_global: usize,
    elf_sections: Vec<Arc<ElfSection>>,
    elf_symbols: Vec<ElfSymbol>,

    input_sections: Vec<Option<InputSectionId>>,
    symbols: Vec<Option<Symbol>>,
}

impl ObjectFile {
    pub fn read_from(file_name: String) -> ObjectFile {
        // TODO: We should use mmap here
        let data = std::fs::read(file_name.clone()).unwrap();
        ObjectFile {
            id: get_next_object_file_id(),
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

    pub fn get_id(&self) -> ObjectId {
        self.id
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

    pub fn get_input_sections(&self) -> &[Option<InputSectionId>] {
        &self.input_sections
    }

    pub fn get_symbols(&self) -> &[Option<Symbol>] {
        &self.symbols
    }

    pub fn parse(&mut self, ctx: &mut Context) {
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

        self.initialize_sections(ctx);
        self.initialize_symbols();
    }

    fn initialize_sections(&mut self, ctx: &mut Context) {
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
                    self.input_sections[i] = Some(input_section.get_id());
                    ctx.set_input_section(input_section);
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

    pub fn register_defined_symbols(&mut self) {
        let object_id = self.get_id();
        for (i, symbol) in self.symbols.iter_mut().enumerate() {
            let esym = &self.elf_symbols[i];
            if esym.sym.is_undefined() {
                continue;
            }
            let Some(symbol) = symbol else {
                continue;
            };

            symbol.file = Some(object_id);
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

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct InputSectionId {
    private: usize,
}

fn get_next_input_section_id() -> InputSectionId {
    static mut INPUT_SECTION_ID: usize = 0;
    let id = unsafe { INPUT_SECTION_ID };
    unsafe { INPUT_SECTION_ID += 1 };
    InputSectionId { private: id }
}

#[derive(Debug, Clone)]
pub struct InputSection {
    id: InputSectionId,
    pub elf_section: Arc<ElfSection>,
    /// Offset from the beginning of the output file
    offset: Option<u64>,
}

impl InputSection {
    fn new(elf_section: Arc<ElfSection>) -> InputSection {
        InputSection {
            id: get_next_input_section_id(),
            elf_section,
            offset: None,
        }
    }

    pub fn get_id(&self) -> InputSectionId {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.elf_section.name
    }

    pub fn get_size(&self) -> u64 {
        assert_eq!(
            self.elf_section.data.len() as u64,
            self.elf_section.header.sh_size
        );
        self.elf_section.header.sh_size
    }

    fn get_offset(&self) -> u64 {
        self.offset.unwrap()
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = Some(offset);
    }

    pub fn copy_to(&self, buf: &mut [u8]) {
        let offset = self.get_offset();
        let size = self.get_size();
        let data = &self.elf_section.data;
        buf[offset as usize..(offset + size) as usize].copy_from_slice(data);
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

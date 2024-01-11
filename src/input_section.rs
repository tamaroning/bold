use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::{context::Context, dummy};
use elf::{
    endian::AnyEndian,
    relocation::Rela,
    section::SectionHeader,
    symbol::{Elf64_Sym, Symbol as ElfSymbolData},
    ElfBytes,
};

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
    elf_symbols: Vec<Arc<ElfSymbol>>,

    input_sections: Vec<Option<InputSectionId>>,
    symbols: Vec<Option<Arc<RefCell<Symbol>>>>,
    is_dso: bool,
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
            is_dso: false,
        }
    }

    pub fn get_id(&self) -> ObjectId {
        self.id
    }

    pub fn get_file_name(&self) -> &str {
        &self.file_name
    }

    pub fn get_first_global(&self) -> usize {
        self.first_global
    }

    pub fn get_elf_sections(&self) -> &[Arc<ElfSection>] {
        &self.elf_sections
    }

    pub fn get_elf_symbols(&self) -> &[Arc<ElfSymbol>] {
        &self.elf_symbols
    }

    pub fn get_input_sections(&self) -> &[Option<InputSectionId>] {
        &self.input_sections
    }

    pub fn get_symbols(&self) -> &[Option<Arc<RefCell<Symbol>>>] {
        &self.symbols
    }

    pub fn parse(&mut self, ctx: &mut Context) {
        let file = ElfBytes::<AnyEndian>::minimal_parse(&self.data).expect("Open ELF file failed");
        self.is_dso = file.ehdr.e_type == elf::abi::ET_DYN;
        log::debug!("dso: {}", self.is_dso);

        let shstrtab_shdr = file.section_header_by_name(".shstrtab").unwrap().unwrap();
        let shstrtab = file.section_data_as_strtab(&shstrtab_shdr).unwrap();
        let section_headers = file.section_headers().unwrap();
        // Arrange elf_sections
        for shdr in section_headers {
            let name = shstrtab.get(shdr.sh_name as usize).unwrap();
            // TODO: remove clone()
            self.elf_sections.push(Arc::new(ElfSection {
                name: name.to_string(),
                header: shdr,
                data: file.section_data(&shdr).unwrap().0.to_vec(),
            }));
        }

        // Arrange elf_symbols
        let (symtab_sec, strtab_sec) = file.symbol_table().unwrap().unwrap();
        // TODO: Use .dsymtab instead of .symtab for dso
        let symtab_shdr = file.section_header_by_name(".symtab").unwrap().unwrap();
        for sym in symtab_sec {
            let name = strtab_sec.get(sym.st_name as usize).unwrap();
            self.elf_symbols.push(Arc::new(ElfSymbol {
                name: name.to_string(),
                sym,
            }));
        }

        self.elf_symtab = symtab_shdr;
        self.first_global = symtab_shdr.sh_info as usize;

        let mut relas = HashMap::new();
        for shdr in section_headers {
            let name = shstrtab.get(shdr.sh_name as usize).unwrap();
            if name.starts_with(".rela") {
                let target = name[5..].to_string();
                dbg!(&target);
                let data = file.section_data_as_relas(&shdr).unwrap();
                for rela in data {
                    relas
                        .entry(target.clone())
                        .or_insert(Vec::new())
                        .push(ElfRela { erela: rela });
                }
            }
        }

        self.initialize_sections(ctx, relas);
        self.initialize_symbols(ctx);
    }

    fn initialize_sections(&mut self, ctx: &mut Context, mut relas: HashMap<String, Vec<ElfRela>>) {
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
                    // Create a new section and attach relocations to it
                    let relas = relas.remove(&elf_section.name).unwrap_or(Vec::new());
                    let input_section = InputSection::new(Arc::clone(elf_section), relas);
                    self.input_sections[i] = Some(input_section.get_id());
                    ctx.set_input_section(input_section);
                }
            }
        }
    }

    fn initialize_symbols(&mut self, ctx: &mut Context) {
        self.symbols.resize(self.elf_symbols.len(), None);

        // Initialize local symbols
        for (i, elf_symbol) in self.elf_symbols.iter().enumerate() {
            if i >= self.first_global {
                break;
            }
            let name = elf_symbol.name.clone();
            if i == 0 {
                log::error!("Expected NULL Symbol at index 0, but found '{}'", name);
                continue;
            }
            self.symbols[i] = Some(Arc::new(RefCell::new(Symbol {
                name,
                file: None,
                esym: Arc::clone(elf_symbol),
                global: false,
            })));
        }

        // Initialize global symbols
        for (i, elf_symbol) in self.elf_symbols.iter().enumerate() {
            if i < self.first_global {
                continue;
            }
            let name_end = elf_symbol.name.find('@').unwrap_or(elf_symbol.name.len());
            let name = elf_symbol.name[..name_end].to_string();
            let symbol = Arc::new(RefCell::new(Symbol {
                name,
                file: None,
                esym: Arc::clone(elf_symbol),
                global: true,
            }));
            self.symbols[i] = Some(Arc::clone(&symbol));
            ctx.add_global_symbol(symbol);
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
    elf_relas: Vec<ElfRela>,
    /// Offset from the beginning of the output file
    offset: Option<u64>,
}

impl InputSection {
    fn new(elf_section: Arc<ElfSection>, elf_relas: Vec<ElfRela>) -> InputSection {
        InputSection {
            id: get_next_input_section_id(),
            elf_section,
            elf_relas,
            offset: None,
        }
    }

    pub fn get_id(&self) -> InputSectionId {
        self.id
    }

    pub fn get_relas(&self) -> &[ElfRela] {
        &self.elf_relas
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

    pub fn copy_buf(&self, buf: &mut [u8]) {
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

pub fn is_abs(sym: &ElfSymbolData) -> bool {
    sym.st_shndx == elf::abi::SHN_ABS as u16
}

pub fn is_common(sym: &ElfSymbolData) -> bool {
    sym.st_shndx == elf::abi::SHN_COMMON as u16
}

impl ElfSymbol {
    pub fn get_esym(&self) -> &ElfSymbolData {
        &self.sym
    }

    pub fn get(&self) -> Elf64_Sym {
        Elf64_Sym {
            st_name: self.sym.st_name,
            st_info: (self.sym.st_bind() << 4) | self.sym.st_symtype(),
            st_other: self.sym.st_vis(),
            st_shndx: self.sym.st_shndx,
            st_value: self.sym.st_value,
            st_size: self.sym.st_size,
        }
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }
}

impl std::fmt::Debug for ElfSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Symbol").field("name", &self.name).finish()
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    /// object file where the symbol is defined
    pub file: Option<ObjectId>,
    pub esym: Arc<ElfSymbol>,
    global: bool,
}

impl Symbol {
    pub fn should_write(&self) -> bool {
        // TODO: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/object_file.cc#L302
        true
    }

    pub fn is_global(&self) -> bool {
        self.global
    }
}

#[derive(Debug, Clone)]
pub struct ElfRela {
    pub erela: Rela,
}

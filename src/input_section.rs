use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::{context::Context, output_section::OutputSectionId};
use elf::{
    endian::AnyEndian,
    relocation::Rela,
    section::SectionHeader,
    symbol::{Elf64_Sym, Symbol as ElfSymbolData},
    ElfBytes,
};

/// Missing constants in elf-rs
const SHF_EXCLUDE: u64 = 0x80000000;

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

    first_global: usize,
    /// All sections corresponding to each section header
    elf_sections: Vec<Arc<ElfSection>>,
    /// All symbols corresponding to each symbol table entry
    elf_symbols: Vec<Arc<ElfSymbol>>,
    /// sections corresponding to each section header
    input_sections: Vec<Option<InputSectionId>>,
    /// symbols corresponding to each symbol table entry
    symbols: Vec<Option<Arc<RefCell<Symbol>>>>,
    is_dso: bool,
    in_archive: bool,
}

impl ObjectFile {
    fn new(file_name: String, data: Vec<u8>, in_archive: bool) -> ObjectFile {
        ObjectFile {
            id: get_next_object_file_id(),
            file_name,
            data,
            first_global: 0,
            elf_sections: Vec::new(),
            elf_symbols: Vec::new(),
            input_sections: Vec::new(),
            symbols: Vec::new(),
            is_dso: false,
            in_archive,
        }
    }

    pub fn read_from(file_name: &str) -> Vec<ObjectFile> {
        fn is_archive(file_name: &str) -> bool {
            file_name.ends_with(".a")
        }

        // TODO: We should use mmap here
        if is_archive(&file_name) {
            log::debug!("Opening archive file: {}", file_name);
            let mut objs = vec![];
            let mut archive = ar::Archive::new(std::fs::File::open(file_name).unwrap());
            while let Some(Ok(mut entry)) = archive.next_entry() {
                let mut buf = Vec::new();
                std::io::copy(&mut entry, &mut buf).unwrap();
                let member_file_name = std::str::from_utf8(entry.header().identifier())
                    .unwrap()
                    .to_string();
                log::debug!("\t{} ({} bytes)", member_file_name, buf.len());
                let member_file = ObjectFile::new(member_file_name, buf, true);
                objs.push(member_file);
            }
            objs
        } else {
            let data = std::fs::read(file_name).expect(&format!("Failed to read {}", file_name));
            log::debug!("Opened object file: {} ({} bytes)", file_name, data.len());
            vec![ObjectFile::new(file_name.to_string(), data, false)]
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

    pub fn is_dso(&self) -> bool {
        self.is_dso
    }

    pub fn is_in_archive(&self) -> bool {
        self.in_archive
    }

    pub fn parse(&mut self, ctx: &mut Context) {
        let file = ElfBytes::<AnyEndian>::minimal_parse(&self.data).expect("Open ELF file failed");
        self.is_dso = file.ehdr.e_type == elf::abi::ET_DYN;

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
        if let Some((symtab_sec, strtab_sec)) = file.symbol_table().unwrap() {
            // TODO: Use .dsymtab instead of .symtab for dso
            let symtab_shdr = file.section_header_by_name(".symtab").unwrap().unwrap();
            for sym in symtab_sec {
                // remove string after @
                let name = strtab_sec.get(sym.st_name as usize).unwrap();
                let name_end = name.find('@').unwrap_or(name.len());
                let name = name[..name_end].to_string();
                self.elf_symbols.push(Arc::new(ElfSymbol {
                    name: name.to_string(),
                    sym,
                }));
            }
            self.first_global = symtab_shdr.sh_info as usize;
        }

        let mut elf_rels = HashMap::new();
        for shdr in section_headers {
            let name = shstrtab.get(shdr.sh_name as usize).unwrap();
            if name.starts_with(".rela") {
                let target = name[5..].to_string();
                let data = file.section_data_as_relas(&shdr).unwrap();
                for rela in data {
                    elf_rels
                        .entry(target.clone())
                        .or_insert(Vec::new())
                        .push(rela);
                }
            }
        }

        self.initialize_sections(ctx);
        self.initialize_symbols(ctx);
        self.initialize_relocations(ctx, elf_rels);
    }

    fn initialize_sections(&mut self, ctx: &mut Context) {
        self.input_sections.resize(self.elf_sections.len(), None);
        for (i, elf_section) in self.elf_sections.iter().enumerate() {
            if (elf_section.header.sh_flags & SHF_EXCLUDE) != 0
                && (elf_section.header.sh_flags & elf::abi::SHF_ALLOC as u64) == 0
            {
                continue;
            }
            match elf_section.header.sh_type {
                elf::abi::SHT_NULL
                | elf::abi::SHT_REL
                | elf::abi::SHT_RELA
                | elf::abi::SHT_SYMTAB
                | elf::abi::SHT_STRTAB => {
                    // Nothing to do
                }
                elf::abi::SHT_NOTE => {
                    let name = &elf_section.name;
                    log::debug!(
                        "TODO: SHT_NOTE {} is not supported, ignored ({})",
                        name,
                        self.get_file_name()
                    );
                }
                elf::abi::SHT_SYMTAB_SHNDX => panic!("SHT_SYMTAB_SHNDX is not supported"),
                elf::abi::SHT_GROUP => {
                    let shdr = elf_section.header;
                    let esym = self.elf_symbols[shdr.sh_info as usize].clone();
                    let signature = esym.get_name();

                    let name = &elf_section.name;
                    log::debug!(
                        "TODO: SHT_GROUP {} is not supported, ignored ({})",
                        name,
                        self.get_file_name()
                    );
                    log::debug!("signature: \"{}\"", signature);
                }
                _ => {
                    if elf_section.name == ".note.GNU-stack" {
                        // https://github.com/tamaroning/mold/blob/c3a86f5b24343f020edfac1f683dea3648a30e61/elf/input-files.cc#L180
                        log::debug!("TODO: {} is not supported, ignored", elf_section.name);
                    }

                    if elf_section.name.starts_with(".gnu.warning.")
                    // FIXME: I am not sure we can ignore these sections
                    // https://github.com/tamaroning/mold/blob/c3a86f5b24343f020edfac1f683dea3648a30e61/elf/input-files.cc#L237
                        || elf_section.name == ".debug_gnu_pubnames"
                        || elf_section.name == ".debug_gnu_pubtypes"
                        || elf_section.name == ".debug_types"
                    {
                        continue;
                    }

                    // Create a new section
                    let input_section = InputSection::new(Arc::clone(elf_section));
                    self.input_sections[i] = Some(input_section.get_id());
                    ctx.set_input_section(input_section);
                }
            }
            // TODO: set is_comdat_member
            // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/object_file.cc#L179
        }
    }

    fn initialize_symbols(&mut self, ctx: &mut Context) {
        self.symbols.resize(self.elf_symbols.len(), None);

        // Initialize local symbols
        for (i, elf_symbol) in self.elf_symbols.iter().enumerate() {
            if i >= self.first_global {
                break;
            }
            if i == 0 {
                continue;
            }
            if elf_symbol.is_common() {
                panic!("common local symbol?")
            }
            self.symbols[i] = Some(Arc::new(RefCell::new(Symbol {
                name: elf_symbol.name.clone(),
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
            let symbol = Arc::new(RefCell::new(Symbol {
                name: elf_symbol.name.clone(),
                file: None,
                esym: Arc::clone(elf_symbol),
                global: true,
            }));
            self.symbols[i] = Some(Arc::clone(&symbol));
            ctx.add_global_symbol(symbol);
        }
    }

    fn initialize_relocations(
        &mut self,
        ctx: &mut Context,
        mut elf_rels: HashMap<String, Vec<Rela>>,
    ) {
        for isec in self.input_sections.iter_mut() {
            if let Some(isec) = isec {
                let isec = ctx.get_input_section_mut(*isec);
                let name = isec.get_name();
                if let Some(rels) = elf_rels.remove(name) {
                    let rels = rels
                        .into_iter()
                        .map(|rela| {
                            let symbol = self.symbols[rela.r_sym as usize].as_ref().unwrap();
                            ElfRela {
                                erela: rela,
                                symbol: Arc::clone(symbol),
                            }
                        })
                        .collect::<Vec<_>>();
                    isec.set_relas(rels);
                }
            }
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
    output_section: Option<OutputSectionId>,
}

impl InputSection {
    fn new(elf_section: Arc<ElfSection>) -> InputSection {
        InputSection {
            id: get_next_input_section_id(),
            elf_section,
            elf_relas: Vec::new(),
            offset: None,
            output_section: None,
        }
    }

    pub fn get_id(&self) -> InputSectionId {
        self.id
    }

    pub fn set_relas(&mut self, elf_relas: Vec<ElfRela>) {
        self.elf_relas = elf_relas;
    }

    pub fn get_relas(&self) -> &[ElfRela] {
        &self.elf_relas
    }

    pub fn get_name(&self) -> &String {
        &self.elf_section.name
    }

    pub fn get_size(&self) -> u64 {
        /* bss, tbss breaks this
        assert_eq!(
            self.elf_section.data.len() as u64,
            self.elf_section.header.sh_size
        );
        */
        self.elf_section.header.sh_size
    }

    pub fn get_offset(&self) -> Option<u64> {
        self.offset
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = Some(offset);
    }

    pub fn get_output_section(&self) -> OutputSectionId {
        self.output_section.unwrap()
    }

    pub fn set_output_section(&mut self, output_section: OutputSectionId) {
        self.output_section = Some(output_section);
    }

    pub fn copy_buf(&self, buf: &mut [u8]) {
        let offset = self.get_offset().unwrap();
        let size = self.get_size();
        let data = &self.elf_section.data;
        // bss and tbss has empty elf section data
        if !data.is_empty() {
            buf[offset as usize..(offset + size) as usize].copy_from_slice(data);
        }
    }
}

#[derive(Clone)]
pub struct ElfSymbol {
    name: String,
    sym: ElfSymbolData,
}

impl ElfSymbol {
    /// Return Elf_Sym definied in the object file
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

    pub fn is_abs(&self) -> bool {
        self.sym.st_shndx == elf::abi::SHN_ABS as u16
    }

    pub fn is_common(&self) -> bool {
        self.sym.st_shndx == elf::abi::SHN_COMMON as u16
    }

    pub fn is_weak(&self) -> bool {
        self.sym.st_bind() == elf::abi::STB_WEAK
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
    pub symbol: Arc<RefCell<Symbol>>,
}

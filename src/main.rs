use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use elf::{
    endian::AnyEndian, file::Elf64_Ehdr, section::SectionHeader, symbol::Symbol as ElfSymbolData,
    ElfBytes,
};
use log::info;

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
struct ObjectId {
    private: usize,
}

fn get_next_object_file_id() -> ObjectId {
    static mut OBJECT_FILE_ID: usize = 0;
    let id = unsafe { OBJECT_FILE_ID };
    unsafe { OBJECT_FILE_ID += 1 };
    ObjectId { private: id }
}

struct Context {
    file_pool: HashMap<ObjectId, Arc<RefCell<ObjectFile>>>,
}

impl Context {
    fn new(files: Vec<ObjectFile>) -> Context {
        Context {
            file_pool: files
                .into_iter()
                .map(|f| (get_next_object_file_id(), Arc::new(RefCell::new(f))))
                .collect(),
        }
    }

    fn get_file(&self, id: ObjectId) -> Option<Arc<RefCell<ObjectFile>>> {
        self.file_pool.get(&id).map(Arc::clone)
    }

    fn resovle_symbols(&mut self) {
        for (id, file) in self.file_pool.iter() {
            file.borrow_mut().register_defined_symbols(*id);
            file.borrow_mut().register_undefined_symbols();
        }
    }

    fn dump(&self) {
        self.dump_sections();
        self.dump_symbols();
    }

    fn dump_sections(&self) {
        for file in self.file_pool.values() {
            let file = file.borrow();
            log::debug!("Sections in '{}'", file.file_name);
            for (elf_section, input_section) in
                file.elf_sections.iter().zip(file.input_sections.iter())
            {
                if let Some(input_section) = input_section {
                    let output_section = &input_section.output_section;
                    let output_section = output_section.read().unwrap();
                    log::debug!(
                        "\t{:?} (InputSection -> {})",
                        elf_section.name,
                        output_section.name
                    );
                    continue;
                } else {
                    log::debug!("\t{:?}", elf_section.name);
                }
            }
        }
    }

    fn dump_symbols(&self) {
        for file in self.file_pool.values() {
            let file = file.borrow();
            log::debug!("Symbols in '{}'", file.file_name);
            for symbol in file.symbols.iter() {
                if let Some(symbol) = symbol {
                    let definiton_loc = if let Some(file_id) = symbol.file {
                        let file = self.get_file(file_id).unwrap();
                        let file = file.borrow();
                        file.get_file_name().to_owned()
                    } else {
                        "UNDEFINED".to_owned()
                    };
                    log::debug!("\t\"{}\" ('{}')", symbol.name, definiton_loc);
                }
            }
        }
    }
}

struct ObjectFile {
    file_name: String,
    // TODO: archive file
    data: Vec<u8>,

    // Elf sections and symbols
    elf_symtab: SectionHeader,
    first_global: usize,
    elf_sections: Vec<Arc<ElfSection>>,
    elf_symbols: Vec<ElfSymbol>,

    input_sections: Vec<Option<InputSection>>,
    symbols: Vec<Option<Symbol>>,
}

macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

impl ObjectFile {
    fn read_from(file_name: String) -> ObjectFile {
        // TODO: use mmap
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

    fn get_file_name(&self) -> &str {
        &self.file_name
    }

    fn parse(&mut self) {
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
                    self.input_sections[i] = Some(input_section);
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

    fn register_defined_symbols(&mut self, this_file_id: ObjectId) {
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

    fn register_undefined_symbols(&mut self) {
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

struct ElfSection {
    name: String,
    header: SectionHeader,
    data: Vec<u8>,
}

impl std::fmt::Debug for ElfSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Section").field("name", &self.name).finish()
    }
}

#[derive(Debug, Clone)]
struct InputSection {
    elf_section: Arc<ElfSection>,
    output_section: Arc<RwLock<OutputSection>>,
}

impl InputSection {
    fn new(elf_section: Arc<ElfSection>) -> InputSection {
        let output_section = OutputSection::from_section_name(&elf_section.name);
        InputSection {
            elf_section,
            output_section,
        }
    }
}

#[derive(Clone)]
struct ElfSymbol {
    name: String,
    sym: ElfSymbolData,
}

impl std::fmt::Debug for ElfSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Symbol").field("name", &self.name).finish()
    }
}

#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    file: Option<ObjectId>,
}

trait OutputChunk {
    fn get_size(&self) -> usize;
    fn get_offset(&self) -> usize;
    fn set_offset(&mut self, offset: usize);
    fn copy_to(&self, buf: &mut [u8]);
    fn relocate(&mut self, relocs: &[u8]);
}

struct OutputEhdr {
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

    fn copy_to(&self, buf: &mut [u8]) {}

    fn relocate(&mut self, relocs: &[u8]) {
        todo!()
    }
}

#[derive(Debug, Clone)]
struct OutputSection {
    name: String,
}

impl OutputSection {
    fn from_section_name(section_name: &String) -> Arc<RwLock<OutputSection>> {
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
                .map(|name| OutputSection {
                    name: name.to_string(),
                })
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
        panic!()
    }
}

impl OutputChunk for OutputSection {
    fn get_size(&self) -> usize {
        todo!()
    }

    fn get_offset(&self) -> usize {
        todo!()
    }

    fn set_offset(&mut self, offset: usize) {
        todo!()
    }

    fn copy_to(&self, buf: &mut [u8]) {
        todo!()
    }

    fn relocate(&mut self, relocs: &[u8]) {
        todo!()
    }
}

fn main() {
    env_logger::init();
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let mut files = args[1..]
        .iter()
        .map(|arg| ObjectFile::read_from(arg.clone()))
        .collect::<Vec<_>>();

    for file in files.iter_mut() {
        info!("Parsing {}", file.file_name);
        file.parse();
    }

    // Set priorities to files
    // What is this?

    let mut ctx = Context::new(files);

    // Register (un)defined symbols
    info!("Resolving symbols");
    ctx.resovle_symbols();

    ctx.dump();

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    let mut output_chunks: Vec<&dyn OutputChunk> = vec![];

    // Bin input sections into output sections
    // How can we handle duplicate sections?
    for file in ctx.file_pool.values() {
        let mut file = file.borrow_mut();
        for input_section in file.input_sections.iter_mut() {
            if let Some(input_section) = input_section {}
        }
    }

    // Assign offsets to input sections

    // Create an output file

    // Copy input sections to the output file
}

use std::rc::Rc;

use elf::{
    endian::AnyEndian, file::Elf64_Ehdr, section::SectionHeader, symbol::Symbol as ElfSymbolData,
    ElfBytes,
};
use log::info;

struct ObjectFile {
    filename: String,
    data: Vec<u8>,

    // Elf sections and symbols
    elf_symtab: SectionHeader,
    elf_sections: Vec<Rc<ElfSection>>,
    elf_symbols: Vec<ElfSymbol>,

    input_sections: Vec<Option<InputSection>>,
    symbols: Vec<Option<ElfSymbol>>,
}

macro_rules! dummy {
    ($name: ty) => {
        unsafe { std::mem::transmute([0 as u8; std::mem::size_of::<$name>()]) }
    };
}

impl ObjectFile {
    fn read_from(filename: String) -> ObjectFile {
        // TODO: use mmap
        let data = std::fs::read(filename.clone()).unwrap();
        ObjectFile {
            filename,
            data,
            elf_symtab: dummy!(SectionHeader),
            elf_sections: Vec::new(),
            elf_symbols: Vec::new(),
            input_sections: Vec::new(),
            symbols: Vec::new(),
        }
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
            self.elf_sections.push(Rc::new(ElfSection {
                name: name.to_string(),
                header: shdr.clone(),
                data: file.section_data(&shdr).unwrap().0.to_vec(),
            }));
        }

        // Arrange elf_symbols
        let (symtab_sec, strtab_sec) = file.symbol_table().unwrap().unwrap();
        // TODO: Use .dsymtab instead of .symtab for dso
        let symtab_shdr = file.section_header_by_name(".symtab").unwrap().unwrap();
        for (i, sym) in symtab_sec.into_iter().enumerate() {
            let name = strtab_sec.get(sym.st_name as usize).unwrap();
            self.elf_symbols.push(ElfSymbol {
                name: name.to_string(),
                sym,
            });
        }

        self.elf_symtab = symtab_shdr;

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
                    self.input_sections[i] = Some(InputSection {
                        elf_section: Rc::clone(elf_section),
                    });
                }
            }
        }
    }

    fn initialize_symbols(&mut self) {
        self.symbols.resize(self.elf_symbols.len(), None);
        for (i, elf_symbol) in self.elf_symbols.iter().enumerate() {
            // Skip until reaching the first global
            if i < self.elf_symtab.sh_info as usize {
                continue;
            }
            self.symbols[i] = Some(elf_symbol.clone());
        }
    }

    fn register_defined_symbols(&mut self) {}

    fn register_undefined_symbols(&mut self) {}
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
    elf_section: Rc<ElfSection>,
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

    files.iter_mut().for_each(|file| {
        info!("Parsing {}", file.filename);
        file.parse();
        dbg!(&file.elf_sections);
        dbg!(&file.elf_symbols);
        dbg!(&file.input_sections);
        dbg!(&file.symbols);
    });

    // Set priorities to files
    // What is this?

    // Register defined symbols

    // Register undefined symbols

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    // Bin input sections into output sections
    // How can we handle duplicate sections?

    // Assign offsets to input sections

    // Create an output file

    // Copy input sections to the output file
}

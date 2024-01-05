use std::{collections::HashMap, rc::Rc};

use elf::{endian::AnyEndian, section::SectionHeader, symbol::Symbol as ElfSymbol, ElfBytes};
use log::info;

struct ObjectFile {
    filename: String,
    data: Vec<u8>,
    elf_sections: Vec<Rc<Section>>,
    input_sections: Vec<Option<Rc<Section>>>,
    symbols: HashMap<String, Symbol>,
}

impl ObjectFile {
    fn read_from(filename: String) -> ObjectFile {
        // TODO: use mmap
        let data = std::fs::read(filename.clone()).unwrap();
        ObjectFile {
            filename,
            data,
            elf_sections: Vec::new(),
            input_sections: Vec::new(),
            symbols: HashMap::new(),
        }
    }

    fn parse(&mut self) {
        let file = ElfBytes::<AnyEndian>::minimal_parse(&self.data).expect("Open ELF file failed");

        let shstrtab_shdr = file.section_header_by_name(".shstrtab").unwrap().unwrap();
        let shstrtab = file.section_data_as_strtab(&shstrtab_shdr).unwrap();
        let section_headers = file.section_headers().unwrap();
        for shdr in section_headers.into_iter() {
            let name = shstrtab.get(shdr.sh_name as usize).unwrap();
            // TODO: remove clone()
            self.elf_sections.push(Rc::new(Section {
                name: name.to_string(),
                header: shdr.clone(),
                data: file.section_data(&shdr).unwrap().0.to_vec(),
            }));
        }

        let (symtab_sec, strtab_sec) = file.symbol_table().unwrap().unwrap();
        let symtab_shdr = file.section_header_by_name(".symtab").unwrap().unwrap();
        for (i, sym) in symtab_sec.into_iter().enumerate() {
            // Skip until reaching the first global
            if i < symtab_shdr.sh_info as usize {
                continue;
            }
            let name = strtab_sec.get(sym.st_name as usize).unwrap();
            self.symbols.insert(name.to_string(), Symbol { data: sym });
        }

        self.initialize_sections();
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
                    self.input_sections[i] = Some(Rc::clone(elf_section));
                }
            }
        }
    }
}

struct Section {
    name: String,
    header: SectionHeader,
    data: Vec<u8>,
}

impl std::fmt::Debug for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Section")
            .field("name", &self.name)
            .field("header", &self.header)
            .finish()
    }
}

#[derive(Debug)]
struct Symbol {
    data: ElfSymbol,
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

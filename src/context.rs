use std::{borrow::Borrow, cell::RefCell, collections::HashMap, ops::Deref, sync::Arc};

use crate::{
    input_section::{InputSection, InputSectionId, ObjectFile, ObjectId, Symbol},
    output_section::{get_output_section_name, OutputSection, OutputSectionId},
};

// https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/output_chunks.cc#L386
pub const COMMON_SECTION_NAMES: [&str; 10] = [
    ".text",
    ".data",
    ".data.rel.ro",
    ".rodata",
    ".bss",
    ".bss.rel.ro",
    //".ctors",
    //".dtors",
    ".init_array",
    ".fini_array",
    ".tbss",
    ".tdata",
];

pub struct Context {
    file_pool: HashMap<ObjectId, ObjectFile>,
    input_sections: HashMap<InputSectionId, InputSection>,
    output_sections: HashMap<OutputSectionId, OutputSection>,
    global_symbols: HashMap<String, Arc<RefCell<Symbol>>>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            file_pool: HashMap::new(),
            output_sections: HashMap::new(),
            input_sections: HashMap::new(),
            global_symbols: HashMap::new(),
        }
    }

    pub fn set_object_file(&mut self, file: ObjectFile) {
        self.file_pool.insert(file.get_id(), file);
    }

    pub fn set_input_section(&mut self, section: InputSection) {
        self.input_sections.insert(section.get_id(), section);
    }

    pub fn files(&self) -> impl Iterator<Item = &ObjectFile> {
        self.file_pool.values()
    }

    pub fn files_mut(&mut self) -> impl Iterator<Item = &mut ObjectFile> {
        self.file_pool.values_mut()
    }

    pub fn get_file(&self, id: ObjectId) -> &ObjectFile {
        self.file_pool.get(&id).unwrap()
    }

    pub fn get_file_mut(&mut self, id: ObjectId) -> &mut ObjectFile {
        self.file_pool.get_mut(&id).unwrap()
    }

    pub fn add_global_symbol(&mut self, symbol: Arc<RefCell<Symbol>>) {
        let sym = symbol.deref().borrow();
        assert!(sym.is_global());
        if sym.esym.get_esym().is_undefined() {
            return;
        }

        let name = sym.name.clone();
        if let Some(dup) = self.global_symbols.get(&name) {
            let dup = dup.deref().borrow();
            if dup.esym.is_weak() {
                log::debug!("Override weak symbol: {}", name);
            } else {
                log::error!("Duplicate non-weak symbol: {}", name);
                //panic!();
            }
        } else {
            log::debug!("Add global symbol: {}", name);
        }
        std::mem::drop(sym);
        self.global_symbols.insert(name, symbol);
    }

    pub fn get_global_symbol(&self, name: &str) -> Option<&Arc<RefCell<Symbol>>> {
        self.global_symbols.get(name)
    }

    pub fn get_global_symbols(&self) -> impl Iterator<Item = &Arc<RefCell<Symbol>>> {
        self.global_symbols.values()
    }

    pub fn get_input_section(&self, id: InputSectionId) -> &InputSection {
        self.input_sections.get(&id).unwrap()
    }

    pub fn get_input_section_mut(&mut self, id: InputSectionId) -> &mut InputSection {
        self.input_sections.get_mut(&id).unwrap()
    }

    pub fn get_output_section(&self, id: OutputSectionId) -> &OutputSection {
        self.output_sections.get(&id).unwrap()
    }

    pub fn get_output_section_mut(&mut self, id: OutputSectionId) -> &mut OutputSection {
        self.output_sections.get_mut(&id).unwrap()
    }

    pub fn output_sections(&self) -> impl Iterator<Item = &OutputSection> {
        self.output_sections.values()
    }

    pub fn output_sections_mut(&mut self) -> impl Iterator<Item = &mut OutputSection> {
        self.output_sections.values_mut()
    }

    pub fn get_or_create_output_section_mut(
        &mut self,
        name: &str,
        sh_type: u32,
        sh_flags: u64,
    ) -> &mut OutputSection {
        let mut find = None;
        for section in &mut self.output_sections_mut() {
            if &section.get_name() == name
                && section.get_sh_type() == sh_type
                && section.get_sh_flags() == sh_flags
            {
                find = Some(section.get_id());
                break;
            }
        }

        let id = find.unwrap_or_else(|| {
            log::debug!("Create new output section: {}", name);
            let section = OutputSection::new(name.to_string(), sh_type, sh_flags);
            let id = section.get_id();
            self.output_sections.insert(id, section);
            id
        });
        self.output_sections.get_mut(&id).unwrap()
    }

    pub fn dump(&self) {
        //self.dump_sections();
        //self.dump_symbols();
    }

    fn dump_sections(&self) {
        for file in self.files() {
            log::debug!("Sections in '{}'", file.get_file_name());
            for (elf_section, input_section) in file
                .get_elf_sections()
                .iter()
                .zip(file.get_input_sections().iter())
            {
                log::debug!("\t{:?}", elf_section.name);
                if let Some(input_section) = input_section {
                    let input_section = self.get_input_section(*input_section);
                    let output_section = get_output_section_name(input_section.get_name());
                    log::debug!("\t\tOutputSection: {:?}", output_section);
                    let num_relas = input_section.get_relas().len();
                    log::debug!("\t\tNumber of Relas: {}", num_relas);
                    continue;
                }
            }
        }
    }

    fn dump_symbols(&self) {
        for file in self.files() {
            log::debug!("Symbols in '{}'", file.get_file_name());
            for symbol in file.get_symbols().iter() {
                if let Some(symbol) = symbol {
                    let symbol = symbol.deref().borrow();
                    let definiton_loc = if let Some(file_id) = symbol.file {
                        let file = self.get_file(file_id);
                        file.get_file_name().to_owned()
                    } else {
                        "undefined".to_owned()
                    };
                    log::debug!("\t\"{}\" ({})", symbol.name, definiton_loc);
                }
            }
        }
        let global_symbols = self
            .global_symbols
            .iter()
            .map(|(_, symbol)| {
                let symbol = symbol.deref().borrow();
                let definiton_loc = if let Some(file_id) = symbol.file {
                    let file = self.get_file(file_id);
                    file.get_file_name().to_owned()
                } else {
                    "undefined".to_owned()
                };
                format!("\t\"{}\" ({})", symbol.name, definiton_loc)
            })
            .collect::<Vec<String>>();
        log::debug!("Global symbols:");
        for symbol in global_symbols {
            log::debug!("{}", symbol);
        }
    }
}

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::input_section::ObjectFile;

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

pub struct Context {
    file_pool: HashMap<ObjectId, ObjectFile>,
}

impl Context {
    pub fn new(files: Vec<ObjectFile>) -> Context {
        Context {
            file_pool: files
                .into_iter()
                .map(|f| (get_next_object_file_id(), f))
                .collect(),
        }
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

    pub fn resovle_symbols(&mut self) {
        for (id, file) in self.file_pool.iter_mut() {
            file.register_defined_symbols(*id);
            file.register_undefined_symbols();
        }
    }

    pub fn dump(&self) {
        self.dump_sections();
        self.dump_symbols();
    }

    fn dump_sections(&self) {
        for file in self.files() {
            log::debug!("Sections in '{}'", file.get_file_name());
            for (elf_section, input_section) in file
                .get_elf_sections()
                .iter()
                .zip(file.get_input_sections().iter())
            {
                if let Some(input_section) = input_section {
                    let input_section = input_section.read().unwrap();
                    let output_section = &input_section.output_section_name;
                    log::debug!(
                        "\t{:?} (InputSection -> {})",
                        elf_section.name,
                        output_section
                    );
                    continue;
                } else {
                    log::debug!("\t{:?}", elf_section.name);
                }
            }
        }
    }

    fn dump_symbols(&self) {
        for file in self.files() {
            log::debug!("Symbols in '{}'", file.get_file_name());
            for symbol in file.get_symbols().iter() {
                if let Some(symbol) = symbol {
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
    }
}

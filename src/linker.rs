use std::{cell::RefCell, collections::HashSet, ops::Deref, sync::Arc};

use elf::{
    abi::{PF_R, PF_W, PF_X, PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_TLS, SHF_WRITE, SHT_NOBITS},
    section::Elf64_Shdr,
    segment::Elf64_Phdr,
    symbol::Elf64_Sym,
};

use crate::{
    config::{Config, PAGE_SIZE},
    context::Context,
    dummy,
    input_section::{InputSectionId, Symbol},
    output_section::{get_output_section_name, ChunkInfo, OutputChunk, OutputSectionId},
    relocation::{relocation_size, relocation_value, RelValue},
    utils::align_to,
};

pub struct Linker<'ctx> {
    ctx: Context,
    // Move this to the main function
    pub chunks: Vec<OutputChunk>,
    pub config: &'ctx Config,
}

impl Linker<'_> {
    pub fn new<'ctx>(ctx: Context, config: &'ctx Config) -> Linker<'ctx> {
        Linker {
            ctx,
            chunks: vec![],
            config,
        }
    }

    pub fn get_ctx(&self) -> &Context {
        &self.ctx
    }

    /// Resolve all symbols
    pub fn resolve_symbols(&mut self) {
        // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/object_file.cc#L536

        // Set file to symbols defined in the object file
        let mut num_defined = 0;
        for file in self.ctx.files() {
            let object_id = file.get_id();
            for (i, symbol) in file.get_symbols().iter().enumerate() {
                let esym = &file.get_elf_symbols()[i];
                if esym.get_esym().is_undefined() {
                    continue;
                }
                let Some(symbol) = symbol else {
                    continue;
                };
                let symbol = symbol.deref();
                symbol.borrow_mut().file = Some(object_id);
                // TODO: visibility?
                num_defined += 1;
            }
        }

        // Symbols defined in other object files
        let mut num_resolved = 0;
        let mut unresolved = HashSet::new();
        for file in self.ctx.files() {
            for (i, symbol) in file.get_symbols().iter().enumerate() {
                if let Some(symbol) = symbol {
                    if i < file.get_first_global() {
                        continue;
                    }
                    let esym = &file.get_elf_symbols()[i];
                    if !esym.get_esym().is_undefined() {
                        continue;
                    }
                    let name = esym.get_name();
                    let Some(global_symbol) = self.ctx.get_global_symbol(name).map(Arc::clone)
                    else {
                        unresolved.insert(name.to_owned());
                        continue;
                    };
                    let defined_file = global_symbol.deref().borrow().file;
                    let defined_esym = Arc::clone(&global_symbol.deref().borrow().esym);
                    assert!(defined_file.is_some());
                    let mut symbol = symbol.deref().borrow_mut();
                    symbol.file = defined_file;
                    symbol.esym = defined_esym;
                    num_resolved += 1;
                }
            }
        }

        log::info!(
            "Summary: Defined: {} symbols, Resolved: {} references, Unresolved: {} symbols",
            num_defined,
            num_resolved,
            unresolved.len()
        );

        if !unresolved.is_empty() {
            log::warn!("Unresolved symbols:");
            for symbol in unresolved {
                log::warn!("\t{}", symbol);
            }
        }
    }

    pub fn bin_input_sections(&mut self) -> Vec<OutputSectionId> {
        let mut input_sections = vec![];
        for file in self.ctx.files_mut() {
            for input_section in file.get_input_sections().iter() {
                if let Some(input_section) = input_section {
                    input_sections.push(*input_section);
                }
            }
        }

        let mut chunks = vec![];
        for input_section_id in input_sections {
            let input_section = self.ctx.get_input_section(input_section_id);
            let output_section_name = get_output_section_name(input_section.get_name());
            let sh_type = input_section.elf_section.header.sh_type;
            let sh_flags = input_section.elf_section.header.sh_flags;
            let output_section =
                self.ctx
                    .get_or_create_output_section_mut(&output_section_name, sh_type, sh_flags);
            let osec_id = output_section.get_id();

            if output_section.get_input_sections_mut().is_empty() {
                let section = &output_section;
                chunks.push(section.get_id());
            }
            output_section
                .get_input_sections_mut()
                .push(input_section_id);

            let input_section = self.ctx.get_input_section_mut(input_section_id);
            input_section.set_output_section(osec_id);
        }
        chunks
    }

    pub fn assign_isec_offsets(&mut self) {
        let _ = self.assign_osec_offsets();
    }

    pub fn update_shdr(&mut self) {
        // Set sh_name to all shdrs
        fn calc_sh_name_from_shstrtab(shstrtab_content: &[u8], section_name: &str) -> usize {
            let mut section_name = unsafe { section_name.to_string().as_bytes_mut() }.to_vec();
            section_name.push(0);
            let mut sh_name = 0;
            let mut i = 0;
            while i < shstrtab_content.len() {
                if shstrtab_content[i..].starts_with(&section_name) {
                    sh_name = i;
                    break;
                }
                i += 1;
            }
            sh_name
        }
        let shstrtab_content = self.get_shstrtab_content();
        for chunk in self.chunks.iter_mut() {
            if !chunk.is_header() {
                let name = chunk.get_section_name(&self.ctx);
                let shdr = &mut chunk.get_common_mut().shdr;
                shdr.sh_name = calc_sh_name_from_shstrtab(&shstrtab_content, &name) as u32;
            }
        }

        // Call update_shdr for all chunks
        let num_shdrs = self.get_shdrs().len();
        let num_phdrs = self.create_phdr().len();
        let shstrtab_size = shstrtab_content.len() as u64;
        let (symtab_content, strtab_content) = self.get_symtab();
        let strtab_shndx = self
            .chunks
            .iter()
            .find_map(|chunk| {
                if let OutputChunk::Strtab(chunk) = chunk {
                    Some(chunk.common.shndx.unwrap() as u32)
                } else {
                    None
                }
            })
            .unwrap();

        for chunk in self.chunks.iter_mut() {
            match chunk {
                OutputChunk::Ehdr(_) => (/* Do nothing */),
                OutputChunk::Shdr(shdr) => {
                    shdr.update_shdr(num_shdrs);
                }
                OutputChunk::Phdr(phdr) => {
                    phdr.update_shdr(num_phdrs);
                }
                OutputChunk::Section(_) => (/* Do nothing */),
                OutputChunk::Symtab(symtab) => {
                    symtab.update_shdr(symtab_content.len() as u64, strtab_shndx)
                }
                OutputChunk::Strtab(strtab) => strtab.update_shdr(strtab_content.len() as u64),
                OutputChunk::Shstrtab(shstrtab) => shstrtab.update_shdr(shstrtab_size),
            }
        }
    }

    pub fn set_section_indices(&mut self) {
        // shndx = 0 is reserved for SHN_UNDEF
        let mut shndx = 1;
        for chunk in self.chunks.iter_mut() {
            if !chunk.is_header() {
                let common = chunk.get_common_mut();
                common.shndx = Some(shndx);
                shndx += 1;
            }
        }
    }

    pub fn assign_osec_offsets(&mut self) -> u64 {
        let mut file_ofs = 0;
        let mut vaddr = self.config.image_base;

        for chunk in self.chunks.iter_mut() {
            if chunk.get_common().should_be_loaded() {
                vaddr = align_to(vaddr, PAGE_SIZE);
            }

            if vaddr % PAGE_SIZE > file_ofs % PAGE_SIZE {
                file_ofs += vaddr % PAGE_SIZE - file_ofs % PAGE_SIZE;
            } else if vaddr % PAGE_SIZE < file_ofs % PAGE_SIZE {
                file_ofs = align_to(file_ofs, PAGE_SIZE) + vaddr % PAGE_SIZE;
            }

            // Align to sh_addralign
            let sh_addralign = chunk.get_common().shdr.sh_addralign;
            file_ofs = align_to(file_ofs, sh_addralign);
            vaddr = align_to(vaddr, sh_addralign);

            chunk.set_offset(&mut self.ctx, file_ofs);

            // Make sure to get sh_size after `chunk.set_offset` because we set a value to sh_size in it
            if chunk.get_common().shdr.sh_flags & SHF_ALLOC as u64 != 0 {
                chunk.get_common_mut().shdr.sh_addr = vaddr;
            }

            let is_bss = chunk.get_common().shdr.sh_type == SHT_NOBITS;
            if !is_bss {
                file_ofs += chunk.get_common_mut().shdr.sh_size;
            }
            let is_tbss = chunk.get_common().shdr.sh_flags & SHF_TLS as u64 != 0;
            if !is_tbss {
                vaddr += chunk.get_common_mut().shdr.sh_size;
            }
        }
        file_ofs
    }

    pub fn copy_buf(&mut self, buf: &mut [u8]) {
        // copy all shdrs to buf
        let e_shoff = self
            .chunks
            .iter()
            .find_map(|chunk| {
                if let OutputChunk::Shdr(chunk) = chunk {
                    Some(chunk.common.shdr.sh_offset)
                } else {
                    None
                }
            })
            .unwrap();

        let e_shnum = self.get_shdrs().len() as u16;
        let e_shstrndx = self
            .chunks
            .iter()
            .find_map(|chunk| {
                if let OutputChunk::Shstrtab(chunk) = chunk {
                    Some(chunk.common.shndx.unwrap() as u16)
                } else {
                    None
                }
            })
            .unwrap();
        let e_phoff = self
            .chunks
            .iter()
            .find_map(|chunk| {
                if let OutputChunk::Phdr(chunk) = chunk {
                    Some(chunk.common.shdr.sh_offset)
                } else {
                    None
                }
            })
            .unwrap();
        let e_entry = self.get_global_symbol_addr("_start").unwrap_or(0);
        let shstrtab_content = self.get_shstrtab_content();
        let (symtab_content, strtab_content) = self.get_symtab();
        let shdrs = self.get_shdrs();
        let phdrs = self.create_phdr();
        // copy all other sections and headers
        for chunk in self.chunks.iter_mut() {
            match chunk {
                // FIXME: dummy
                OutputChunk::Ehdr(chunk) => chunk.copy_buf(
                    buf,
                    e_entry,
                    e_phoff,
                    e_shoff,
                    phdrs.len() as u16,
                    e_shnum,
                    e_shstrndx,
                ),
                OutputChunk::Shdr(chunk) => {
                    chunk.copy_buf(buf, e_shoff as usize, &shdrs);
                }
                OutputChunk::Phdr(chunk) => {
                    chunk.copy_buf(buf, &phdrs);
                }
                OutputChunk::Section(chunk) => {
                    // TODO: apply relocation
                    // mold: apply_reloc_alloc
                    let chunk = self.ctx.get_output_section(chunk.get_id());
                    chunk.copy_buf(&self.ctx, buf);
                }
                OutputChunk::Strtab(chunk) => {
                    chunk.copy_buf(buf, &strtab_content);
                }
                OutputChunk::Symtab(chunk) => {
                    chunk.copy_buf(buf, &symtab_content);
                }
                OutputChunk::Shstrtab(chunk) => {
                    chunk.copy_buf(buf, &shstrtab_content);
                }
            }
        }
    }

    pub fn relocation(&self, buf: &mut [u8]) {
        let relocation_data = self.get_relocation_data();
        for relval in relocation_data {
            let RelValue {
                file_ofs,
                value,
                size,
            } = relval;
            log::debug!("Relocation: {:#x} -> {:#x}", file_ofs, value);
            let value = value.to_le_bytes();
            buf[file_ofs..file_ofs + size].copy_from_slice(&value[0..size]);
        }
    }

    fn get_shdrs(&self) -> Vec<Elf64_Shdr> {
        let mut shdrs = vec![dummy!(Elf64_Shdr)];
        for chunk in &self.chunks {
            if !chunk.is_header() {
                shdrs.push(chunk.get_common().get_elf64_shdr());
            }
        }
        shdrs
    }

    fn get_shstrtab_content(&self) -> Vec<u8> {
        let mut content = vec![0];
        for chunk in &self.chunks {
            if !chunk.is_header() {
                let name = chunk.get_section_name(&self.ctx);
                content.extend_from_slice(name.as_bytes());
                content.push(0);
            }
        }
        content
    }

    fn get_symbols(&self) -> Vec<&Arc<RefCell<Symbol>>> {
        let mut symbols = vec![];
        for file in self.ctx.files() {
            for sym in file.get_symbols() {
                if let Some(symbol_ref) = sym {
                    let symbol = symbol_ref.borrow();
                    if symbol.file == Some(file.get_id()) {
                        if symbol.should_write() && symbol.file == Some(file.get_id()) {
                            symbols.push(symbol_ref);
                        }
                    }
                }
            }
        }
        symbols
    }

    fn get_symtab(&self) -> (Vec<Elf64_Sym>, Vec<u8>) {
        let mut symtab_content = vec![dummy!(Elf64_Sym)];
        let mut strtab_content = vec![0];
        let symbols = self.get_symbols();
        for symbol_ref in symbols {
            let sym = symbol_ref.borrow_mut();
            let mut esym = sym.esym.get();
            esym.st_name = strtab_content.len() as u32;
            if sym.esym.is_abs() {
                // Keep esym.st_value
                // Keep esym.st_shndx
            } else if sym.esym.is_common() {
                panic!("common: {}", sym.name);
            } else {
                esym.st_value = self.get_symbol_addr(&sym).unwrap_or(0);
                let file = self.ctx.get_file(sym.file.unwrap());
                let shndx = sym.esym.get_esym().st_shndx as usize;
                // log::debug!("Symbol: {}", sym.name);
                let isec = file.get_input_sections()[shndx].unwrap();
                let isec = self.ctx.get_input_section(isec);
                let osec_id = isec.get_output_section();
                let common = self.get_common_from_osec(osec_id);
                esym.st_shndx = common.map(|chunk| chunk.shndx.unwrap() as u16).unwrap();
            }

            /* TODO: remove
            log::debug!(
                "Symbol: {} (st_value: {:#x}, st_shndx: {})",
                sym.name,
                esym.st_value,
                esym.st_shndx
            );
            */
            symtab_content.push(esym);
            strtab_content.extend_from_slice(sym.name.as_bytes());
            strtab_content.push(0);
        }
        (symtab_content, strtab_content)
    }

    fn create_phdr(&self) -> Vec<Elf64_Phdr> {
        fn to_phdr_flags(shdr: &Elf64_Shdr) -> u32 {
            let mut ret = PF_R;
            if shdr.sh_flags & SHF_WRITE as u64 != 0 {
                ret |= PF_W;
            }
            if shdr.sh_flags & SHF_EXECINSTR as u64 != 0 {
                ret |= PF_X;
            }
            ret
        }

        fn new_phdr(
            p_type: u32,
            p_flags: u32,
            p_align: u64,
            chunk_shdr: &Elf64_Shdr,
        ) -> Elf64_Phdr {
            Elf64_Phdr {
                p_type,
                p_flags,
                p_offset: chunk_shdr.sh_offset,
                p_vaddr: chunk_shdr.sh_addr,
                p_paddr: chunk_shdr.sh_addr,
                p_filesz: if chunk_shdr.sh_type == SHT_NOBITS {
                    0
                } else {
                    chunk_shdr.sh_size
                },
                p_memsz: chunk_shdr.sh_size,
                p_align,
            }
        }

        let mut phdrs = vec![];
        // Create PT_LOAD
        for chunk in &self.chunks {
            if chunk.get_common().should_be_loaded() {
                let shdr = &chunk.get_common().shdr;
                let phdr = new_phdr(PT_LOAD, to_phdr_flags(shdr), PAGE_SIZE, shdr);
                phdrs.push(phdr);
            }
        }
        phdrs
    }

    fn get_common_from_osec(&self, id: OutputSectionId) -> Option<&ChunkInfo> {
        self.chunks
            .iter()
            .find(|chunk| {
                if let OutputChunk::Section(chunk) = chunk {
                    chunk.get_id() == id
                } else {
                    false
                }
            })
            .map(|chunk| chunk.get_common())
    }

    fn get_isec_addr(&self, id: InputSectionId) -> u64 {
        let isec = self.ctx.get_input_section(id);
        let isec_file_ofs = isec.get_offset().unwrap_or(0);
        let osec_id = isec.get_output_section();
        let osec_common = self.get_common_from_osec(osec_id).unwrap();
        let osec_addr = osec_common.shdr.sh_addr;
        let osec_file_ofs = osec_common.shdr.sh_offset;
        osec_addr + (isec_file_ofs - osec_file_ofs)
    }

    fn get_symbol_addr(&self, symbol: &Symbol) -> Option<u64> {
        let file = self.ctx.get_file(symbol.file.unwrap());
        let shndx = symbol.esym.get_esym().st_shndx as usize;
        file.get_input_sections()[shndx].map(|isec_id| {
            let isec_addr = self.get_isec_addr(isec_id);
            isec_addr + symbol.esym.get_esym().st_value
        })
    }

    fn get_global_symbol_addr(&self, name: &str) -> Option<u64> {
        self.ctx.get_global_symbol(name).map(|symbol| {
            let symbol = symbol.deref().borrow();
            self.get_symbol_addr(&symbol).unwrap_or(0)
        })
    }

    /// Returns [(file_ofs, u64)]
    fn get_relocation_data(&self) -> Vec<RelValue> {
        let mut ret = Vec::new();
        for file in self.ctx.files() {
            for isec_id in file.get_input_sections() {
                if let Some(isec_id) = isec_id {
                    let isec_addr = self.get_isec_addr(*isec_id);
                    let isec = self.ctx.get_input_section(*isec_id);
                    for rel in isec.get_relas() {
                        let symbol = rel.symbol.deref().borrow();
                        let symbol_addr = self.get_symbol_addr(&symbol).unwrap();
                        if let Some(value) = relocation_value(symbol_addr, isec_addr, &rel.erela) {
                            let isec_file_ofs = isec.get_offset().unwrap();
                            let file_ofs = (isec_file_ofs + rel.erela.r_offset) as usize;
                            ret.push(RelValue {
                                file_ofs,
                                value,
                                size: relocation_size(&rel.erela),
                            });
                        }
                    }
                }
            }
        }
        ret
    }
}

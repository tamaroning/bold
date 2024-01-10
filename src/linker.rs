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
    output_section::{get_output_section_name, OutputChunk, OutputSectionId},
    utils::{align_to, padding},
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

    pub fn resolve_symbols(&mut self) {
        // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/object_file.cc#L536
        for file in self.ctx.files_mut() {
            file.register_defined_symbols();
            file.register_undefined_symbols();
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

            if output_section.sections.is_empty() {
                let section = &output_section;
                chunks.push(section.get_id());
            }
            output_section.sections.push(input_section_id);
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
        let (symtab_content, strtab_content) = self.get_symtab_and_strtab();
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
        let mut shndx = 0;
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

            let sh_addralign = chunk.get_common().shdr.sh_addralign;
            file_ofs = align_to(file_ofs, sh_addralign);
            vaddr = align_to(vaddr, sh_addralign);

            let sh_addralign = chunk.get_common().shdr.sh_addralign;
            file_ofs += padding(file_ofs, sh_addralign);
            chunk.set_offset(&mut self.ctx, file_ofs);

            // Make sure to get sh_size after `chunk.set_offset` because we set a value to sh_size in it
            // TODO: bss and tbss
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
        let shstrtab_content = self.get_shstrtab_content();
        let (symtab_content, strtab_content) = self.get_symtab_and_strtab();
        let shdrs = self.get_shdrs();
        let phdrs = self.create_phdr();
        // copy all other sections and headers
        for chunk in self.chunks.iter_mut() {
            match chunk {
                // FIXME: dummy
                OutputChunk::Ehdr(chunk) => chunk.copy_buf(
                    buf,
                    0,
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
                    let chunk = self.ctx.get_output_section(chunk.get_id());
                    chunk.copy_buf(&self.ctx, buf);
                }
                OutputChunk::Strtab(chunk) => {
                    chunk.copy_buf(buf, &strtab_content);
                }
                OutputChunk::Symtab(chunk) => {
                    // TODO: st_value and st_shndx must be set
                    // 1. Set address to st_value. mold get symbol address by calling `Symbol::get_addr() const`
                    // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/object_file.cc#L732
                    // 2. Symbol::get_addr() const calls `InputSection::get_addr() const`
                    // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/mold.h#L1184
                    // 3. In turn, it gets address from sh_addr.
                    // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/mold.h#L1218
                    // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/mold.h#L1223
                    // 4. sh_addr is set in `set_osec_offsets()`
                    // https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L567

                    // TODO: rename OutputSection to `MergedSection`
                    // `MergedSection` contains multiple `SectionFragment`s
                    chunk.copy_buf(buf, &symtab_content);
                }
                OutputChunk::Shstrtab(chunk) => {
                    chunk.copy_buf(buf, &shstrtab_content);
                }
            }
        }
    }

    fn get_shdrs(&self) -> Vec<Elf64_Shdr> {
        let mut shdrs = vec![];
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

    fn get_symtab_and_strtab(&self) -> (Vec<Elf64_Sym>, Vec<u8>) {
        let mut symtab_content = vec![];
        let mut strtab_content = vec![0];
        for file in self.ctx.files() {
            for sym in file.get_symbols() {
                if let Some(sym) = sym {
                    if sym.should_write() {
                        let mut esym = sym.esym.get();
                        esym.st_name = strtab_content.len() as u32;
                        let name = &sym.name;

                        symtab_content.push(esym);
                        strtab_content.extend_from_slice(name.as_bytes());
                        strtab_content.push(0);
                    }
                }
            }
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
}

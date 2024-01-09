use elf::{section::Elf64_Shdr, symbol::Elf64_Sym};

use crate::{
    context::Context,
    output_section::{get_output_section_name, OutputChunk, OutputSectionId},
    utils::padding,
};

pub struct Linker {
    ctx: Context,
    // Move this to the main function
    pub chunks: Vec<OutputChunk>,
}

impl Linker {
    pub fn new(ctx: Context) -> Linker {
        Linker {
            ctx,
            chunks: vec![],
        }
    }

    pub fn get_ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn resolve_symbols(&mut self) {
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
        let shstrtab_content = self.get_shstrtab_content();
        for chunk in self.chunks.iter_mut() {
            if !chunk.is_header() {
                let name = chunk.get_section_name(&self.ctx);
                let shdr = &mut chunk.get_common_mut(&mut self.ctx).shdr;
                shdr.sh_name = calc_sh_name_from_shstrtab(&shstrtab_content, &name) as u32;
            }
        }

        let num_shdrs = self.calc_num_shdrs();
        let shstrtab_size = shstrtab_content.len() as u64;
        let (symtab_content, strtab_content) = self.get_symtab_and_strtab();
        for chunk in self.chunks.iter_mut() {
            match chunk {
                OutputChunk::Ehdr(_) => (/* Do nothing */),
                OutputChunk::Shdr(shdr) => {
                    shdr.update_shdr(num_shdrs);
                }
                OutputChunk::Phdr(_) => {
                    log::error!("TODO: update_shdr for Phdr");
                }
                OutputChunk::Section(_) => (/* Do nothing */),
                OutputChunk::Symtab(symtab) => symtab.update_shdr(symtab_content.len() as u64),
                OutputChunk::Strtab(strtab) => strtab.update_shdr(strtab_content.len() as u64),
                OutputChunk::Shstrtab(shstrtab) => shstrtab.update_shdr(shstrtab_size),
            }
        }
    }

    pub fn set_section_indices(&mut self) {
        let mut shndx = 0;
        for chunk in self.chunks.iter_mut() {
            if !chunk.is_header() {
                let common = chunk.get_common_mut(&mut self.ctx);
                common.shndx = Some(shndx);
                shndx += 1;
            }
        }
    }

    pub fn assign_osec_offsets(&mut self) -> u64 {
        let mut filesize = 0;
        //filesize += padding(filesize, 0x1000);
        for chunk in self.chunks.iter_mut() {
            let sh_addralign = chunk.get_common(&self.ctx).shdr.sh_addralign;
            let sh_size = chunk.get_common(&self.ctx).shdr.sh_size;
            //filesize += padding(filesize, sh_addralign);
            chunk.set_offset(&mut self.ctx, filesize);
            filesize += sh_size;
        }
        filesize
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
        let mut shdr_ofs = e_shoff;
        for chunk in &self.chunks {
            if !chunk.is_header() {
                let size = std::mem::size_of::<Elf64_Shdr>();
                let view = &chunk.get_common(&self.ctx).shdr as *const _ as *const u8;
                let slice = unsafe { std::slice::from_raw_parts(view, size) };
                buf[shdr_ofs as usize..shdr_ofs as usize + size].copy_from_slice(slice);
                shdr_ofs += size as u64;
            }
        }

        let e_shnum = self.calc_num_shdrs() as u16;
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
        let shstrtab_content = self.get_shstrtab_content();
        let (symtab_content, strtab_content) = self.get_symtab_and_strtab();
        // copy all other sections and headers
        for chunk in self.chunks.iter_mut() {
            match chunk {
                // FIXME: dummy
                OutputChunk::Ehdr(chunk) => {
                    chunk.copy_buf(buf, 0, 0, e_shoff, 0, e_shnum, e_shstrndx)
                }
                OutputChunk::Shdr(_) => {
                    // Do nothing
                }
                OutputChunk::Phdr(_) => {
                    log::error!("TODO: copy_buf for Phdr");
                }
                OutputChunk::Section(chunk) => {
                    let chunk = self.ctx.get_output_section(*chunk);
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

    fn calc_num_shdrs(&self) -> usize {
        let mut n = 0;
        for chunk in self.chunks.iter() {
            if !chunk.is_header() {
                n += 1;
            }
        }
        n
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
        let mut strtab_ofs = 1;
        for file in self.ctx.files() {
            for sym in file.get_symbols() {
                if let Some(sym) = sym {
                    if sym.should_write() {
                        let esym = sym.esym.get();
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
}

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

use elf::section::Elf64_Shdr;

use crate::{
    context::Context,
    output_section::{get_output_section_name, OutputChunk, OutputSectionId},
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
        let n = self.calc_num_shdrs();
        for chunk in self.chunks.iter_mut() {
            match chunk {
                OutputChunk::Ehdr(_) => (),
                OutputChunk::Shdr(shdr) => {
                    shdr.update_shdr(n);
                }
                OutputChunk::Phdr(_) => {
                    log::error!("TODO: update_shdr for Phdr");
                }
                OutputChunk::Section(_) => (),
                _ => panic!(),
            }
        }
    }

    pub fn assign_osec_offsets(&mut self) -> u64 {
        let mut filesize = 0;
        for chunk in self.chunks.iter_mut() {
            chunk.set_offset(&mut self.ctx, filesize);
            filesize += chunk.get_common(&self.ctx).shdr.sh_size;
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
        // copy all other sections and headers
        for chunk in self.chunks.iter_mut() {
            match chunk {
                // FIXME: dummy
                OutputChunk::Ehdr(chunk) => chunk.copy_buf(buf, 0, 0, e_shoff, 0, e_shnum, 0),
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
}

use crate::{
    context::Context,
    output_section::{get_output_section_name, OutputChunk, OutputSectionId},
};

pub struct Linker {
    ctx: Context,
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

            // Push the section to chunks at most once
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
        let mut n = 1;
        for chunk in self.chunks.iter() {
            if !chunk.is_header() {
                n += 1;
            }
        }

        for chunk in self.chunks.iter_mut() {
            match chunk {
                OutputChunk::Ehdr(_) => (),
                OutputChunk::Shdr(shdr) => {
                    shdr.update_shdr(n);
                }
                OutputChunk::Phdr(_) => {
                    // TODO:
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
        for chunk in self.chunks.iter_mut() {
            chunk.copy_buf(buf);
        }
    }
}

use crate::{
    context::Context,
    output_section::{
        get_output_section_name, OutputChunk, OutputEhdr, OutputPhdr, OutputSectionId, OutputShdr,
    },
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

    pub fn assign_offsets(&mut self) -> usize {
        let mut filesize = 0;
        for chunk in self.chunks.iter_mut() {
            chunk.set_offset(&mut self.ctx, filesize);
            filesize += chunk.get_size(&self.ctx);
        }
        filesize
    }

    pub fn update_shdr(&mut self) {
        // TODO:
    }

    pub fn copy_regular_sections(&mut self, buf: &mut [u8]) {
        for chunk in self.chunks.iter_mut() {
            let size = chunk.get_size(&self.ctx);
            let offset = chunk.get_offset(&self.ctx);
            if let OutputChunk::Section(section) = chunk {
                let section = self.ctx.get_output_section(*section);
                log::debug!(
                    "\tCopy {} bytes of {} to offset {}",
                    size,
                    section.get_name(),
                    offset,
                );
                section.copy_to(&self.ctx, buf);
            }
        }
    }
}

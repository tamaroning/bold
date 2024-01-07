use crate::{
    context::Context,
    output_section::{OutputChunk, OutputEhdr, OutputPhdr, OutputShdr},
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

    pub fn push_common_chunks(&mut self) {
        let ehdr = OutputChunk::Ehdr(OutputEhdr::new());
        let shdr = OutputChunk::Shdr(OutputShdr::new());
        let phdr = OutputChunk::Phdr(OutputPhdr::new());
        self.chunks.push(ehdr);
        self.chunks.push(shdr);
        self.chunks.push(phdr);
    }

    pub fn bin_input_sections(&mut self) {
        let mut input_sections = vec![];
        for file in self.ctx.files_mut() {
            for input_section in file.get_input_sections().iter() {
                if let Some(input_section) = input_section {
                    input_sections.push(*input_section);
                }
            }
        }

        for input_section_id in input_sections {
            let input_section = self.ctx.get_input_section(input_section_id);
            let output_section_name = input_section.output_section_name.clone();
            let output_section = self
                .ctx
                .get_output_section_by_name_mut(&output_section_name);

            // Push the section to chunks at most once
            if output_section.sections.is_empty() {
                let section = &output_section;
                self.chunks.push(OutputChunk::Section(section.get_id()));
            }
            output_section.sections.push(input_section_id);
        }
    }

    pub fn assign_offsets(&mut self) -> usize {
        let mut filesize = 0;
        for chunk in self.chunks.iter_mut() {
            chunk.set_offset(&mut self.ctx, filesize);
            filesize += chunk.get_size(&self.ctx);
        }
        filesize
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

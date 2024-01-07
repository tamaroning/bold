use std::{cell::RefCell, sync::Arc};

use crate::{
    context::Context,
    input_section::ObjectFile,
    output_section::{Chunk, OutputChunk, OutputEhdr, OutputPhdr, OutputShdr},
};

mod context;
mod input_section;
mod output_section;
mod utils;

fn main() {
    env_logger::builder().format_timestamp(None).init();
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let mut files = args[1..]
        .iter()
        .map(|arg| ObjectFile::read_from(arg.clone()))
        .collect::<Vec<_>>();

    for file in files.iter_mut() {
        log::info!("Parsing {}", file.get_file_name());
        file.parse();
    }

    // Set priorities to files
    // What is this?

    let mut ctx = Context::new(files);

    // Register (un)defined symbols
    log::info!("Resolving symbols");
    for file in ctx.files_mut() {
        file.register_defined_symbols();
        file.register_undefined_symbols();
    }

    ctx.dump();

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    let mut output_chunks: Vec<Arc<RefCell<OutputChunk>>> = vec![];
    let ehdr = Arc::new(RefCell::new(OutputChunk::Ehdr(OutputEhdr::new())));
    let shdr = Arc::new(RefCell::new(OutputChunk::Shdr(OutputShdr::new())));
    let phdr = Arc::new(RefCell::new(OutputChunk::Phdr(OutputPhdr::new())));
    output_chunks.push(Arc::clone(&ehdr));
    output_chunks.push(Arc::clone(&shdr));
    output_chunks.push(Arc::clone(&phdr));

    // Bin input sections into output sections
    log::info!("Merging sections");
    for file in ctx.files() {
        for input_section in file.get_input_sections().iter() {
            if let Some(input_section_ref) = input_section {
                let input_section = input_section_ref.read().unwrap();
                let output_section_name = &input_section.output_section_name;
                let output_section_ref = output_sections.get_section_by_name(output_section_name);
                let mut output_section = output_section_ref.borrow_mut();

                // Push the section to chunks at most once
                if output_section.sections.is_empty() {
                    let section = &output_section_ref;
                    output_chunks.push(OutputChunk::Section(section));
                }

                output_section.sections.push(Arc::clone(&input_section_ref));
            }
        }
    }

    // Assign offsets to input sections
    log::info!("Assigning offsets");
    let mut filesize = 0;
    for chunk in output_chunks.iter_mut() {
        let mut chunk = chunk.borrow_mut();
        chunk.set_offset(filesize);
        filesize += chunk.get_size();
    }

    // Create an output file

    // Allocate a buffer for the output file
    // TODO: We should not zero-clear the buffer for performance reasons
    let mut buf: Vec<u8> = vec![];
    buf.resize(filesize, 0);

    // Copy input sections to the output file
    log::info!("Copying regular sections");
    for chunk in output_chunks.iter_mut() {
        let chunk = chunk.borrow();
        if let OutputChunk::Section(section) = &*chunk {
            let section = section.borrow();
            log::debug!(
                "\tCopy {} bytes of {} to offset {}",
                section.get_size(),
                section.get_name(),
                section.get_offset(),
            );
            section.copy_to(&mut buf);
        }
    }

    let mut shdr_ref = shdr.borrow_mut();
    if let OutputChunk::Shdr(shdr) = &mut *shdr_ref {
        shdr.update_shdr(&output_chunks);
    } else {
        unreachable!();
    }

    log::info!("Copying ELF heades");
    let ehdr_ref = ehdr.borrow_mut();
    if let OutputChunk::Ehdr(ehdr) = &*ehdr_ref {
        ehdr.copy_to(&mut buf);
    } else {
        unreachable!();
    }
    //shdr.copy_to(&mut buf);
    //phdr.copy_to(&mut buf);
}

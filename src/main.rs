use std::sync::{Arc, RwLock};

use crate::{context::Context, input_section::ObjectFile, output_section::OutputChunk};

mod context;
mod input_section;
mod output_section;

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
    ctx.resovle_symbols();

    ctx.dump();

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    let mut output_chunks: Vec<Arc<RwLock<dyn OutputChunk>>> = vec![];

    // Bin input sections into output sections
    log::info!("Merging sections");
    for file in ctx.files() {
        let file = file.borrow_mut();
        for input_section in file.get_input_sections().iter() {
            if let Some(input_section_ref) = input_section {
                let input_section = input_section_ref.read().unwrap();
                let output_section_ref = &input_section.output_section;
                let mut output_section = output_section_ref.write().unwrap();

                // Push the section to chunks at most once
                if output_section.sections.is_empty() {
                    let chunk = Arc::clone(output_section_ref) as Arc<RwLock<dyn OutputChunk>>;
                    output_chunks.push(chunk);
                }

                output_section.sections.push(Arc::clone(&input_section_ref));
            }
        }
    }

    // Assign offsets to input sections
    log::info!("Assigning offsets");
    let mut filesize = 0;
    for chunk in output_chunks.iter_mut() {
        let mut chunk = chunk.write().unwrap();
        chunk.set_offset(filesize);
        filesize += chunk.get_size();
    }

    // Create an output file

    // Allocate a buffer for the output file
    // TODO: We should not zero-clear the buffer for performance reasons
    let mut buf: Vec<u8> = vec![];
    buf.resize(filesize, 0);

    // Copy input sections to the output file
    log::debug!("Copying chunks");
    for chunk in output_chunks.iter_mut() {
        let chunk = chunk.write().unwrap();
        log::debug!(
            "\tCopy {} bytes of {} to offset {}",
            chunk.get_size(),
            chunk.as_string(),
            chunk.get_offset(),
        );
        chunk.copy_to(&mut buf);
    }

    // Relocation
}

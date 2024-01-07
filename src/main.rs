use crate::{
    context::Context,
    input_section::ObjectFile,
};

mod context;
mod input_section;
mod linker;
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

    let mut ctx = Context::new();

    for file in files.iter_mut() {
        log::info!("Parsing {}", file.get_file_name());
        file.parse(&mut ctx);
    }

    // Set priorities to files
    // What is this?

    for file in files {
        ctx.set_object_file(file);
    }

    let mut linker = linker::Linker::new(ctx);

    // Register (un)defined symbols
    log::info!("Resolving symbols");
    linker.resolve_symbols();

    linker.get_ctx().dump();

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    linker.arrange_chunks();

    // Bin input sections into output sections
    log::info!("Merging sections");
    linker.merge_sections();

    // Assign offsets to input sections
    log::info!("Assigning offsets");
    let filesize = linker.assign_offsets();

    // Create an output file

    // Allocate a buffer for the output file
    // TODO: We should not zero-clear the buffer for performance reasons
    let mut buf: Vec<u8> = vec![];
    buf.resize(filesize, 0);

    // Copy input sections to the output file
    log::info!("Copying regular sections");
    linker.copy_regular_sections(&mut buf);

}

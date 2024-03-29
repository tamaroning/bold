use std::{io::Write, path::Path};

use crate::{
    context::Context,
    input_section::ObjectFile,
    output_section::{
        OutputChunk, OutputEhdr, OutputPhdr, OutputSectionRef, OutputShdr, Shstrtab, Strtab, Symtab,
    },
};

mod config;
mod context;
mod input_section;
mod linker;
mod output_section;
mod relocation;
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
        .flat_map(|arg| ObjectFile::read_from(arg))
        .collect::<Vec<_>>();

    let mut ctx = Context::new();

    for file in files.iter_mut() {
        log::debug!("Parsing {}", file.get_file_name());
        file.parse(&mut ctx);
    }

    // Set priorities to files
    // What is this?

    for file in files {
        ctx.set_object_file(file);
    }

    let config = config::Config::new();
    let mut linker = linker::Linker::new(ctx, &config);

    let ehdr = OutputChunk::Ehdr(OutputEhdr::new());
    let shdr = OutputChunk::Shdr(OutputShdr::new());
    let phdr = OutputChunk::Phdr(OutputPhdr::new());
    let symtab = OutputChunk::Symtab(Symtab::new());
    let strtab = OutputChunk::Strtab(Strtab::new());
    let shstrtab = OutputChunk::Shstrtab(Shstrtab::new());

    // Register (un)defined symbols
    log::info!("Resolving symbols");
    linker.resolve_symbols();

    linker.get_ctx().dump();

    // Eliminate unused archive members
    // What is this?

    // Eliminate duplicate comdat groups
    // What is this?

    // Bin input sections into output sections
    // mold: bin_sections
    log::info!("Merging sections");
    let output_sections = linker.bin_input_sections();

    // Assign offsets to input sections
    // mold: set_isec_offsets
    log::info!("Assigning isec offsets");
    linker.assign_isec_offsets();

    // Add sections to the section lists
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1214
    // TODO: merged sections?
    for output_section in output_sections {
        let output_section = linker.get_ctx().get_output_section(output_section);
        linker
            .chunks
            .push(OutputChunk::Section(OutputSectionRef::from(output_section)));
    }

    // TODO: Sort the sections by section flags so that we'll have to create
    // as few segments as possible.
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1224

    // Beyond this point, no new symbols will be added to the result.

    // TODO: Convert weak symbols to absolute symbols with value 0
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1236

    // TODO: Make sure that all symbols have been resolved
    // mold: check_duplicate_symbols

    // TODO: Copy shared object name strings to .dynstr.
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1249

    // Copy DT_RUNPATH strings to .dynstr.
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1254

    // Add headers and sections that have to be at the beginning
    // or the ending of a file.
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1256
    linker.chunks.insert(0, ehdr);
    linker.chunks.insert(1, phdr);
    linker.chunks.insert(2, shdr);
    linker.chunks.push(symtab);
    linker.chunks.push(strtab);
    linker.chunks.push(shstrtab);
    // TODO: interp

    // TODO: Scan relocations to find symbols that need entries in .got, .plt,
    // .got.plt, .dynsym, .dynstr, etc.
    // mold: scan_rels

    // TODO: Put symbols to .dynsym.
    // mold: export_dynamic

    // TODO: Sort .dynsym contents. Beyond this point, no symbol should be
    // added to .dynsym.
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1271

    // TODO: Fill .gnu.version and .gnu.version_r section contents.
    // mold: fill_symbol_versions

    // TODO: Compute .symtab and .strtab sizes for each file.
    // mold: ObjectFile::compute_symtab

    // TODO: delete empty output sections

    // FIXME: update_shdr should be called here?

    // Set section indices
    log::debug!("Setting section indices");
    linker.set_section_indices();

    // TODO: eh_frame
    // mold: https://github.com/tamaroning/mold/blob/3489a464c6577ea1ee19f6b9ae3fe46237f4e4ee/main.cc#L1283

    linker.update_shdr();

    log::debug!("Assigning osec offsets");
    let filesize = linker.assign_osec_offsets();
    log::debug!("File size: {}", filesize);

    // mold: https://github.com/tamaroning/mold/blob/c3a86f5b24343f020edfac1f683dea3648a30e61/elf/main.cc#L629
    linker.fix_synthetic_symbols();

    // Create an output file

    // Allocate a buffer for the output file
    // TODO: We should not zero-clear the buffer for performance reasons
    let mut buf: Vec<u8> = vec![];
    buf.resize(filesize as usize, 0);

    log::debug!("Chunks:");
    for chunk in linker.chunks.iter() {
        let shndx = chunk.get_common().shndx;
        log::debug!(
            "\t[{}]: {}",
            shndx.map(|x| x.to_string()).unwrap_or("-".to_string()),
            chunk.as_string(linker.get_ctx())
        );
    }

    // Copy input sections to the output file
    log::info!("Copying sections to buffer");
    linker.copy_buf(&mut buf);
    linker.relocation(&mut buf);

    log::info!("Writing buffer to file");
    let filepath = Path::new("a.out");
    let mut f = std::fs::File::create(filepath).unwrap();
    f.write_all(&buf).unwrap();
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(filepath)
        .output()
        .expect("Failed to execute chmod");
    log::info!(
        "Successfully wrote to {}",
        std::fs::canonicalize(filepath).unwrap().to_str().unwrap()
    );
}

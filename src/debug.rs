use crate::serial_println;
use alloc::borrow::Cow;
use alloc::vec::Vec;
use object::read::macho;
use object::{Object, ObjectSection};

pub fn print_stacktrace() {
    serial_println!("stack trace:");
    let elf: &[u8] = unsafe { std::slice::from_raw_parts(0x7F_8000_0000, 8) };
    let image = macho::DyldCache::parse(elf, &[]);
    object::File::parse_dyld_cache_image(image);
    //let endian = if object.is_little_endian() {
    //    gimli::RunTimeEndian::Little
    //} else {
    //    gimli::RunTimeEndian::Big
    //};
    dump_file(object, endian);
    find_cfi_sections();
}

fn dump_file(object: &object::File, endian: gimli::RunTimeEndian) -> Result<(), gimli::Error> {
    // Load a section and return as `Cow<[u8]>`.
    let load_section = |id: gimli::SectionId| -> Result<Cow<[u8]>, gimli::Error> {
        match object.section_by_name(id.name()) {
            Some(ref section) => Ok(section
                .uncompressed_data()
                .unwrap_or(Cow::Borrowed(&[][..]))),
            None => Ok(Cow::Borrowed(&[][..])),
        }
    };

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&load_section)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section: &dyn for<'a> Fn(
        &'a Cow<[u8]>,
    ) -> gimli::EndianSlice<'a, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);

    // Iterate over the compilation units.
    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        serial_println!(
            "Unit at <.debug_info+0x{:x}>",
            header.offset().as_debug_info_offset().unwrap().0
        );
        let unit = dwarf.unit(header)?;

        // Iterate over the Debugging Information Entries (DIEs) in the unit.
        let mut depth = 0;
        let mut entries = unit.entries();
        while let Some((delta_depth, entry)) = entries.next_dfs()? {
            depth += delta_depth;
            serial_println!("<{}><{:x}> {}", depth, entry.offset().0, entry.tag());

            // Iterate over the attributes in the DIE.
            let mut attrs = entry.attrs();
            while let Some(attr) = attrs.next()? {
                serial_println!("   {}: {:?}", attr.name(), attr.value());
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct EhRef {
    pub text: AddrRange,
    pub eh_frame_hdr: AddrRange,
    pub eh_frame_end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddrRange {
    pub start: u64,
    pub end: u64,
}

impl AddrRange {
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn len(&self) -> u64 {
        self.end - self.start
    }
}

extern "C" {
    static __text_start: usize;
    static __text_end: usize;
    static __ehframehdr_start: usize;
    static __ehframehdr_end: usize;
    static __ehframe_end: usize;
}

pub fn find_cfi_sections() -> Vec<EhRef> {
    let mut cfi: Vec<EhRef> = Vec::new();
    unsafe {
        // Safety: None of those are actual accesses - we only get the address
        // of those values.
        let text = AddrRange {
            start: &__text_start as *const _ as u64,
            end: &__text_end as *const _ as u64,
        };
        let eh_frame_hdr = AddrRange {
            start: &__ehframehdr_start as *const _ as u64,
            end: &__ehframehdr_end as *const _ as u64,
        };
        let eh_frame_end = &__ehframe_end as *const _ as u64;

        cfi.push(EhRef {
            text,
            eh_frame_hdr,
            eh_frame_end,
        });
    }
    serial_println!("CFI sections: {:?}", cfi);
    cfi
}

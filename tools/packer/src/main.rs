use packer_abi::{Entry, Header};
use std::{env, fs};

fn main() -> std::io::Result<()> {
    // args: <input_dir> <out_bundle>
    let mut args = env::args().skip(1);
    let dir = args.next().expect("input dir");
    let out = args.next().expect("out bundle");

    // Collect files (regular files only)
    let mut items = Vec::new();
    for ent in fs::read_dir(&dir)? {
        let ent = ent?;
        let md = ent.metadata()?;
        if md.is_file() {
            let name = ent.file_name().into_string().expect("utf8 name");
            let bytes = fs::read(ent.path())?;
            items.push((name, bytes));
        }
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));
    let count = items.len();

    // Build names blob (NUL-terminated, 8B aligned)
    let mut names = Vec::new();
    let mut name_offs = Vec::with_capacity(count);
    for (name, _) in &items {
        let off = names.len();
        names.extend_from_slice(name.as_bytes());
        names.push(0);
        name_offs.push(off);
    }

    while names.len() % 8 != 0 {
        names.push(0);
    }

    // Build files blob and file offsets
    let mut files = Vec::new();
    let mut file_offs = Vec::with_capacity(count);
    for (_, data) in &items {
        let off = files.len();
        files.extend_from_slice(data);
        while files.len() % 8 != 0 {
            files.push(0);
        } // pad each file to 8B
        file_offs.push((off, data.len()));
    }

    // Header + entries
    let mut out_bytes = Vec::new();
    let hdr_size = size_of::<Header>();
    let ents_size = count * size_of::<Entry>();
    let entries_off = align8(hdr_size);
    let names_off = align8(entries_off + ents_size);
    let files_off = align8(names_off + names.len());

    // Write header (placeholder; patch later)
    out_bytes.resize(files_off, 0);

    // Entries
    for i in 0..count {
        let e = Entry {
            name_off: name_offs[i] as u64,
            file_off: file_offs[i].0 as u64,
            file_len: file_offs[i].1 as u64,
        };

        let p = entries_off + i * size_of::<Entry>();
        out_bytes[p..p + 24].copy_from_slice(unsafe {
            std::slice::from_raw_parts((&raw const e).cast::<u8>(), 24)
        });
    }

    // Names + Files
    out_bytes[names_off..names_off + names.len()].copy_from_slice(&names);
    out_bytes.extend_from_slice(&files);

    // Final header
    let hdr = Header {
        count: u32::try_from(count).expect("invalid count"),
        names_off: names_off as u64,
        files_off: files_off as u64,
        entries_off: entries_off as u64,
        ..Header::default()
    };

    out_bytes[0..size_of::<Header>()].copy_from_slice(unsafe {
        std::slice::from_raw_parts((&raw const hdr).cast::<u8>(), hdr_size)
    });

    fs::write(&out, &out_bytes)?;
    eprintln!("packed {count} files into {out}");
    Ok(())
}

const fn align8(x: usize) -> usize {
    (x + 7) & !7
}

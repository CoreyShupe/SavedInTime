use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use tar::{Builder, Header};

use crate::processor::{Entry, EntryType};

pub fn create_tarball<P: AsRef<Path>>(
    origin: P,
    entries: Vec<Entry>,
    tarball_path: P,
) -> std::io::Result<()> {
    let tarball_file = File::create(tarball_path)?;
    let tarball_writer = BufWriter::new(tarball_file);

    let mut builder = Builder::new(tarball_writer);

    let origin_buf = origin.as_ref().to_path_buf();

    for entry in entries {
        let relative_path = find_relative_path(&origin_buf, &entry.path);

        match entry.entry_type {
            EntryType::File(data) => {
                let mut header = Header::new_gnu();
                header.set_path(relative_path)?;
                header.set_metadata(&entry.metadata);
                header.set_cksum();
                builder.append(&header, Cursor::new(data))?;
            }
            EntryType::Symlink => match entry.path.read_link() {
                Ok(link) => {
                    if !link.starts_with(origin.as_ref()) {
                        log::error!(
                            "Symlink points outside of the target directory: {}",
                            link.display()
                        );
                        continue;
                    }
                    let mut header = Header::new_gnu();
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    builder.append_link(
                        &mut header,
                        relative_path,
                        find_relative_path(&origin_buf, link),
                    )?;
                }
                Err(_) => {
                    log::error!("Failed to resolve symlink: {}", entry.path.display());
                    continue;
                }
            },
        }
    }

    builder.into_inner()?.flush()
}

fn find_relative_path<P1: AsRef<Path>, P2: AsRef<Path>>(origin: P1, relative: P2) -> PathBuf {
    let origin_path_path = origin.as_ref();
    let relative_path = relative.as_ref();

    if !relative_path.starts_with(origin_path_path) {
        panic!(
            "Something went wrong; could not find relative path between {} and {}",
            origin_path_path.display(),
            relative_path.display()
        );
    }

    let relative_path = origin_path_path.join(
        relative_path
            .strip_prefix(origin_path_path)
            .expect("Something went wrong; could not strip prefix."),
    );

    relative_path.to_path_buf()
}

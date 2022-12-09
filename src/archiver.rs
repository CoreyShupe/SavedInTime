use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tar::Header;

fn archive_processed_info(origin: PathBuf, processed_data: HashMap<PathBuf, Vec<u8>>) {
    let mut archive = tar::Builder::new(Vec::new());
    for (path, data) in processed_data {
        let relative = find_relative_path(&origin, &path);
        log::trace!(
            "Adding {} to archive; size {}",
            relative.display(),
            data.len()
        );
        let header = Header::new_gnu();
    }
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

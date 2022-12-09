use std::collections::HashMap;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Entry {
    path: PathBuf,
    metadata: Metadata,
    data: Option<Vec<u8>>,
}

impl From<WeakEntry> for Entry {
    fn from(value: WeakEntry) -> Self {
        Self {
            path: value.path,
            metadata: value.metadata,
            data: Some(value.encoded_data),
        }
    }
}

impl From<SymlinkEntry> for Entry {
    fn from(value: SymlinkEntry) -> Self {
        Self {
            path: value.path,
            metadata: value.metadata,
            data: None,
        }
    }
}

struct WeakEntry {
    path: PathBuf,
    metadata: Metadata,
    visit_revision: SystemTime,
    encoded_data: Vec<u8>,
}

impl WeakEntry {
    // returns true if the error is recoverable; if it should try again
    pub fn new<P: AsRef<Path>>(path: P, visit_revision: SystemTime) -> Result<Self, bool> {
        let path_buf = path.as_ref().to_path_buf();
        let metadata = match path.as_ref().metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!path_buf.exists()),
        };
        if let Ok(modified) = metadata.modified() {
            if modified > visit_revision {
                return Err(true);
            }
        }
        let encoded_data =
            match zstd::encode_all(File::open(path).unwrap(), zstd::DEFAULT_COMPRESSION_LEVEL) {
                Ok(data) => data,
                Err(err) => {
                    log::error!("Failed to encode data for {}: {}", path_buf.display(), err);
                    return Err(true);
                }
            };
        Ok(Self {
            path: path_buf,
            metadata,
            visit_revision,
            encoded_data,
        })
    }

    pub fn visit(&mut self, visit_revision: SystemTime) -> Result<(), bool> {
        let metadata = match self.path.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!self.path.exists()),
        };
        if let Ok(modified) = metadata.modified() {
            if modified > visit_revision {
                return Err(true);
            }
            if modified < self.visit_revision {
                self.visit_revision = visit_revision;
                return Ok(());
            }
        } else {
            return Ok(()); // if we can't get the modified time, we can't do anything
        }
        self.metadata = metadata;
        self.encoded_data = match zstd::encode_all(
            File::open(&self.path).unwrap(),
            zstd::DEFAULT_COMPRESSION_LEVEL,
        ) {
            Ok(data) => data,
            Err(err) => {
                log::error!("Failed to encode data for {}: {}", self.path.display(), err);
                return Err(true);
            }
        };
        self.visit_revision = visit_revision;
        Ok(())
    }
}

struct SymlinkEntry {
    path: PathBuf,
    metadata: Metadata,
    visit_revision: SystemTime,
}

impl SymlinkEntry {
    pub fn new<P: AsRef<Path>>(path: P, visit_revision: SystemTime) -> Result<Self, bool> {
        let path_buf = path.as_ref().to_path_buf();
        let metadata = match path.as_ref().metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!path_buf.exists()),
        };
        Ok(Self {
            path: path_buf,
            metadata,
            visit_revision,
        })
    }

    pub fn visit(&mut self, visit_revision: SystemTime) -> Result<(), bool> {
        let metadata = match self.path.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!self.path.exists()),
        };
        self.metadata = metadata;
        self.visit_revision = visit_revision;
        Ok(())
    }
}

// represents a "directory" managing all of it's subdirectories and files
struct Visitor {
    origin: PathBuf,
    metadata: Option<Metadata>,
    revision: SystemTime,
    entries: HashMap<PathBuf, WeakEntry>,
    sub_visitors: HashMap<PathBuf, Visitor>,
    links: HashMap<PathBuf, SymlinkEntry>,
}

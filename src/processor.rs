use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Entry {
    pub path: PathBuf,
    pub metadata: Metadata,
    pub entry_type: EntryType,
}

pub enum EntryType {
    File(Vec<u8>),
    Symlink,
}

#[derive(Debug)]
pub enum ProcessError {
    UnrecoverableUnknown,
    PathNotDir,
    MetadataFetchFailed,
    IterationBoundExceeded,
}

impl Error for ProcessError {}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::UnrecoverableUnknown => write!(f, "Unrecoverable unknown error"),
            ProcessError::PathNotDir => write!(f, "Path is not a directory"),
            ProcessError::MetadataFetchFailed => write!(f, "Failed to fetch metadata"),
            ProcessError::IterationBoundExceeded => write!(f, "Iteration bound exceeded"),
        }
    }
}

pub fn process_directory<P: AsRef<Path>>(
    directory_path: P,
    max_iterations: i32,
    compression_level: i32,
) -> Result<Vec<Entry>, ProcessError> {
    let path = directory_path.as_ref();
    if !path.is_dir() {
        return Err(ProcessError::PathNotDir);
    }
    let mut iterations = 0;
    let mut visitor = Visitor::create(path, SystemTime::now(), compression_level)
        .map_err(|err| ProcessError::MetadataFetchFailed)?;
    let mut last_time = SystemTime::now();
    while match visitor.visit(last_time, compression_level) {
        Ok(_) => false,
        Err(recoverable) => {
            if iterations >= max_iterations {
                return Err(ProcessError::IterationBoundExceeded);
            }
            if recoverable {
                true
            } else {
                return Err(ProcessError::UnrecoverableUnknown);
            }
        }
    } {
        iterations += 1;
        last_time = SystemTime::now();
    }

    let mut compiled_entries = Vec::new();
    visitor.compile(last_time, &mut compiled_entries);
    Ok(compiled_entries)
}

impl From<WeakEntry> for Entry {
    fn from(value: WeakEntry) -> Self {
        Self {
            path: value.path,
            metadata: value.metadata,
            entry_type: EntryType::File(value.encoded_data),
        }
    }
}

impl From<SymlinkEntry> for Entry {
    fn from(value: SymlinkEntry) -> Self {
        Self {
            path: value.path,
            metadata: value.metadata,
            entry_type: EntryType::Symlink,
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
    pub fn new<P: AsRef<Path>>(
        path: P,
        visit_revision: SystemTime,
        compression_level: i32,
    ) -> Result<Self, bool> {
        let path_buf = path.as_ref().to_path_buf();
        let metadata = match path.as_ref().metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!path_buf.exists()),
        };
        if let Ok(modified) = metadata.modified() {
            if modified > visit_revision {
                log::info!(
                    "File {} was modified after the visit revision; skipping, will revisit.",
                    path_buf.display()
                );
                return Err(true);
            }
        }
        let mut self_ref = Self {
            path: path_buf,
            metadata,
            visit_revision,
            encoded_data: vec![],
        };
        self_ref.fvisit(compression_level)?;
        Ok(self_ref)
    }

    pub fn visit(
        &mut self,
        visit_revision: SystemTime,
        compression_level: i32,
    ) -> Result<(), bool> {
        let metadata = match self.path.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!self.path.exists()),
        };
        if let Ok(modified) = metadata.modified() {
            if modified > visit_revision {
                log::info!(
                    "File {} was modified after the visit revision; skipping, will revisit.",
                    self.path.display()
                );
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
        self.fvisit(compression_level)?;
        self.visit_revision = visit_revision;
        Ok(())
    }

    fn fvisit(&mut self, compression_level: i32) -> Result<(), bool> {
        self.encoded_data =
            match zstd::encode_all(File::open(&self.path).unwrap(), compression_level) {
                Ok(data) => data,
                Err(err) => {
                    log::error!("Failed to encode data for {}: {}", self.path.display(), err);
                    return Err(true);
                }
            };
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

struct Visitor {
    origin: PathBuf,
    metadata: Metadata,
    revision: SystemTime,
    entries: HashMap<PathBuf, WeakEntry>,
    sub_visitors: HashMap<PathBuf, Visitor>,
    links: HashMap<PathBuf, SymlinkEntry>,
}

impl Visitor {
    pub fn create<P: AsRef<Path>>(
        path: P,
        revision: SystemTime,
        compression_level: i32,
    ) -> Result<Self, bool> {
        let path_buf = path.as_ref().to_path_buf();
        let metadata = match path.as_ref().metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!path_buf.exists()),
        };

        if let Ok(modified) = metadata.modified() {
            if modified > revision {
                log::info!(
                    "Directory {} was modified after the visit revision; skipping, will revisit.",
                    path_buf.display()
                );
                return Err(true);
            }
        }

        Ok(Self {
            origin: path_buf,
            metadata,
            revision,
            entries: HashMap::new(),
            sub_visitors: HashMap::new(),
            links: HashMap::new(),
        })
    }

    pub fn visit(
        &mut self,
        visit_revision: SystemTime,
        compression_level: i32,
    ) -> Result<(), bool> {
        let metadata = match self.origin.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Err(!self.origin.exists()),
        };

        if let Ok(modified) = metadata.modified() {
            if modified > visit_revision {
                log::info!(
                    "Directory {} was modified after the visit revision; skipping, will revisit.",
                    self.origin.display()
                );
                return Err(true);
            }
        }

        self.metadata = metadata;
        self.fvisit(visit_revision, compression_level)
    }

    pub fn fvisit(
        &mut self,
        visit_revision: SystemTime,
        compression_level: i32,
    ) -> Result<(), bool> {
        for entry in match self.origin.read_dir() {
            Ok(read_dir) => read_dir,
            Err(err) => {
                log::error!(
                    "Failed to read directory {}: {}",
                    self.origin.display(),
                    err
                );
                return Err(false);
            }
        } {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    log::error!("Failed to read directory entry: {}", err);
                    return Err(true);
                }
            };
            let path = entry.path();
            if path.is_dir() {
                if self.sub_visitors.contains_key(&path) {
                    self.sub_visitors
                        .get_mut(&path)
                        .unwrap()
                        .visit(visit_revision, compression_level)?;
                } else {
                    let visitor = Visitor::create(&path, visit_revision, compression_level)?;
                    self.sub_visitors.insert(path.clone(), visitor);
                    self.sub_visitors // we want to ensure we can cache what's possible
                        .get_mut(&path)
                        .unwrap()
                        .fvisit(visit_revision, compression_level)?;
                }
            } else if path.is_file() {
                if self.entries.contains_key(&path) {
                    self.entries
                        .get_mut(&path)
                        .unwrap()
                        .visit(visit_revision, compression_level)?;
                } else {
                    let entry = WeakEntry::new(&path, visit_revision, compression_level)?;
                    self.entries.insert(path.clone(), entry);
                }
            } else if path.is_symlink() {
                if self.links.contains_key(&path) {
                    self.links.get_mut(&path).unwrap().visit(visit_revision)?;
                } else {
                    let entry = SymlinkEntry::new(&path, visit_revision)?;
                    self.links.insert(path.clone(), entry);
                }
            } else {
                log::error!("Failed to process path {}, what is this?", path.display());
            }
        }
        Ok(())
    }

    pub fn compile(self, time_to_match: SystemTime, compiled_entries: &mut Vec<Entry>) {
        if self.revision != time_to_match {
            return;
        }

        for (_, entry) in self.entries {
            if entry.visit_revision == time_to_match {
                compiled_entries.push(Entry::from(entry));
            }
        }
        for (_, visitor) in self.sub_visitors {
            visitor.compile(time_to_match, compiled_entries);
        }
        for (_, link) in self.links {
            if link.visit_revision == time_to_match {
                compiled_entries.push(Entry::from(link));
            }
        }
    }
}

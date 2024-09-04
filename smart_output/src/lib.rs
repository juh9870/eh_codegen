use crate::utils::{compress, decompress, sha256};
use ahash::AHashSet;
use bytes::Bytes;
use flate2::Compression;
use miette::Diagnostic;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::debug;

mod utils;

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("Failed to read marker file at `{}`: {}", .path.display(), .source)]
    ManagedFileReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to write marker file to `{}`: {}", .path.display(), .source)]
    ManagedFileWriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to decode marker file")]
    ManagedFileDecodeError { source: bitcode::Error },
    #[error("Managed file backup present at `{}`, refusing to overwrite", .path.display())]
    ManagedFileBackupPresent { path: PathBuf },
    #[error("Failed to create marker file backup at `{}`: {}", .path.display(), .source)]
    ManagedFileBackupError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to remove marker file backup at `{}`: {}", .path.display(), .source)]
    ManagedFileBackupDeleteError {
        path: PathBuf,
        #[source]
        source: trash::Error,
    },

    #[error("Output directory is not empty and lacks `.managed_files` marker: path=`{}`", .path.display()
    )]
    NewProjectDirectoryNotEmpty { path: PathBuf },

    #[error("Failed to read project directory at `{}`: {}", .path.display(), .source)]
    ProjectDirReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Output file is outside of the output directory: root=`{}`, path=`{}`", .root.display(), .path.display()
    )]
    FileOutsideRoot { root: PathBuf, path: PathBuf },
    #[error("Output file is a duplicate: `{}`", .path.display())]
    DuplicateFile { path: PathBuf },

    #[error("Failed to create parent directory at `{}`: {}", .path.display(), .source)]
    ParentDirCreateError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file at `{}`: {}", .path.display(), .source)]
    FileWriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to cleanup files: {}", .source)]
    CleanupError {
        #[source]
        source: trash::Error,
    },

    #[error("Path `{}` contains non-UTF8 sequences", .path.display())]
    NonUtf8Path { path: PathBuf },
}

type Result<T, E = Error> = std::result::Result<T, E>;

const MANAGED_FILES_NAME: &str = ".managed_files";
const MANAGED_FILES_BACKUP_NAME: &str = ".managed_files.bk";

#[must_use]
#[derive(Debug)]
pub struct SmartOutput {
    files: BTreeMap<PathBuf, Bytes>,
    root: PathBuf,
    managed_files_path: PathBuf,
    managed_files_backup_path: PathBuf,
    parent_dirs: AHashSet<PathBuf>,
    hashes: BTreeMap<String, Vec<u8>>,
}

impl SmartOutput {
    pub fn init(path: PathBuf) -> Result<Self> {
        let managed_files_path = path.join(MANAGED_FILES_NAME);
        let managed_files_backup_path = path.join(MANAGED_FILES_BACKUP_NAME);
        let mut out = Self {
            files: BTreeMap::new(),
            hashes: Default::default(),
            root: path,
            managed_files_path,
            managed_files_backup_path,
            parent_dirs: Default::default(),
        };

        out.init_hashes()?;

        Ok(out)
    }

    fn init_hashes(&mut self) -> Result<()> {
        if self.managed_files_backup_path.exists() {
            return Err(Error::ManagedFileBackupPresent {
                path: self.managed_files_backup_path.to_path_buf(),
            });
        };

        let hashes = if self.managed_files_path.exists() {
            let data = fs_err::read(&self.managed_files_path).map_err(|e| {
                Error::ManagedFileReadError {
                    path: self.managed_files_path.to_path_buf(),
                    source: e,
                }
            })?;
            let data = decompress(&data);
            bitcode::decode(&data).map_err(|e| Error::ManagedFileDecodeError { source: e })?
        } else {
            // todo: re-enable this as a config option for projects that want to be ultra-safe?
            // if self.root.exists()
            //     && fs_err::read_dir(&self.root)
            //         .map_err(|e| Error::ProjectDirReadError {
            //             path: self.root.to_path_buf(),
            //             source: e,
            //         })?
            //         .next()
            //         .is_some()
            // {
            //     return Err(Error::NewProjectDirectoryNotEmpty {
            //         path: self.root.to_path_buf(),
            //     });
            // }

            fs_err::write(
                &self.managed_files_path,
                compress(
                    &bitcode::encode(&BTreeMap::<String, Vec<u8>>::new()),
                    Compression::best(),
                ),
            )
            .map_err(|e| Error::ManagedFileWriteError {
                path: self.managed_files_path.to_path_buf(),
                source: e,
            })?;

            BTreeMap::<String, Vec<u8>>::default()
        };

        self.hashes = hashes;

        Ok(())
    }

    pub fn add_file(&mut self, path: PathBuf, content: impl Into<Bytes>) -> Result<()> {
        if !path.starts_with(&self.root) {
            return Err(Error::FileOutsideRoot {
                root: self.root.clone(),
                path,
            });
        }

        match self.files.entry(path.clone()) {
            Entry::Occupied(_) => return Err(Error::DuplicateFile { path }),
            Entry::Vacant(entry) => {
                entry.insert(content.into());
            }
        }

        let parent = path.parent().expect("Path has parent");

        self.parent_dirs.insert(parent.to_path_buf());

        Ok(())
    }

    /// Flushes the output to the filesystem
    pub fn flush(self) -> Result<()> {
        use rayon::prelude::*;

        let SmartOutput {
            files,
            root,
            managed_files_path,
            managed_files_backup_path,
            parent_dirs,
            hashes,
        } = self;

        fs_err::copy(&managed_files_path, &managed_files_backup_path).map_err(|e| {
            Error::ManagedFileBackupError {
                path: managed_files_backup_path.to_path_buf(),
                source: e,
            }
        })?;

        parent_dirs.par_iter().try_for_each(|p| {
            fs_err::create_dir_all(p).map_err(|e| Error::ParentDirCreateError {
                path: p.to_path_buf(),
                source: e,
            })
        })?;

        let updated_count = Arc::new(AtomicUsize::new(0));
        let total_to_write = files.len();

        fn try_write_file(
            root: &Path,
            path: &Path,
            data: Bytes,
            hashes: &BTreeMap<String, Vec<u8>>,
            updated_count: &AtomicUsize,
        ) -> Result<Option<(String, Vec<u8>)>> {
            let relative = path
                .strip_prefix(root)
                .expect("All file paths are inside root");
            let Some(relative) = relative.as_os_str().to_str() else {
                return Err(Error::NonUtf8Path {
                    path: path.to_path_buf(),
                });
            };
            let hash = sha256(&data);

            let old_hash = hashes.get(relative).cloned();

            if !old_hash.is_some_and(|old_hash| old_hash == hash) {
                fs_err::write(path, data).map_err(|e| Error::FileWriteError {
                    path: path.to_path_buf(),
                    source: e,
                })?;

                updated_count.fetch_add(1, Ordering::Release);
            }

            Ok(Some((relative.to_string(), hash)))
        }

        let new_hashes = files
            .into_par_iter()
            .filter_map(|(path, data)| {
                try_write_file(&root, &path, data, &hashes, &updated_count).transpose()
            })
            .collect::<Result<ahash::HashMap<String, Vec<u8>>, Error>>()?;

        fs_err::write(
            &managed_files_path,
            compress(&bitcode::encode(&new_hashes), Compression::best()),
        )
        .map_err(|e| Error::ManagedFileWriteError {
            path: managed_files_path,
            source: e,
        })?;

        let updated_count = updated_count.load(Ordering::Acquire);

        let gone_files = hashes
            .keys()
            .filter(|k| !new_hashes.contains_key(&**k))
            .map(|path| root.join(path))
            .filter(|p| p.exists())
            .collect::<Vec<_>>();

        let cleaned_count = gone_files.len();

        trash::delete_all(gone_files).map_err(|e| Error::CleanupError { source: e })?;

        trash::delete(&managed_files_backup_path).map_err(|e| {
            Error::ManagedFileBackupDeleteError {
                path: managed_files_backup_path,
                source: e,
            }
        })?;

        debug!(
            updated_files = updated_count,
            skipped_files = total_to_write - updated_count,
            cleaned_files = cleaned_count,
            "Output flushed successfully"
        );
        Ok(())
    }
}

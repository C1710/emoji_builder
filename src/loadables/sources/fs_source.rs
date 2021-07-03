use std::path::{PathBuf, Path};
use crate::loadables::sources::LoadableSource;
use std::io::{BufReader, Read};
use std::fs::File;
use std::option::Option::None;
use itertools::Itertools;

#[derive(Clone, Debug)]
pub struct FsSource {
    base_dir: PathBuf,
    base_file: Option<PathBuf>
}

impl LoadableSource for FsSource {
    type Error = std::io::Error;

    fn request(&self, path: &Path) -> Result<Box<dyn Read>, Self::Error> {
        let normalized = self.relate_path(path);
        let file = File::open(normalized)?;
        Ok(Box::new(BufReader::new(file)))
    }

    fn root(&self) -> &Path {
        &self.base_dir
    }

    fn root_file(&self) -> Option<&Path> {
        self.base_file.as_deref()
    }

    fn request_source(&self, path: &Path) -> Result<Self, Self::Error> {
        let source = Self::new(path.to_path_buf())
            .unwrap_or_else(|| Self::new_with_dir(self.base_dir.clone(), Some(path.to_path_buf())));
        Ok(source)
    }

    fn contents(&self) -> Result<Vec<Self>, Self::Error> {
        let read_dir = std::fs::read_dir(&self.base_dir)?;
        Ok(
            read_dir.filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .map(|entry| if entry.is_dir() {
                    Self {
                        base_dir: entry,
                        base_file: None
                    }
                } else {
                    Self {
                        base_dir: self.base_dir.clone(),
                        base_file: Some(entry)
                    }
                })
                .collect_vec()
        )
    }
}


impl FsSource {
    fn relate_path(&self, target_path: &Path) -> PathBuf {
        // We use has_root here instead of is_absolute since otherwise
        // \file would be equal to .\file on Windows, which does not seem to be the usual expected
        // behavior.
        if !target_path.has_root() {
            // In this case, we assume, the path is relative in which case, we'll append it to the
            // root_dir and canonicalize it
            self.base_dir.join(&target_path)
        } else {
            target_path.to_path_buf()
        }
    }

    pub fn new(base_file: PathBuf) -> Option<Self> {
        let base_dir = base_file.parent()?.to_path_buf();
        Some(
            Self {
                base_dir,
                base_file: Some(base_file)
            }
        )
    }

    pub fn new_with_dir(base_dir: PathBuf, base_file: Option<PathBuf>) -> Self {
        Self {
            base_dir,
            base_file
        }
    }
}
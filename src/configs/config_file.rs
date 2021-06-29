use std::path::{PathBuf, Path};
use crate::packs::pack_files::EmojiPackFile;
use std::collections::HashMap;
use crate::loadable::{Loadable, LoadingError, LoadableImpl, normalize_paths, ResultAnyway};
use serde::Deserialize;
use crate::configs::config::PackConfig;
use itertools::{Itertools, Either};
use std::convert::TryFrom;
use std::io::{Read, BufReader};

#[derive(Deserialize)]
pub struct PackConfigFile {
    output_path: Option<PathBuf>,
    output_name: Option<String>,
    build_path: Option<PathBuf>,
    packs: Vec<PathBuf>,
    config: HashMap<String, String>
}

impl PackConfigFile {
    fn normalize_paths(&mut self, root_dir: &Path) {
        normalize_paths(&mut self.packs, root_dir);
    }

    pub fn load(self) -> ResultAnyway<PackConfig, LoadingError> {
        let (packs, errors): (Vec<_>, Vec<_>) = self.packs.into_iter()
            .map(|pack_path| EmojiPackFile::from_file(&pack_path))
            .map(crate::packs::pack::EmojiPack::try_from)
            .partition_map(|pack_result| match pack_result {
                Ok(pack) => Either::Left(pack),
                Err(err) => Either::Right(err)
            });

        let config = PackConfig {
            output_path: self.output_path,
            output_name: self.output_name,
            build_path: self.build_path,
            packs,
            config: self.config
        };
        if errors.is_empty() {
            Ok(config)
        } else {
            Err((config, errors.into()))
        }
    }
}

impl Loadable for PackConfigFile {
    fn from_file(file: &Path) -> Result<Self, LoadingError> {
        let reader = std::fs::File::open(file)?;
        let reader = BufReader::new(reader);
        let result = Self::from_reader(reader);
        if let Ok(mut config) = result {
            if let Some(parent) = file.parent() {
                config.normalize_paths(parent);
            }
            Ok(config)
        } else {
            result
        }
    }

    fn from_reader<R>(reader: R) -> Result<Self, LoadingError> where R: Read {
        Self::from_reader_impl(reader)
    }
}
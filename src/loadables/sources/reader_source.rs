use std::io::Read;
use crate::loadables::sources::LoadableSource;
use std::path::{Path, PathBuf};
use crate::loadables::NoError;
use std::fmt::{Debug, Formatter};

pub struct ReaderSource<R>
    where R: Read + Sized + Clone + Send + Sync {
    reader: R
}

impl<R> From<R> for ReaderSource<R>
    where R: Read + Sized + Clone + Send + Sync {
    fn from(reader: R) -> Self {
        Self {
            reader
        }
    }
}

impl<R> Debug for ReaderSource<R>
    where R: Read + Sized + Clone + Send + Sync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Reader Source").finish()
    }
}

lazy_static! {
    static ref EMPTY_PATH: PathBuf = PathBuf::new();
}

impl<R: 'static> LoadableSource for ReaderSource<R>
    where R: Read + Sized + Clone + Send + Sync {
    type Error = NoError;

    fn request(&self, path: &Path) -> Result<Box<dyn Read>, Self::Error> {
        if path != *EMPTY_PATH {
            Err(NoError {})
        } else {
            Ok(Box::new(self.reader.clone()))
        }
    }

    fn root(&self) -> &Path {
        &*EMPTY_PATH
    }

    fn request_source(&self, _: &Path) -> Result<Self, Self::Error> {
        Err(NoError {})
    }

    fn contents(&self) -> Result<Vec<Self>, Self::Error> {
        Err(NoError {})
    }

    fn request_root_file(&self) -> Result<Box<dyn Read>, Self::Error> {
        self.request(&*EMPTY_PATH)
    }
}
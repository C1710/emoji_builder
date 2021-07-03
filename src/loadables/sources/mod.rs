use std::error::Error;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::fmt::Debug;

pub mod fs_source;
pub mod reader_source;

pub trait LoadableSource: Debug + Send + Sync + Sized {
    type Error: Error + Sized;
    
    // TODO: This is the source (e.g. a file source or ZIP-file, etc.), where we can request data from
    // TODO: implement a request-function that can take a Path(?) and return a reader over that path
    // TODO: Maybe make it &mut self?
    fn request(&self, path: &Path) -> Result<Box<dyn Read>, Self::Error>;

    fn root(&self) -> &Path;
    
    fn root_file(&self) -> Option<&Path> {
        None
    }

    /// Returns a new source instead of a reader.
    /// This is useful for recursive Loadables.
    fn request_source(&self, path: &Path) -> Result<Self, Self::Error>;
    
    fn contents(&self) -> Result<Vec<Self>, Self::Error>;
    
    fn request_root_file(&self) -> Result<Box<dyn Read>, Self::Error> {
        let empty_file = PathBuf::new();
        let base_file = self.root_file().unwrap_or_else(|| &empty_file);
        
        self.request(base_file)
    }
}
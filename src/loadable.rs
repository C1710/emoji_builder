use std::path::{Path, PathBuf};
use std::io::{Error, BufReader, Read};
use crate::emoji::EmojiError;
use crate::emoji_tables::ExpansionError;
use serde::Deserialize;
use std::fs::File;
use std::fmt::{Debug, Formatter, Display};
use serde::de::DeserializeOwned;



pub trait Loadable: Sized {
    fn from_file(file: &Path) -> Result<Self, LoadingError> {
        let reader = std::fs::File::open(file)?;
        let reader = BufReader::new(reader);
        Self::from_reader(reader)
    }

    fn from_reader<R>(reader: R) -> Result<Self, LoadingError>
        where R: Read;
}

pub trait LoadableImpl: Sized {
    fn from_file_impl(file: &Path) -> Result<Self, LoadingError>;

    fn from_reader_impl<R>(reader: R) -> Result<Self, LoadingError>
        where R: Read;
}

impl<T> LoadableImpl for T
    where T: DeserializeOwned + Sized {
    fn from_file_impl(file: &Path) -> Result<Self, LoadingError> {
        let deserializer = DeserializerFunction::for_file(file);
        let deserializer = deserializer.unwrap_or_else(|| {
            warn!("No appropriate Deserializer found for {:?}. Assuming {}",
                file,
                DEFAULT_EXTENSION
            );
            DeserializerFunction::default()
        });
        let reader = File::open(file)?;
        let reader = BufReader::new(reader);
        deserializer.deserialize(reader).map_err(|err| LoadingError::Serde(Box::new(err)))
    }

    fn from_reader_impl<R>(reader: R) -> Result<Self, LoadingError>
        where R: Read {
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        Self::deserialize(&mut deserializer).map_err(|err| LoadingError::Serde(Box::new(err)))
    }
}

struct DeserializerFunction<R, T> {
    extension: &'static str,
    function: fn(R) -> Result<T, SimplifiedError>
}

impl<'de, R, T> Debug for DeserializerFunction<R, T>
    where R: Read, T: Deserialize<'de> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("Deserialize {}", self.extension))
            .field(&core::any::type_name::<R>())
            .field(&core::any::type_name::<T>())
            .finish()
    }
}

struct SimplifiedError {
    debug: String,
    display: String
}

impl Debug for SimplifiedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.debug, f)
    }
}

impl Display for SimplifiedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.display, f)
    }
}

impl std::error::Error for SimplifiedError {}

impl<E> From<E> for SimplifiedError
    where E: serde::de::Error {
    fn from(err: E) -> Self {
        let debug = format!("{:?}", err);
        let display = format!("{}", err);
        Self {
            debug,
            display
        }
    }
}

pub const DEFAULT_EXTENSION: &str = "json";


macro_rules! new_deserializer {
    ($deserialize_function: path, $new_deserialize_function: ident) => {
        fn $new_deserialize_function(arg: R) -> Result<T, SimplifiedError> {
            $deserialize_function(arg).map_err(SimplifiedError::from)
        }
    };
}


impl<R, T> DeserializerFunction<R, T>
    where R: Read, T: serde::de::DeserializeOwned {
    pub fn deserialize(&self, reader: R) -> Result<T, SimplifiedError> {
        (self.function)(reader)
    }

    pub fn new(extension: &'static str, function: fn(R) -> Result<T, SimplifiedError>) -> Self {
        Self {
            extension,
            function
        }
    }

    pub fn for_file(file: &Path) -> Option<Self> {
        if let Some(extension) = file.extension() {
            Self::for_extension(extension.to_string_lossy().as_ref())
        } else {
            None
        }
    }

    // Unfortunately we need to use a macro here, as Rust doesn't support Decorators on regular
    // functions and closures do not seem to be an option
    new_deserializer!(serde_json::from_reader, from_reader_json);

    pub fn for_extension(extension: &str) -> Option<Self> {
        let extension = extension.to_lowercase();
        match extension.as_str() {
            "json" => Some(Self::new("json", Self::from_reader_json)),
            _ => None
        }
    }

}

impl<R, T> Default for DeserializerFunction<R, T>
    where R: std::io::Read, T: serde::de::DeserializeOwned {
    fn default() -> Self {
        Self::for_extension(DEFAULT_EXTENSION).unwrap()
    }
}


#[derive(Debug)]
pub enum LoadingError {
    Io(std::io::Error),
    Multiple(Vec<LoadingError>),
    Emoji(EmojiError),
    Serde(Box<dyn std::error::Error>),
    MissingParameter,
    #[cfg(feature = "online")]
    Reqwest(reqwest::Error)
}


impl From<std::io::Error> for LoadingError {
    fn from(err: Error) -> Self {
        Self::Io(err)
    }
}

impl From<EmojiError> for LoadingError {
    fn from(err: EmojiError) -> Self {
        Self::Emoji(err)
    }
}

impl<T> From<Vec<T>> for LoadingError
    where T: Into<LoadingError> {
    fn from(errs: Vec<T>) -> Self {
        if errs.len() != 1 {
            Self::Multiple(errs.into_iter().map(|err| err.into()).collect())
        } else {
            // We know that there is (exactly) one item, so this will not panic
            errs.into_iter().next().unwrap().into()
        }
    }
}

impl From<ExpansionError> for LoadingError {
    fn from(err: ExpansionError) -> Self {
        match err {
            ExpansionError::Io(err) => Self::Io(err),
            ExpansionError::Multiple(err) => err.into(),
            #[cfg(feature = "online")]
            ExpansionError::Reqwest(err) => Self::Reqwest(err)
        }
    }
}

impl<T> From<LoadingError> for (T, LoadingError)
    where T: Default {
    fn from(error: LoadingError) -> Self {
        (T::default(), error)
    }
}

pub type ResultAnyway<T, E> = Result<T, (T, E)>;

pub fn normalize_paths(target_paths: &mut Vec<PathBuf>, root_dir: &Path) {
    target_paths.iter_mut()
        .for_each(|path| normalize_path(path, root_dir));
}

fn normalize_path(target_path: &mut PathBuf, root_dir: &Path) {
    if !target_path.has_root() {
        // In this case, we assume, the path is relative in which case, we'll append it to the
        // root_dir and canonicalize it
        *target_path = root_dir.join(&target_path);
    }
}
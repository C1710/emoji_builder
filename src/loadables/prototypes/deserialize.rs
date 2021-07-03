use std::fmt::{Debug, Display, Formatter};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::loadables::loadable::LoadablePrototype;
use crate::loadables::prototype_error::PrototypeLoadingError;
use crate::loadables::sources::LoadableSource;
use serde::de::DeserializeOwned;

impl<D, S> LoadablePrototype<S> for D
    where D: DeserializeOwned + Send + Sync + Debug,
          S: LoadableSource {
    type Error = SerdeError;

    fn load_prototype(source: &S) -> Result<Self, PrototypeLoadingError<Self, S>> {
        let empty_path = PathBuf::new();
        let base_file = source.root_file().unwrap_or(&empty_path);
        let reader = source.request_root_file()
            .map_err(PrototypeLoadingError::Source)?;
        let deserializer = DeserializerFunction::for_file(base_file).unwrap_or_default();
        deserializer.deserialize(reader).map_err(PrototypeLoadingError::Prototype)
    }
}

#[derive(Debug)]
pub enum DeserializeError {
    Io(std::io::Error),
    Serde(SerdeError)
}

impl Display for DeserializeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserializeError::Io(error) => Display::fmt(error, f),
            DeserializeError::Serde(error) => Display::fmt(error, f)
        }
    }
}

impl std::error::Error for DeserializeError {
    // TODO: Find a way to implement this
    /*fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DeserializeError::Io(error) => Some(error),
            DeserializeError::Serde(error) => Some(error)
        }
    }*/
}

impl From<std::io::Error> for DeserializeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SerdeError> for DeserializeError {
    fn from(error: SerdeError) -> Self {
        Self::Serde(error)
    }
}


#[derive(Clone)]
pub struct SerdeError {
    debug: String,
    display: String
}

impl std::error::Error for SerdeError {}

impl Debug for SerdeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.debug, f)
    }
}

impl Display for SerdeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.display, f)
    }
}

impl<E> From<E> for SerdeError
    where E: serde::de::Error {
    fn from(error: E) -> Self {
        Self {
            debug: format!("{:?}", error),
            display: format!("{}", error)
        }
    }
}


struct DeserializerFunction<R, T> {
    extension: &'static str,
    function: fn(R) -> Result<T, SerdeError>
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

pub const DEFAULT_EXTENSION: &str = "json";


macro_rules! new_deserializer {
    ($deserialize_function: path, $new_deserialize_function: ident) => {
        fn $new_deserialize_function(arg: R) -> Result<T, SerdeError> {
            $deserialize_function(arg).map_err(SerdeError::from)
        }
    };
}


macro_rules! deserializer_for_extensions {
    ($matching_extension:expr, $($extension:literal $(| $pattern:pat)? => $function:path),+$(,)?) => {
        match $matching_extension {
            $(
                $extension $(| $pattern)* => Some(Self::new($extension, $function)),
            )*
            _ => None
        }
    };
}


impl<R, T> DeserializerFunction<R, T>
    where R: Read, T: serde::de::DeserializeOwned {
    pub fn deserialize(&self, reader: R) -> Result<T, SerdeError> {
        (self.function)(reader)
    }

    pub fn new(extension: &'static str, function: fn(R) -> Result<T, SerdeError>) -> Self {
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


    // Unfortunately we need to use a macro here, as Rust doesn't support Decorators ond regular
    // functions and closures do not seem to be an option
    new_deserializer!(serde_json::from_reader, from_reader_json);
    new_deserializer!(serde_yaml::from_reader, from_reader_yaml);

    pub fn for_extension(extension: &str) -> Option<Self> {
        // TODO: Check if this is a mistake
        // For some reason, extension is recognized as unused...
        #[allow(unused)]
        let extension = extension.to_lowercase();
        deserializer_for_extensions!(extension.as_str(),
            "json" => Self::from_reader_json,
            "yaml" | "yml" => Self::from_reader_yaml,
            "test" => Self::from_reader_json,
        )
    }

}

impl<R, T> Default for DeserializerFunction<R, T>
    where R: std::io::Read, T: serde::de::DeserializeOwned {
    fn default() -> Self {
        Self::for_extension(DEFAULT_EXTENSION).unwrap()
    }
}
//! Android stubs for desktop-only libraries.

pub mod rfd {
    use std::path::{Path, PathBuf};

    #[derive(Default, Clone, Debug)]
    pub struct AsyncFileDialog {
        pub(crate) file_name: Option<String>,
        pub(crate) title: Option<String>,
    }

    impl AsyncFileDialog {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn add_filter(self, _name: impl Into<String>, _ext: &[impl AsRef<str>]) -> Self {
            self
        }

        pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
            self.file_name = Some(name.into());
            self
        }

        pub fn set_title(mut self, title: impl Into<String>) -> Self {
            self.title = Some(title.into());
            self
        }

        pub fn set_directory(self, _path: impl Into<PathBuf>) -> Self {
            self
        }

        pub async fn pick_file(self) -> Option<FileHandle> {
            None
        }

        pub async fn pick_files(self) -> Option<Vec<FileHandle>> {
            None
        }

        pub async fn pick_folder(self) -> Option<FileHandle> {
            None
        }

        pub async fn save_file(self) -> Option<FileHandle> {
            None
        }
    }

    #[derive(Clone, Debug)]
    pub struct FileHandle {
        pub(crate) path: PathBuf,
    }

    impl FileHandle {
        pub fn wrap(path: PathBuf) -> Self {
            Self { path }
        }

        pub fn path(&self) -> &Path {
            &self.path
        }

        pub fn file_name(&self) -> String {
            self.path
                .file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default()
        }

        pub async fn read(&self) -> Vec<u8> {
            std::fs::read(&self.path).unwrap_or_default()
        }

        pub async fn write(&self, data: &[u8]) -> std::io::Result<()> {
            std::fs::write(&self.path, data)
        }
    }
}

pub mod arboard {
    use std::borrow::Cow;
    use std::fmt;
    use std::path::Path;

    #[derive(Debug)]
    pub enum Error {
        ContentNotAvailable,
        ClipboardNotSupported,
        ClipboardOccupied,
        ConversionFailure,
        Unknown { description: String },
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::ContentNotAvailable => write!(f, "Content not available"),
                Self::ClipboardNotSupported => write!(f, "Clipboard not supported"),
                Self::ClipboardOccupied => write!(f, "Clipboard occupied"),
                Self::ConversionFailure => write!(f, "Conversion failure"),
                Self::Unknown { description } => write!(f, "Unknown error: {description}"),
            }
        }
    }

    impl std::error::Error for Error {}

    #[derive(Clone, Debug)]
    pub struct ImageData<'a> {
        pub width: usize,
        pub height: usize,
        pub bytes: Cow<'a, [u8]>,
    }

    pub struct Clipboard;

    impl Clipboard {
        pub fn new() -> Result<Self, Error> {
            Ok(Self)
        }

        pub fn get_text(&mut self) -> Result<String, Error> {
            Err(Error::ContentNotAvailable)
        }

        pub fn set_text<'a, T: Into<Cow<'a, str>>>(&mut self, _text: T) -> Result<(), Error> {
            Ok(())
        }

        pub fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
            Err(Error::ContentNotAvailable)
        }

        pub fn set_image(&mut self, _img: ImageData) -> Result<(), Error> {
            Ok(())
        }

        pub fn clear(&mut self) -> Result<(), Error> {
            Ok(())
        }

        pub fn set(&mut self) -> SetBuilder {
            SetBuilder
        }
    }

    pub struct SetBuilder;

    impl SetBuilder {
        pub fn text<'a, T: Into<Cow<'a, str>>>(self, _text: T) -> Result<(), Error> {
            Ok(())
        }

        pub fn file_list(self, _files: &[impl AsRef<Path>]) -> Result<(), Error> {
            Ok(())
        }
    }
}

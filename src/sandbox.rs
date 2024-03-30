use std::io::Read;
use std::path::PathBuf;
use std::{cell::RefCell, collections::HashMap};

use comemo::Prehashed;
use flate2::read::GzDecoder;
use js_sys::Uint8Array;
use tar::Archive;
use typst::diag::{eco_format, FileError, FileResult, PackageError, PackageResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::package::PackageSpec;
use typst::{
    syntax::{FileId, Source},
    text::{Font, FontBook},
    Library,
};
use web_sys::{console, XmlHttpRequest, XmlHttpRequestResponseType};

#[derive(Clone)]
struct FileEntry {
    bytes: Bytes,
    /// This field is filled on demand.
    source: Option<Source>,
}

impl FileEntry {
    fn source(&mut self, id: FileId) -> FileResult<Source> {
        // Fallibe `get_or_insert`.
        let source = if let Some(source) = &self.source {
            source
        } else {
            let contents = std::str::from_utf8(&self.bytes).map_err(|_| FileError::InvalidUtf8)?;
            // Defuse the BOM!
            let contents = contents.trim_start_matches('\u{feff}');
            let source = Source::new(id, contents.into());
            self.source.insert(source)
        };
        Ok(source.clone())
    }
}
pub struct Sandbox {
    library: Prehashed<Library>,
    book: Prehashed<FontBook>,
    fonts: Vec<Font>,

    files: RefCell<HashMap<FileId, FileEntry>>,
    package_files: RefCell<HashMap<(PackageSpec, PathBuf), Vec<u8>>>,
}

impl Sandbox {
    pub fn new() -> Self {
        let fonts = Vec::new();

        Self {
            library: Prehashed::new(Library::default()),
            book: Prehashed::new(FontBook::from_fonts(&fonts)),
            fonts,

            // cache_directory: todo!(),
            // http: todo!(),
            files: RefCell::new(HashMap::new()),
            // packages: RefCell::new(HashMap::new()),
            // files: HashMap::new(),
            package_files: RefCell::new(HashMap::new()),
        }
    }

    pub fn set_fonts(&mut self, font_files: Vec<&[u8]>) {
        let fonts = Self::parse_fonts(font_files);
        self.book = Prehashed::new(FontBook::from_fonts(&fonts));
        self.fonts = fonts;
    }

    pub fn with_source(&self, source: String) -> WithSource<'_> {
        WithSource {
            sandbox: self,
            source: make_source(source),
            time: get_time(),
        }
    }

    fn parse_fonts(font_files: Vec<&[u8]>) -> Vec<Font> {
        font_files
            .into_iter()
            .enumerate()
            .flat_map(|(idx, entry)| {
                let face_count = ttf_parser::fonts_in_collection(entry).unwrap_or(1);
                (0..face_count).map(move |face| {
                    Font::new(Bytes::from(entry), face)
                        .unwrap_or_else(|| panic!("failed to load font {idx}"))
                })
            })
            .collect()
    }

    fn file(&self, id: FileId) -> FileResult<FileEntry> {
        if let Some(entry) = self.files.borrow().get(&id) {
            return Ok(entry.clone());
        }

        if let Some(package) = id.package() {
            self.load_package(package)?;

            let package_files = self.package_files.borrow();
            let file = package_files
                .get(&(package.clone(), id.vpath().as_rootless_path().to_path_buf()))
                .map(|x| FileEntry {
                    bytes: x[..].into(),
                    source: None,
                });
            if let Some(file) = file {
                self.files.borrow_mut().insert(id, file.clone());
                return Ok(file);
            }
        }
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }

    fn load_package(&self, package: &PackageSpec) -> PackageResult<()> {
        if self
            .package_files
            .borrow()
            .contains_key(&(package.clone(), PathBuf::from("typst.toml")))
        {
            return Ok(());
        }

        let url = format!(
            "https://packages.typst.org/{}/{}-{}.tar.gz",
            package.namespace, package.name, package.version,
        );
        let req = XmlHttpRequest::new().unwrap();
        req.open_with_async("GET", &url, false).unwrap();
        req.set_response_type(XmlHttpRequestResponseType::Arraybuffer);
        req.send().map_err(|e| {
            console::log_1(&e);
            PackageError::NetworkFailed(Some(eco_format!(
                "Failed to send network request! Check console for more info."
            )))
        })?;

        // code != 2XX
        if req.status().unwrap_or_default() / 100 != 2 {
            return Err(PackageError::NetworkFailed(Some(eco_format!(
                "{} {}",
                req.status().unwrap_or_default(),
                req.status_text().unwrap_or_default()
            ))));
        }
        let tar_gz = Uint8Array::new(&req.response().unwrap()).to_vec();
        let tar = GzDecoder::new(&tar_gz[..]);
        let mut archive = Archive::new(tar);

        let malformed = |err: String| PackageError::MalformedArchive(Some(eco_format!("{}", err)));
        for e in archive.entries().unwrap() {
            match e {
                Ok(entry) => {
                    let path = entry.path().unwrap().to_path_buf();
                    let bytes = entry
                        .bytes()
                        .collect::<Result<_, _>>()
                        .map_err(|err| malformed(err.to_string()))?;
                    self.package_files
                        .borrow_mut()
                        .insert((package.clone(), path), bytes);
                }
                Err(err) => return Err(malformed(err.to_string())),
            }
        }
        Ok(())
    }
}

pub struct WithSource<'a> {
    sandbox: &'a Sandbox,
    source: Source,
    time: time::OffsetDateTime,
}

fn make_source(source: String) -> Source {
    Source::detached(source)
}

fn get_time() -> time::OffsetDateTime {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    time::OffsetDateTime::from_unix_timestamp(now).unwrap()
}

impl WithSource<'_> {
    pub fn main_source(&self) -> &Source {
        &self.source
    }
}

impl typst::World for WithSource<'_> {
    fn library(&self) -> &Prehashed<Library> {
        &self.sandbox.library
    }

    fn main(&self) -> Source {
        self.source.clone()
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            self.sandbox.file(id)?.source(id)
        }
    }

    fn book(&self) -> &Prehashed<FontBook> {
        &self.sandbox.book
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.sandbox.fonts.get(index).cloned()
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<typst::foundations::Bytes> {
        self.sandbox.file(id).map(|file| file.bytes.clone())
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        // We are in UTC
        let offset = offset.unwrap_or(0);
        let offset = time::UtcOffset::from_hms(offset.try_into().ok()?, 0, 0).ok()?;
        let time = self.time.checked_to_offset(offset)?;
        Some(Datetime::Date(time.date()))
    }
}

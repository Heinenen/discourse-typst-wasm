use std::cell::RefMut;
use std::{cell::RefCell, collections::HashMap};

use comemo::Prehashed;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::{
    syntax::{FileId, Source},
    text::{Font, FontBook},
    Library,
};

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

    // cache_directory: PathBuf,
    // http: todo!(),
    files: RefCell<HashMap<FileId, FileEntry>>,
}

impl Sandbox {
    pub fn new() -> Self {
        let fonts = fonts();

        Self {
            library: Prehashed::new(Library::default()),
            book: Prehashed::new(FontBook::from_fonts(&fonts)),
            fonts,

            // cache_directory: todo!(),
            // http: todo!(),
            files: RefCell::new(HashMap::new()),
        }
    }

    pub fn with_source(&self, source: String) -> WithSource<'_> {
        WithSource {
            sandbox: self,
            source: make_source(source),
            time: get_time(),
        }
    }

    fn file(&self, id: FileId) -> FileResult<RefMut<'_, FileEntry>> {
        if let Ok(entry) = RefMut::filter_map(self.files.borrow_mut(), |files| files.get_mut(&id)) {
            return Ok(entry);
        }

        // TODO handle packages
        return Err(FileError::NotFound(id.vpath().as_rootless_path().into()));
    }
}

pub struct WithSource<'a> {
    sandbox: &'a Sandbox,
    source: Source,
    time: time::OffsetDateTime,
}
fn fonts() -> Vec<Font> {
    vec![
        &include_bytes!("../fonts/DejaVuSansMono.ttf")[..],
        &include_bytes!("../fonts/LinLibertine_R.ttf")[..],
        &include_bytes!("../fonts/NewCM10-Regular.otf")[..],
        &include_bytes!("../fonts/Roboto-Regular.ttf")[..],
    ]
    .into_iter()
    .flat_map(|entry| {
        let face_count = ttf_parser::fonts_in_collection(entry).unwrap_or(1);
        (0..face_count).map(move |face| {
            Font::new(Bytes::from(entry), face).unwrap_or_else(|| panic!("failed to load font"))
        })
    })
    .collect()
}

fn make_source(source: String) -> Source {
    Source::detached(source)
}

fn get_time() -> time::OffsetDateTime {
    // time::OffsetDateTime::now_utc()
    time::OffsetDateTime::UNIX_EPOCH
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

use crate::errors::ErrResponse;
use std::{
    fs::{rename, File},
    io::{self, BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
};

/// adds a file to a zip, returns the name that was used inside the zip
#[tracing::instrument]
pub fn add_file_to_zip(
    file_path: &Path,
    file_name: &str,
    zip_path: &Path,
) -> Result<String, ErrResponse> {
    let file_name = deal_with_duplicates(zip_path, file_name.into())?;
    let file_name = file_name.display().to_string();

    // open existing zip
    let zip_file = File::options().read(true).write(true).open(zip_path)?;
    let zip_file = BufReadWrite::new(zip_file)?;
    let mut zip = zip::ZipWriter::new_append(zip_file)?;

    // read file
    let file = File::open(file_path)?;
    let mut file = BufReader::new(file);

    // add stem file to zip
    zip.start_file(&file_name, Default::default())?;
    std::io::copy(&mut file, &mut zip)?;
    zip.finish()?;

    Ok(file_name)
}

// appends "-copy" to a file
pub fn deal_with_duplicates(zip_path: &Path, mut path: PathBuf) -> Result<PathBuf, ErrResponse> {
    let file = File::options().read(true).open(zip_path)?;
    let reader = BufReader::new(file);
    let mut zip = zip::ZipArchive::new(reader)?;

    while zip.by_name(&path.display().to_string()).is_ok() {
        let mut name = path.file_stem().unwrap_or_default().to_os_string();
        name.push("-copy.");
        name.push(path.extension().unwrap_or_default());
        path = path.with_file_name(name);
    }

    Ok(path)
}

#[tracing::instrument]
pub fn remove_file_from_zip(
    file_name: &str,
    zip_path: &Path,
    artist_username: &str,
    song_slug: &str,
) -> Result<(), ErrResponse> {
    tracing::debug!("removing {file_name} from {}", zip_path.display());

    // NOTE: there's no way to just remove a file, so we have to create a new one
    // https://github.com/zip-rs/zip/issues/283

    // move existing zip to old.zip
    let old_zip_path = zip_path.with_file_name("old.zip");
    rename(&zip_path, &old_zip_path)?;

    // open old zip as reader
    let mut old_zip = {
        let file = File::open(&old_zip_path)?;
        let file = BufReader::new(file);
        zip::ZipArchive::new(file)?
    };

    // create new zip
    let mut new_zip = {
        let file = std::fs::File::create(&zip_path).unwrap();
        let file = BufWriter::new(file);
        zip::ZipWriter::new(file)
    };
    new_zip.add_directory(
        &format!("{} - {}", artist_username, song_slug),
        Default::default(),
    )?;

    // copy all files from old to new, except `file_name`
    for i in 0..old_zip.len() {
        let mut file = old_zip.by_index(i)?;
        tracing::debug!("Filename: {}", file.name());
        if file.name() != file_name {
            tracing::debug!("adding file to new zip: {}", file.name());
            new_zip.start_file(file.name(), Default::default())?;
            std::io::copy(&mut file, &mut new_zip)?;
        }
    }
    new_zip.finish()?;

    // it's not a big issue if we fail to remove old.zip
    let _ = std::fs::remove_file(old_zip_path);

    Ok(())
}

pub struct BufReadWrite {
    r: BufReader<File>,
    w: BufWriter<File>,
}

impl BufReadWrite {
    pub fn new(f: File) -> io::Result<Self> {
        Ok(BufReadWrite {
            r: BufReader::new(f.try_clone()?),
            w: BufWriter::new(f),
        })
    }
}

impl Read for BufReadWrite {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.r.read(buf)
    }
}

impl Write for BufReadWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

impl Seek for BufReadWrite {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.w.flush()?;
        // BufReader's implementation of Seek::seek() guarantees to immediately
        // seek the underlying handle even if the seek is within the buffer
        // bounds. This is why this `seek()` works for writing as well.
        self.r.seek(pos)
    }
}

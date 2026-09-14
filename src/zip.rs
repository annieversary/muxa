use crate::errors::ErrResponse;
use std::{
    fs::{rename, File},
    io::{self, BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
};
use zip::write::SimpleFileOptions;

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

    zip.start_file(&file_name, SimpleFileOptions::default())?;
    std::io::copy(&mut file, &mut zip)?;
    // BufWriter swallows errors when it flushes on drop, so it gets flushed by hand
    zip.finish()?.flush()?;

    Ok(file_name)
}

// appends "-copy" to a file
pub fn deal_with_duplicates(zip_path: &Path, mut path: PathBuf) -> Result<PathBuf, ErrResponse> {
    let file = File::options().read(true).open(zip_path)?;
    let reader = BufReader::new(file);
    let mut zip = zip::ZipArchive::new(reader)?;

    while zip.by_name(&path.display().to_string()).is_ok() {
        let mut name = path.file_stem().unwrap_or_default().to_os_string();
        name.push("-copy");
        // an extensionless name would otherwise come out with a trailing dot
        if let Some(ext) = path.extension() {
            name.push(".");
            name.push(ext);
        }
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

    // the new zip is built next to the old one under a unique name and only moved into
    // place once it is complete, so a failure part way through leaves the original alone
    let new_zip_path = zip_path.with_file_name(format!("new-{}.zip", uuid::Uuid::new_v4()));

    let res = write_zip_without(
        file_name,
        zip_path,
        &new_zip_path,
        artist_username,
        song_slug,
    );
    if res.is_err() {
        let _ = std::fs::remove_file(&new_zip_path);
        return res;
    }

    rename(&new_zip_path, zip_path)?;

    Ok(())
}

fn write_zip_without(
    file_name: &str,
    zip_path: &Path,
    new_zip_path: &Path,
    artist_username: &str,
    song_slug: &str,
) -> Result<(), ErrResponse> {
    let mut old_zip = {
        let file = File::open(zip_path)?;
        let file = BufReader::new(file);
        zip::ZipArchive::new(file)?
    };

    let mut new_zip = {
        let file = File::create(new_zip_path)?;
        let file = BufWriter::new(file);
        zip::ZipWriter::new(file)
    };

    // entries are copied over verbatim, so the directory entry only needs adding when the
    // old zip did not already have one; adding it twice is a duplicate-name error
    let dir_name = format!("{} - {}/", artist_username, song_slug);
    if old_zip.by_name(&dir_name).is_err() {
        new_zip.add_directory(dir_name, SimpleFileOptions::default())?;
    }

    for i in 0..old_zip.len() {
        let file = old_zip.by_index_raw(i)?;
        if file.name() != file_name {
            tracing::debug!("adding file to new zip: {}", file.name());
            new_zip.raw_copy_file(file)?;
        }
    }

    // BufWriter swallows errors when it flushes on drop, so it gets flushed by hand
    new_zip.finish()?.flush()?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("muxa-zip-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(zip_path: &Path) -> Vec<String> {
        let mut zip = zip::ZipArchive::new(File::open(zip_path).unwrap()).unwrap();
        (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect()
    }

    fn read_entry(zip_path: &Path, name: &str) -> Vec<u8> {
        let mut zip = zip::ZipArchive::new(File::open(zip_path).unwrap()).unwrap();
        let mut buf = Vec::new();
        zip.by_name(name).unwrap().read_to_end(&mut buf).unwrap();
        buf
    }

    fn make_zip(dir: &Path) -> PathBuf {
        let zip_path = dir.join("test.zip");
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        zip.add_directory("artist - song", SimpleFileOptions::default())
            .unwrap();
        for (name, body) in [("a.txt", "aaa"), ("b.txt", "bbb"), ("c.txt", "ccc")] {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        zip_path
    }

    #[test]
    fn remove_file_twice() {
        let dir = temp_dir();
        let zip_path = make_zip(&dir);

        remove_file_from_zip("a.txt", &zip_path, "artist", "song").unwrap();
        assert_eq!(names(&zip_path), ["artist - song/", "b.txt", "c.txt"]);

        remove_file_from_zip("b.txt", &zip_path, "artist", "song").unwrap();
        assert_eq!(names(&zip_path), ["artist - song/", "c.txt"]);
        assert_eq!(read_entry(&zip_path, "c.txt"), b"ccc");

        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_then_remove() {
        let dir = temp_dir();
        let zip_path = make_zip(&dir);
        let src = dir.join("d.txt");
        std::fs::write(&src, "ddd").unwrap();

        let name = add_file_to_zip(&src, "a.txt", &zip_path).unwrap();
        assert_eq!(name, "a-copy.txt");
        assert_eq!(read_entry(&zip_path, "a-copy.txt"), b"ddd");

        remove_file_from_zip("a.txt", &zip_path, "artist", "song").unwrap();
        assert_eq!(
            names(&zip_path),
            ["artist - song/", "b.txt", "c.txt", "a-copy.txt"]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

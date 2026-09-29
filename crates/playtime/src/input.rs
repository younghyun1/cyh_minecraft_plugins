//! Discover chronological rotations and stream bounded UTF-8 lines.
use crate::error::Error;
use chrono::{DateTime, Local, NaiveDate};
use flate2::read::MultiGzDecoder;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::PathBuf,
};

pub struct Input {
    pub path: PathBuf,
    pub date: NaiveDate,
    pub rotation: u32,
    latest: bool,
}

/// Parse archive dates; latest.log defaults to its modification date in local time.
fn describe(path: PathBuf, latest_date: Option<NaiveDate>) -> Result<Input, Error> {
    let name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Err(Error::Filename(path)),
    };
    if name == "latest.log" {
        let date = match latest_date {
            Some(date) => date,
            None => DateTime::<Local>::from(fs::metadata(&path)?.modified()?).date_naive(),
        };
        return Ok(Input {
            path,
            date,
            rotation: u32::MAX,
            latest: true,
        });
    }
    let stem = match name
        .strip_suffix(".log.gz")
        .or_else(|| name.strip_suffix(".log"))
    {
        Some(stem) => stem,
        None => return Err(Error::Filename(path)),
    };
    let date = stem
        .get(..10)
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok());
    let rotation = stem
        .get(10..)
        .and_then(|suffix| suffix.strip_prefix('-'))
        .and_then(|number| number.parse::<u32>().ok());
    match (date, rotation) {
        (Some(date), Some(rotation)) => Ok(Input {
            path,
            date,
            rotation,
            latest: false,
        }),
        _ => Err(Error::Filename(path)),
    }
}

/// Expand directories, remove repeated paths, and sort rotation numbers numerically.
pub fn discover(paths: &[PathBuf], latest_date: Option<NaiveDate>) -> Result<Vec<Input>, Error> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if entry.file_type()?.is_file()
                    && (name.ends_with(".log") || name.ends_with(".log.gz"))
                {
                    files.push(fs::canonicalize(entry.path())?);
                }
            }
        } else {
            files.push(fs::canonicalize(path)?);
        }
    }
    files.sort();
    files.dedup();
    let mut inputs: Vec<Input> = files
        .into_iter()
        .map(|path| describe(path, latest_date))
        .collect::<Result<_, _>>()?;
    inputs.sort_by(|a, b| {
        (a.date, a.latest, a.rotation, &a.path).cmp(&(b.date, b.latest, b.rotation, &b.path))
    });
    if inputs.is_empty() {
        return Err(Error::NoLogs);
    }
    Ok(inputs)
}

/// Snapshot the file length so an actively written latest.log has a finite boundary.
pub fn reader(input: &Input) -> Result<Box<dyn BufRead>, Error> {
    let file = File::open(&input.path)?;
    let len = file.metadata()?.len();
    let snapshot = file.take(len);
    if input.path.extension().is_some_and(|ext| ext == "gz") {
        Ok(Box::new(BufReader::with_capacity(
            64 * 1024,
            MultiGzDecoder::new(snapshot),
        )))
    } else {
        Ok(Box::new(BufReader::with_capacity(64 * 1024, snapshot)))
    }
}

/// Read at most 1 MiB per line; discard an unfinished final line during a live write.
pub fn line(reader: &mut dyn BufRead, buffer: &mut String) -> Result<bool, Error> {
    const LIMIT: u64 = 1024 * 1024;
    buffer.clear();
    let count = reader.take(LIMIT + 1).read_line(buffer)?;
    if count as u64 > LIMIT {
        return Err(Error::LineLimit);
    }
    Ok(count > 0 && buffer.ends_with('\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rotation identifiers are integers rather than lexical suffixes.
    #[test]
    fn archive_metadata() -> Result<(), Error> {
        let input = describe(PathBuf::from("2026-09-27-10.log.gz"), None)?;
        assert_eq!(input.rotation, 10);
        assert_eq!(input.date.to_string(), "2026-09-27");
        assert!(describe(PathBuf::from("2026-99-27-1.log"), None).is_err());
        Ok(())
    }

    /// A live file's partial final record must not create a premature event.
    #[test]
    fn partial_and_oversized_lines() -> Result<(), Error> {
        let mut buffer = String::new();
        assert!(!line(&mut &b"unfinished"[..], &mut buffer)?);
        assert!(line(&mut &b"complete\n"[..], &mut buffer)?);
        let large = vec![b'x'; 1024 * 1024 + 1];
        assert!(matches!(
            line(&mut large.as_slice(), &mut buffer),
            Err(Error::LineLimit)
        ));
        Ok(())
    }
}

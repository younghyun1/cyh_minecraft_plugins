use flate2::write::GzEncoder;
use flate2::Compression;
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::Mutex;
use tar::Builder;
use walkdir::WalkDir;

fn compress_directory(source: &Path, destination: &Path) -> io::Result<()> {
    let tar_gz_path = destination.join(".tar.gz");
    let tar_gz = File::create(tar_gz_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let tar = Mutex::new(Builder::new(enc));

    WalkDir::new(source)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>()
        .par_iter()
        .map(|entry| {
            let path = entry.path();
            let name = path.strip_prefix(source).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            if path.is_dir() {
                Ok((Some((name.to_path_buf(), path.to_path_buf())), None))
            } else {
                let mut file = File::open(path)?;
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)?;
                Ok((Some((name.to_path_buf(), path.to_path_buf())), Some(contents)))
            }
        })
        .try_for_each(|result: Result<_, io::Error>| {
            let (path_info, contents) = result?;
            if let Some((name, path)) = path_info {
                let mut tar = tar.lock().unwrap();
                if let Some(contents) = contents {
                    tar.append_data(&mut tar::Header::new_gnu(), &name, &*contents)?;
                } else {
                    tar.append_dir(&name, &path)?;
                }
            }
            Ok::<_, io::Error>(())
        })?;

    let enc = tar.into_inner().unwrap().into_inner()?;
    enc.finish()?;
    Ok(())
}

fn main() -> io::Result<()> {
    let source = Path::new("mcserver");
    let destination = Path::new("mcserver");

    compress_directory(&source, &destination)
}

use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use tar::Builder;

fn compress_folder(folder_path: &Path) -> io::Result<Vec<u8>> {
    let tar_gz = Vec::new();
    let encoder = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(encoder);

    fn add_dir_recursive<W: Write>(
        builder: &mut Builder<W>,
        path: &Path,
        base_path: &Path,
    ) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_name = entry_path.strip_prefix(base_path).unwrap();
            if entry_path.is_dir() {
                builder.append_dir(entry_name, &entry_path)?;
                add_dir_recursive(builder, &entry_path, base_path)?;
            } else {
                let mut file = File::open(&entry_path)?;
                builder.append_file(entry_name, &mut file)?;
            }
        }
        Ok(())
    }

    add_dir_recursive(&mut tar, folder_path, folder_path)?;
    tar.finish()?;
    let encoder = tar.into_inner()?;
    let compressed_data = encoder.finish()?;
    Ok(compressed_data)
}

fn save_compressed_data(data: Vec<u8>, backup_dir: &Path) -> io::Result<()> {
    let timestamp = Local::now().format("%Y-%m-%d_%H:%M:%S").to_string();
    let backup_file_path = backup_dir.join(format!("backup_{}.tar.gz", timestamp));
    let mut file = File::create(backup_file_path)?;
    file.write_all(&data)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let home_dir = dirs::home_dir().expect("Could not find home directory");
    let mcserver_path = home_dir.join("mcserver");
    let backup_dir = home_dir.join("backup");

    let compressed_data = compress_folder(&mcserver_path)?;
    save_compressed_data(compressed_data, &backup_dir)?;

    Ok(())
}

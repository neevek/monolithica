use anyhow::{bail, Result};
use std::collections::HashMap;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

pub struct AssetArhiver {}
impl AssetArhiver {
    pub fn create_archive(
        src_dir: &str,
        blob_path: &Path,
        blob_index_path: &Path,
        overwrite_existing: bool,
    ) -> Result<()> {
        Self::check_path(blob_path, overwrite_existing)?;
        Self::check_path(blob_index_path, overwrite_existing)?;

        let mut blob_file = File::create(blob_path)?;
        let mut blob_index_file = File::create(blob_index_path)?;
        let mut offset = 0u64;
        let src_dir = src_dir.strip_suffix('/').unwrap_or(src_dir);

        Self::concat_files(
            src_dir,
            src_dir,
            &mut blob_file,
            &mut blob_index_file,
            &mut offset,
        )?;

        Ok(())
    }

    fn concat_files(
        base_dir: &str,
        src_dir: &str,
        blob_file: &mut File,
        blob_index_file: &mut File,
        offset: &mut u64,
    ) -> Result<()> {
        let path_start_pos = base_dir.len() + 1;
        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;

            let path = entry.path();
            if path.is_file() {
                let mut file = File::open(&path)?;
                let file_len = file.metadata().unwrap().len();

                let mime = match mime_guess::from_path(&path).first() {
                    Some(mime) => mime.to_string(),
                    None => "".to_owned(),
                };

                writeln!(
                    blob_index_file,
                    "{}//{}//{}//{}",
                    &path.to_path_buf().to_str().unwrap()[path_start_pos..],
                    offset,
                    file_len,
                    mime
                )?;

                *offset += file_len;

                let mut buffer = [0u8; 8192];
                loop {
                    let bytes_read = file.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    blob_file.write_all(&buffer[..bytes_read])?;
                }
            } else if path.is_dir() {
                Self::concat_files(
                    base_dir,
                    path.to_str().unwrap(),
                    blob_file,
                    blob_index_file,
                    offset,
                )?;
            }
        }

        Ok(())
    }

    fn check_path(blob_path: &Path, overwrite_existing: bool) -> Result<()> {
        if blob_path.is_file() || blob_path.is_symlink() {
            if !overwrite_existing {
                tracing::error!("file already exists");
                bail!("file already exists");
            }
            std::fs::remove_file(blob_path)?;
        }

        if blob_path.exists() {
            tracing::error!("path exists but not a file");
            bail!("path exists but not a file");
        }

        Ok(())
    }
}

pub struct Asset {
    pub offset: u64,
    pub len: u64,
    pub mime: String,
}

type AssetPath<'a> = &'a str;
type AssetMap<'a> = HashMap<AssetPath<'a>, Asset>;

pub struct AssetIndexer<'a> {
    asset_map: AssetMap<'a>,
}

impl<'a> AssetIndexer<'a> {
    pub fn new(content: &'a str) -> Self {
        let mut asset_map = HashMap::new();
        for line in content.lines() {
            let fields: Vec<&str> = line.split("//").collect();

            let path = fields[0];
            let asset = Asset {
                offset: fields[1].parse().unwrap(),
                len: fields[2].parse().unwrap(),
                mime: fields[3].parse().unwrap(),
            };

            tracing::debug!("asset: {path}");

            asset_map.insert(path, asset);
        }

        Self { asset_map }
    }

    pub fn locate_asset(&self, path: &str) -> Option<&Asset> {
        Some(self.asset_map.get(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let archive_file = Path::new("target/test.blob");
        let archive_file_index = Path::new("target/test.blob.idx");
        AssetArhiver::create_archive("target", archive_file, archive_file_index, true).unwrap();

        let mut file = File::open(archive_file_index).unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();

        let indexer = AssetIndexer::new(&s);
        let asset = indexer.locate_asset(".rustc_info.json");

        assert!(asset.is_some());
        assert!(asset.unwrap().len > 0);
        assert!(asset.unwrap().mime == "application/json");
    }
}

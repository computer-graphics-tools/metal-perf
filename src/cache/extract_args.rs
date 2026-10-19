use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use clap::Args;
use regex::bytes::Regex;

use super::Error;

const GPU_MACH_O_MAGIC: &str = r"(?-u)\xCF\xFA\xED\xFE\x13\x00\x00\x01";

#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// Substring of the kernel name; repeat for several kernels
    #[arg(long = "name", required = true)]
    names: Vec<String>,
    /// Substring that must also appear in the binary, such as a source path or build hash
    #[arg(long)]
    path: Option<String>,
    /// Keep only binaries that carry a DWARF line table
    #[arg(long)]
    require_debug: bool,
    /// Metal cache directory [default: the largest one under $(getconf DARWIN_USER_CACHE_DIR)com.apple.metal]
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Output directory
    #[arg(long)]
    out: PathBuf,
}

impl ExtractArgs {
    pub fn run(&self) -> Result<(), Error> {
        let cache = match &self.cache {
            Some(cache) => cache.clone(),
            None => default_cache()?,
        };
        fs::create_dir_all(&self.out).map_err(|source| Error::Write {
            path: self.out.clone(),
            source,
        })?;
        let magic = Regex::new(GPU_MACH_O_MAGIC)?;
        let names = self
            .names
            .iter()
            .map(|name| Ok((name.as_str(), Regex::new(&regex::escape(name))?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let path = self.path.as_deref().map(regex::escape).map(|path| Regex::new(&path)).transpose()?;
        let debug_line = Regex::new("__debug_line")?;

        let mut written = 0;
        for data_path in function_archives(&cache)? {
            let data = fs::read(&data_path).map_err(|source| Error::Read {
                path: data_path.clone(),
                source,
            })?;
            let archive = data_path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let starts: Vec<usize> = magic.find_iter(&data).map(|found| found.start()).collect();
            for (index, &start) in starts.iter().enumerate() {
                let binary = &data[start..starts.get(index + 1).copied().unwrap_or(data.len())];
                if path.as_ref().is_some_and(|path| !path.is_match(binary))
                    || (self.require_debug && !debug_line.is_match(binary))
                {
                    continue;
                }
                let Some((name, _)) = names.iter().find(|(_, pattern)| pattern.is_match(binary)) else {
                    continue;
                };
                let target = self.out.join(format!("{}__{archive}_{start:x}.bin", file_safe(name)));
                fs::write(&target, binary).map_err(|source| Error::Write {
                    path: target.clone(),
                    source,
                })?;
                println!("{} {}", target.display(), binary.len());
                written += 1;
            }
        }
        println!("cache {}: wrote {written} binaries", cache.display());
        Ok(())
    }
}

fn default_cache() -> Result<PathBuf, Error> {
    let output = Command::new("getconf").arg("DARWIN_USER_CACHE_DIR").output().map_err(Error::CacheDirectory)?;
    let root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("com.apple.metal");
    let entries = fs::read_dir(&root).map_err(|_| Error::NoCache(root.clone()))?;
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_device_cache = name.split_once('_').is_some_and(|(left, right)| {
            [left, right].iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        });
        if !is_device_cache {
            continue;
        }
        let size = function_archives(&entry.path())?
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        if best.as_ref().is_none_or(|(largest, _)| size > *largest) {
            best = Some((size, entry.path()));
        }
    }
    best.map(|(_, path)| path).ok_or(Error::NoCache(root))
}

fn function_archives(cache: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = fs::read_dir(cache).map_err(|source| Error::Read {
        path: cache.to_owned(),
        source,
    })?;
    let mut archives: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy())
                .is_some_and(|name| name.starts_with("functions") && name.ends_with(".data"))
        })
        .collect();
    archives.sort();
    Ok(archives)
}

fn file_safe(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || "_.-".contains(character) {
                character
            } else {
                '_'
            }
        })
        .take(80)
        .collect()
}

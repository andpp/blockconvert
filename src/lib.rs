#[macro_use]
extern crate lazy_static;

pub mod certstream;
pub mod dns_lookup;
pub mod doh;
pub mod domain_filter;
pub mod list_downloader;
pub mod passive_dns;
pub mod validator;

pub mod blockconvert;

pub use blockconvert::{BlockConvert, BlockConvertBuilder};

pub use validator::Domain;

use serde::*;

use async_std::fs::OpenOptions;
use async_std::io::BufWriter;
use async_std::prelude::*;

lazy_static! {
    static ref DOMAIN_REGEX: regex::Regex =
        regex::Regex::new("(?:[0-9A-Za-z-]+[.])+[0-9A-Za-z-]+").unwrap();
}

lazy_static! {
    static ref IP_REGEX: regex::Regex =
        regex::Regex::new("[12]?[0-9]{0,2}[.][12]?[0-9]{0,2}[.][12]?[0-9]{0,2}[.][12]?[0-9]{0,2}")
            .unwrap();
}

pub const EXTRACTED_DOMAINS_DIR: &str = "extracted";

pub const MAX_AGE: u64 = 7 * 86400;

pub fn get_blocked_domain_path() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from("output");
    path.push("blocked_domains.txt");
    path
}

pub fn get_allowed_domain_path() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from("output");
    path.push("allowed_domains.txt");
    path
}

pub fn get_blocked_ips_path() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from("output");
    path.push("blocked_ips.txt");
    path
}

pub fn get_allowed_ips_path() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from("output");
    path.push("allowed_ips.txt");
    path
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub struct FilterListRecord {
    pub name: String,
    pub url: String,
    pub author: String,
    pub license: String,
    pub expires: u64,
    pub list_type: FilterListType,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Ord)]
pub enum FilterListType {
    Adblock,
    DomainBlocklist,
    DomainAllowlist,
    IPBlocklist,
    IPAllowlist,
    RegexAllowlist,
    RegexBlocklist,
    Hostfile,
    DNSRPZ,
    PrivacyBadger,
}

pub struct DirectoryDB {
    path: std::path::PathBuf,
    wtr: BufWriter<async_std::fs::File>,
}

impl DirectoryDB {
    pub async fn new(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let dir_path = std::path::PathBuf::from(path);
        let _ = async_std::fs::create_dir_all(&dir_path).await;

        let mut path = std::path::PathBuf::from(&dir_path);
        path.push(std::path::PathBuf::from(format!(
            "{:?}",
            chrono::Utc::today()
        )));
        let mut wtr = BufWriter::new(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .await?,
        );
        wtr.write_all(b"\n").await?;
        Ok(Self {
            path: dir_path,
            wtr,
        })
    }
    pub async fn read<I>(&self, mut handle_input: I) -> Result<(), Box<dyn std::error::Error>>
    where
        I: FnMut(&str),
    {
        let _ = async_std::fs::create_dir_all(&self.path).await;
        for entry in async_std::fs::read_dir(&self.path).await?.next().await {
            let entry = entry?;
            let metadata = entry.metadata().await?;
            if let Ok(modified) = metadata.modified().or_else(|_| metadata.created()) {
                let now = std::time::SystemTime::now();
                if let Ok(duration_since) = now.duration_since(modified) {
                    if duration_since.as_secs() < MAX_AGE {
                        if let Ok(file) = async_std::fs::File::open(entry.path()).await {
                            let mut file = async_std::io::BufReader::new(file);
                            let mut line = String::new();
                            while let Ok(len) = file.read_line(&mut line).await {
                                if len == 0 {
                                    break;
                                }
                                handle_input(&line);
                                line.clear();
                            }
                        }

                        continue;
                    }
                }
            }
            println!("Removing expired record");
        }
        Ok(())
    }

    pub async fn write_line(&mut self, line: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.wtr.write_all(line).await?;
        self.wtr.write_all(b"\n").await?;
        Ok(())
    }
    pub async fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.wtr.flush().await?;
        Ok(())
    }
}

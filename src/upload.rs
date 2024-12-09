use log::{info, error};
use serde::Deserialize;
use std::{collections::HashMap, io::Error, path::Path, process::{Command, ExitStatus}};
use crate::{config::*, get_msg};

#[derive(Deserialize, Debug, Clone)]
pub struct Upload {
    pub active: bool,
    pub timeout: f32,
    pub protocol: String,
    pub server: String,
    pub path: String,
    pub user: String,
    pub key: String,
    pub pubkey: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl Upload {
    pub fn run(&self, filepath: &str) -> Result<ExitStatus, std::io::Error> {
        let protocol = self.protocol.to_lowercase();
        let upload_path = format!("{}://{}{}/", protocol, self.server, self.path);
        let status: Result<ExitStatus, std::io::Error>;
        match protocol.as_str() {
            "sftp" | "scp" => {
                status = Command::new("curl")
                .arg("-m")
                .arg(format!("{}", &self.timeout))
                .arg("-u")
                .arg(format!("{}:", &self.user))
                .arg("--key")
                .arg(Path::new(&self.key))
                .arg("--pubkey")
                .arg(Path::new(&self.pubkey))
                .arg("-T")
                .arg(Path::new(filepath))
                .arg(upload_path)
                .status();
            }
            "https" | "http" => {
                status = Command::new("curl")
                .arg("-m")
                .arg(format!("{}", &self.timeout))
                .arg("-d")
                .arg(format!("@{}", filepath))
                .arg("-H")
                .arg("Content-Type: application/xml")
                .arg(upload_path)
                .status();
            }
            _ => {
                status = Err(Error::from_raw_os_error(22));
            }
        }
        status
    }
}

fn upload_check(upload: &Upload, file: &str, msg_config: &HashMap<String, HashMap<String, String>>, lang: &String) -> bool {
    let result: bool;
    match upload.run(file) {
        Ok(status) => {
            result = status.success();
            if result {
                let msg = get_msg(&msg_config, "upload_successful", lang);
                info!("{msg}: {file} ➔ {0} ✅", upload.server);
            } else {
                let msg = get_msg(&msg_config, "upload_failed", lang);
                error!("{msg}: {file} ➔ {0} ❌", upload.server);
            }
        }
        Err(err) => {
            result = false;
            let msg = get_msg(&msg_config, "upload_failed", lang);
            error!("{msg}: {file} ➔ {0} ❌ - {err}", upload.server);
        }
    }
    result
}

pub fn run_uploads(config: &Config, msg_config: &HashMap<String, HashMap<String, String>>, filepath: &String) -> Vec<Upload> {
    let uploads = &config.uploads;
    

    let path_split: Vec<&str> = filepath.split("/").collect();
    let filename = path_split.last().unwrap();
    let filename_split: Vec<&str> = filename.split("_").collect();

    let mut uploads_failed: Vec<Upload>= Vec::new();
    
    match filename_split.first() {
        Some(prefix) => {
            for upload in uploads {
                if upload.active {
                    let allowed: bool;
                    match (upload.include.is_empty(), upload.exclude.is_empty()) {
                        (true, true) => {
                            allowed = &prefix.to_string() != &config.filter.residue &&
                            &prefix.to_string() != &config.split.default;
                            }
                        (true, false) => {
                            allowed = !upload.exclude.contains(&prefix.to_string());
                        }
                        (false, true) => {
                            allowed = upload.include.contains(&prefix.to_string());
                            }
                        (false, false) => {
                            allowed = upload.include.contains(&prefix.to_string()) &&
                            !upload.exclude.contains(&prefix.to_string()); 
                        }
                    }
                    if allowed { 
                        if !upload_check(&upload, &filepath, &msg_config, &config.settings.lang) {
                            uploads_failed.push(upload.clone());  
                        }
                    };
                }
            }
        }
        None => {
            let msg1 = get_msg(&msg_config, "missing_prefix", &config.settings.lang);
            let msg2 = get_msg(&msg_config, "upload_aborted", &config.settings.lang);
            error!("{msg1}: {filename} - {msg2} ❌");
            for upload in uploads {
                uploads_failed.push(upload.clone());
            }
        }
    }
    uploads_failed
}
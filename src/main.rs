use std::env;
use std::fs::create_dir_all;
use std::path::Path;
use std::collections::HashMap;
use chrono::Local;
use quick_xml::events::Event;
use log::{error, info};
use log4rs;

use crate::transformations::transform::*;
use crate::config::*;
use utils::*;
use upload::*;

mod transformations;
mod config;
mod utils;
mod upload;

fn main() {
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(err) => {
            let msg = "Error: Can't import environment variables";
            eprintln!("{msg}: {err}");
            return;
        }
    }

    let log4rs_path: String;
    let config_path: String;
    let msg_config_path: String;
    
    match env::vars().find(|(k, _)| k == "LOG4RS") {
        None => {
            eprintln!("{}", "ERROR: No environment variable for log4rs configuration path.");
            return;
        }
        Some(entry) => log4rs_path = entry.1,
    }

    match env::vars().find(|(k, _)| k == "CONFIG") {
        None => {
            error!("{}", "No environment variable for configuration path.");
            return;
        }
        Some(entry) => config_path = entry.1,
    }

    match env::vars().find(|(k, _)| k == "MSG_CONFIG") {
        None => {
            error!("{}", "No environment variable for message configuration path.");
            return;
        }
        Some(entry) => msg_config_path = entry.1,
    }

    match log4rs::init_file(Path::new(&log4rs_path), Default::default()) {
        Ok(_) => (),
        Err(_) => {
            eprintln!("ERROR: Can't read log4rs configuration file.");
        }
    }

    let config = get_config(&config_path);
    let lang = &config.settings.lang;

    let msg_config: HashMap<String, HashMap<String, String>> = get_msg_config(&msg_config_path);

    let hist_dir = Path::new(&config.settings.dirs.history);
    let time_format = &config.settings.timeformats.history_folder;
    let storage_period = &config.settings.history_size;
    let timestamp: String = Local::now().format(&time_format).to_string();

    check_history(hist_dir, storage_period, time_format, &timestamp, &msg_config, lang);

    if config.settings.consistency_check {
        check_consistency(&config, &msg_config, lang);
    }

    match get_original(&config, &msg_config) {
        Ok(original_file) => {
            let str_path_to_original = format!("{}/{}", config.settings.dirs.original, original_file);
            let path_to_original: &Path = Path::new(&str_path_to_original);
            
            let current_history = &hist_dir.join(&timestamp);
            match create_dir_all(current_history) {
                Ok(_) => {
                    let msg = get_msg(&msg_config, "history_creation_successful", lang);
                    info!("{msg}: {timestamp}");
                }
                Err(err) => {
                    let msg = get_msg(&msg_config, "history_creation_failed", lang);
                    error!("{msg}: {timestamp} - {err}");
                    panic!();
                }
            }

            match transform(path_to_original, &config, &msg_config) {
                Ok(transformed) => {
                    let file_stem = Path::new(&original_file).file_stem().unwrap().to_str().unwrap();
                    let timestamp: String = chrono::Local::now()
                    .format(&config.settings.timeformats.files).to_string();                    
                    let revised: HashMap<String, &Vec<Event>> = transformed.iter()
                    .map(
                        |(group, event_list)| (
                            format!(
                                "{}/{}_{}_{}.xml",
                                config.settings.dirs.transformed,
                                group,
                                file_stem,
                                timestamp
                            ),
                            event_list
                        )
                    ).collect();

                    let mut uploads: HashMap<String, Vec<Upload>> = HashMap::new();
                    let mut archiving_failed: Vec<String> = Vec::new();
                    for (file_path_str, xml) in revised {
                        match write_xml(xml, &file_path_str, &msg_config, &config) {
                            Ok(_) => {
                                let path_split: Vec<&str> = file_path_str.split("/").collect();
                                let filename = path_split.last().unwrap();
                                let failed = run_uploads(&config, &msg_config, &file_path_str);
                                uploads.insert(filename.to_string(), failed.clone());
                                if failed.is_empty() {
                                    let file_path = Path::new(&file_path_str);
                                    let file_history = &current_history.join(&filename);
                                    match archive(file_path, &file_history) {
                                        Ok(_) => {
                                            let msg = get_msg(&msg_config, "archiving_successful", lang);
                                            info!("{msg}: {filename} ✅");
                                        }
                                        Err(err) => {
                                            let msg = get_msg(&msg_config, "archiving_failed", lang);
                                            error!("{msg}: {filename} ❌ - {err}");
                                            archiving_failed.push(filename.to_string());
                                        }
                                    }
                                } else {
                                    let msg = get_msg(&msg_config, "archiving_prevented", lang);
                                    error!("{msg}: {file_path_str}");   
                                }
                            }
                            Err(err) => {
                                let msg = get_msg(&msg_config, "archiving_prevented", lang);
                                error!("{msg}: {file_path_str} - {err}");
                            }
                        }
                    }

                    let uploads_failed: HashMap<String, Vec<Upload>> = uploads
                    .into_iter()
                    .filter(|(_k, v)| !v.is_empty())
                    .collect();

                    if !uploads_failed.is_empty() {
                        let list = uploads_failed
                        .into_iter().map(|(k, v)| format!("{}: {:?}", k, v)).collect::<Vec<String>>().join("\n");
                        let msg = get_msg(&msg_config, "upload_report", lang);
                        let upload_report = msg + ":\n\n" + &list;
                        send_mail(&config, &msg_config, upload_report);
                    }
                    if !archiving_failed.is_empty() {
                        let msg = get_msg(&msg_config, "archiving_report", lang);
                        let list = archiving_failed.join("\n");
                        let archiving_report = msg + ":\n\n" + &list;
                        send_mail(&config, &msg_config, archiving_report);
                    }

                    let original_history = &current_history.join(&original_file);
                    match archive(path_to_original, original_history) {
                        Ok(_) => {
                            let msg = get_msg(&msg_config, "archiving_successful", lang);
                            info!("{msg}: {original_file} ✅");
                        }
                        Err(err) => {
                            let msg = get_msg(&msg_config, "archiving_failed", lang);
                            let error_msg = format!("{msg}: {original_file} ❌ - {err}");
                            let addition = get_msg(&msg_config, "archiving_original_failed", lang);
                            error!("{error_msg}. {addition}");
                            send_mail(&config, &msg_config, error_msg + "\n\n" + &addition);
                            panic!();
                        }
                    }
                }
                Err(err) => {
                    let msg = get_msg(&msg_config, "transformation_failed", lang);
                    let error_msg = format!("{msg}: {original_file} - {err}");
                    let addition = format!(
                        "{}. {}.",
                        get_msg(&msg_config, "transforming_original_failed", lang),
                        get_msg(&msg_config, "process_cancelled", lang)
                    );
                    error!("{error_msg}. {addition}");
                    send_mail(&config, &msg_config, error_msg + "\n\n" + &addition);
                    panic!();
                }
            }
        }
        Err(err) => {
            let msg = get_msg(&msg_config, "reading_original_failed", lang);
            let error_msg = format!("{msg}: {err}.");
            let addition = format!("{}.", get_msg(&msg_config, "process_cancelled", lang));
            error!("{error_msg} {addition}");
            send_mail(&config, &msg_config, error_msg + "\n\n" + &addition);
            panic!();
        }
    }
}
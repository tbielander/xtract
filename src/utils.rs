use std::collections::hash_map::RandomState;
use std::env;
use std::fs::{self, DirEntry};
use std::fs::{read_dir, remove_dir_all};
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;
use std::collections::{HashMap, HashSet};
use chrono::NaiveDate;
use lettre::message::{header, Mailbox, Mailboxes, MessageBuilder};
use log::{error, info, warn};
use quick_xml::events::{Event, BytesStart, BytesText, BytesEnd};
use quick_xml::writer::Writer;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::config::*;

pub fn update_sliding_window(
    hist_dir: &Path,
    folder: &DirEntry,
    current_date: NaiveDate,
    storage_period: &usize,
    time_format: &String,
    msg_config: &HashMap<String, HashMap<String, String>>,
    lang: &String
) -> () {
    if let Some(folder_name) = folder
    .file_name()
    .to_str() {
        match NaiveDate::parse_from_str(
            folder_name,
            &time_format
        ) {
            Ok(folder_date) => {
                let diff = current_date - folder_date;
                if diff.num_days() > *storage_period as i64 {
                    match remove_dir_all(hist_dir.join(folder_name)) {
                        Ok(_) => {
                        let msg = get_msg(
                            &msg_config,
                            "history_cleared",
                            lang
                        );
                        info!("{msg}");
                    }
                        Err(err) => {
                            let msg = get_msg(
                                &msg_config,
                                "history_clearing_failed",
                                lang
                            );
                            error!("{msg}: {err}");
                        }
                    }
                }
            }
            Err(err) => {
                let msg = get_msg(
                    &msg_config,
                    "parse_date_from_folder_name_failed",
                    lang
                );
                error!("{}", format!("{}: {:?} - {}", msg, folder, err));
            }
        }
    }
}

pub fn check_history(
    hist_dir: &Path,
    storage_period: &usize,
    time_format: &String,
    timestamp: &String,
    msg_config: &HashMap<String, HashMap<String, String>>,
    lang: &String
) -> () {
    match NaiveDate::parse_from_str(&timestamp, &time_format) {
        Ok(current_date) => {
            match read_dir(hist_dir) {
                Ok(hist_folders) => {
                    for entry in hist_folders {
                        match &entry {
                            Ok(folder) => {
                                update_sliding_window(
                                    hist_dir,
                                    folder,
                                    current_date,
                                    storage_period,
                                    time_format,
                                    msg_config,
                                    lang
                                );
                            }
                            Err(err) => {
                                let msg = get_msg(&msg_config, "reading_dir_entry_failed", lang);
                                error!("{}", format!("{}: {:?} - {}", msg, entry, err));
                            }
                        }
                    }
                }
                Err(err) => {
                    let msg = get_msg(&msg_config, "reading_hist_dir_failed", lang);
                    error!("{}", format!("{}: {:?} - {}", msg, hist_dir, err));
                }
            }
        }
        Err(err) => {
            let msg = get_msg(&msg_config, "parse_date_from_timestamp_failed", lang);
            error!("{}", format!("{}: {:?} - {}", msg, hist_dir, err));
        }
    }
}

pub fn get_intersection(hs1: &HashSet<&String, RandomState>, hs2: &HashSet<&String, RandomState>) -> Vec<String> {
    let intersection: Vec<String> = hs1.intersection(&hs2)
    .into_iter()
    .map(|s| s.to_string())
    .collect();
    intersection
}

pub fn get_difference(hs1: &HashSet<&String, RandomState>, hs2: &HashSet<&String, RandomState>) -> Vec<String> {
    let difference: Vec<String> = hs1.difference(&hs2)
    .into_iter()
    .map(|s| s.to_string())
    .collect();
    difference
}

pub fn embed<'a>(value: String, path: String) -> Vec<Event<'a>> {
    let mut new_element_names: Vec<String> = Vec::new();
    let mut new_elements: Vec<Event<'_>> = Vec::new();
    let split_path= path.split("/");
    for name in split_path {
        new_element_names.push(name.to_string());
    }
    for element in &new_element_names {
        let start_tag = BytesStart::new( element.clone());
        new_elements.push(Event::Start(start_tag));
    }
    new_elements.push(Event::Text(BytesText::new(&value).into_owned()));
    new_element_names.reverse();
    for name in new_element_names {
        let end_tag = BytesEnd::new(name.clone());
        new_elements.push(Event::End(end_tag));
    }

    new_elements
}

pub fn format_warning(
    inconsistent_values: (&String, Vec<String>),
    msg_config: &HashMap<String, HashMap<String, String>>,
    lang: &String
) -> String {
    let (xml_element, conflicting_values) = inconsistent_values;

    let warning = [
        format!("  {}: {}", get_msg(&msg_config, "xml_element", lang), xml_element),
        format!("  {}: {}", get_msg(&msg_config, "values", lang), &conflicting_values.join(", ")),
    ].join("\n");
    
    warning
}

pub fn compile_warnings(
    inconsistencies: Vec<(&String, Vec<String>)>,
    msg_config: &HashMap<String, HashMap<String, String>>,
    lang: &String
) -> String {
    let compiled_warnings: Vec<String> = inconsistencies
    .into_iter()
    .map(|inconsistency| format_warning(inconsistency, msg_config, lang))
    .collect();

    compiled_warnings.join("\n  ---\n")
}

pub fn check_consistency(config: &Config, msg_config: &HashMap<String, HashMap<String, String>>, lang: &String) {
    let keys_allow_exact: HashSet<&String, RandomState> = HashSet::from_iter(config.filter.allowlist.exact.keys());
    let keys_allow_regex: HashSet<&String, RandomState> = HashSet::from_iter(config.filter.allowlist.regex.keys());
    let keys_allow = keys_allow_exact.union(&keys_allow_regex).cloned().collect();
    let keys_block_exact: HashSet<&String, RandomState> = HashSet::from_iter(config.filter.blocklist.exact.keys());
    let keys_block_regex: HashSet<&String, RandomState> = HashSet::from_iter(config.filter.blocklist.regex.keys());
    let keys_block = keys_block_exact.union(&keys_block_regex).cloned().collect();
    let inconsistent_filter = get_intersection(&keys_allow, &keys_block);
    let filter_consistency = inconsistent_filter.is_empty();

    if filter_consistency {
        let mut splitting_without_allowance: Vec<(&String, Vec<String>)> = Vec::new();
        let mut allowance_without_splitting: Vec<(&String, Vec<String>)> = Vec::new();
        let mut splitting_despite_blocking: Vec<(&String, Vec<String>)> = Vec::new();

        for (xml_path, _labeling) in &config.split.grouping {
            if config.split.grouping.contains_key(xml_path) {
                let splitting = HashSet::from_iter(
                    config.split.grouping
                    .get(xml_path)
                    .unwrap()
                    .keys()
                );
                if keys_allow.contains(xml_path) {
                    let allowed_exact: HashSet<&String> = HashSet::from_iter(
                        config.filter.allowlist.exact
                        .get(xml_path)
                        .unwrap()
                        .into_iter()
                    );
                    let allowed_regex: HashSet<&String> = HashSet::from_iter(
                        config.filter.allowlist.regex
                        .get(xml_path)
                        .unwrap()
                        .into_iter()
                    );
                    let allowed = allowed_exact.union(&allowed_regex).cloned().collect();
                    let split_but_not_allowed: Vec<String> = get_difference(&splitting, &allowed);
                    let allowed_but_not_split: Vec<String> = get_difference(&allowed_exact, &splitting);
                    if !split_but_not_allowed.is_empty() {
                        splitting_without_allowance.push((xml_path, split_but_not_allowed));
                    }
                    if !allowed_but_not_split.is_empty() {
                        allowance_without_splitting.push((xml_path, allowed_but_not_split))
                    }
                }
                if config.filter.blocklist.exact.contains_key(xml_path) {
                    let blocked = HashSet::from_iter(
                        config.filter.blocklist.exact
                        .get(xml_path)
                        .unwrap()
                        .into_iter()
                    );
                    let split_while_blocked: Vec<String> = get_intersection(&splitting, &blocked);
                    if !split_while_blocked.is_empty() {
                        splitting_despite_blocking.push((xml_path, split_while_blocked));
                    }
                }
            }
        }

        let mut collected_warnings: Vec<String> = Vec::new();

        if !splitting_without_allowance.is_empty() {
            let msg = get_msg(&msg_config, "splitting_without_allowance", lang);
            let warning = format!("• {}:\n\n{}\n", msg, compile_warnings(splitting_without_allowance, msg_config, lang));
            collected_warnings.push(warning);
        }
        if !splitting_despite_blocking.is_empty() {
            let msg = get_msg(&msg_config, "splitting_despite_blocking", lang);
            let warning = format!("• {}:\n\n{}\n", msg, compile_warnings(splitting_despite_blocking, msg_config, lang));
            collected_warnings.push(warning);
        }
        if !allowance_without_splitting.is_empty() {
            let msg = get_msg(&msg_config, "allowance_without_splitting", lang);
            let warning = format!("• {}:\n\n{}\n", msg, compile_warnings(allowance_without_splitting, msg_config, lang));
            collected_warnings.push(warning);
        }
        if !collected_warnings.is_empty() {
            let msg = get_msg(&msg_config, "filter_split_conflict", lang);
            let warnings = format!("{}:\n\n{}", msg, collected_warnings.join("\n"));
            warn!("{warnings}");
            if config.settings.inconsistency_notification {
                send_mail(config, msg_config, warnings);
            }
        }
    } else {
        let msg = get_msg(&msg_config, "allow_block_conflict", lang);
        let warning = format!("{}:\n  • {}", msg, inconsistent_filter.join("\n  • "));
        warn!("{warning}");
        if config.settings.inconsistency_notification {
            send_mail(config, msg_config, warning);
        }
    }
}

pub fn get_original(config: &Config, msg_config: &HashMap<String, HashMap<String, String>>) -> Result<String> {
    let original: String;
    let dir = Path::new(&config.settings.dirs.original);
    let lang = &config.settings.lang;
    
    match fs::read_dir(dir) {
        Ok(entries) => {
            let mut files:Vec<String> = vec![];
            for entry in entries {
                match entry {
                    Ok(file) => {
                        files.push(file.file_name().into_string().unwrap());
                    }
                    Err(err) => {
                        let msg = get_msg(&msg_config, "reading_dir_entry_failed", lang);
                        let error_msg = format!("{}: {:?} - {}", msg, dir, err);
                        error!("{error_msg}");
                        send_mail(config, msg_config, error_msg);
                        panic!();
                    }
                }
            }
            match files.len() {
                0 => {
                    let msg = get_msg(&msg_config, "missing_original", lang);
                    let error_msg = format!("{}: {:?}", msg, dir);
                    error!("{error_msg}");
                    send_mail(config, msg_config, error_msg);
                    panic!();
                }
                1 => original = files[0].clone(),
                _ => {
                    let msg = get_msg(&msg_config, "more_than_one_original", lang);
                    let error_msg = format!("{}: {:?}", msg, files);
                    error!("{error_msg}");
                    send_mail(config, msg_config, error_msg);
                    panic!();
                }
            }
        }
        Err(err) => {
            let msg = get_msg(&msg_config, "reading_original_dir_failed", lang);
            let error_msg = format!("{}: {:?} - {}", msg, dir, err);
            error!("{error_msg}");
            send_mail(config, msg_config, error_msg);
            panic!();
        }
    }
    
    Ok(original)
}

pub fn write_file(file_path: &str, contents: &str) -> Result<()> {
    let mut file = fs::File::create(file_path)?;
    file.write_all(contents.as_bytes())
}

pub fn write_xml(
    xml_events: &Vec<Event>,
    output_path: &str,
    msg_config: &HashMap<String, HashMap<String, String>>,
    config: &Config
) -> Result<()> {
    let lang = &config.settings.lang;
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer: Writer<&mut Vec<u8>> = Writer::new(&mut buffer);
    for event in xml_events {
        if let Err(err) = writer.write_event(event.clone()) {
            let msg = get_msg(&msg_config, "writing_event_failed", lang);
            error!("{}", format!("{}: {:?} - {}", msg, &event, err));
        }
    }

    match std::str::from_utf8(writer.into_inner()) {
        Ok(revised_xml) => {
            match write_file(output_path, revised_xml) {
                Ok(_) => {
                    let msg = get_msg(&msg_config, "file_written", lang);
                    info!("{}", format!("{}: {:?}", msg, Path::new(&output_path)));
                }
                Err(err) => {
                    let msg = get_msg(&msg_config, "writing_file_failed", lang);
                    let error_msg = format!("{}: {}", msg, err);
                    error!("{error_msg}");
                    send_mail(config, msg_config, error_msg);
                }
            }
        }
        Err(err) => {
            let msg = get_msg(&msg_config, "str_from_writer_failed", lang);
            let error_msg = format!("{}: {}", msg, err);
            error!("{error_msg}");
            send_mail(config, msg_config, error_msg);
        }
    } 
    Ok(())
}

pub fn superordinate(path1: &Vec<String>, path2: &Vec<&str>) -> Result<bool> {
    let matching = path2.iter().zip(path1).filter(|&(a, b)| a == b);
    Ok(matching.count() < path2.len())
}

pub fn get_config(path_str: &str) -> Config {
    let config_path: &Path = Path::new(path_str);
    let config_str = match fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(err) => {
            error!("Can't read configuration file {:?}: {}", config_path, err);
            panic!();
        }
    };

    let config: Config = match toml::from_str(&config_str) {
        Ok(c) =>  c,
        Err(err) => {
            error!("Unable to parse configuration {:?}: {}", config_path, err);
            panic!();
        }
    };
    config
}

pub fn get_msg_config(path_str: &str) -> HashMap<String, HashMap<String, String>> {
    let msg_config_path: &Path = Path::new(path_str);
    let msg_config_str = match fs::read_to_string(&msg_config_path) {
        Ok(s) => s,
        Err(err) => {
            let error_msg = format!("Can't read message configuration file {:?}: {}", msg_config_path, err);
            error!("{error_msg}");
            panic!();
        }
    };

    let msg_config: HashMap<String, HashMap<String, String>> = match toml::from_str(&msg_config_str) {
        Ok(c) =>  c,
        Err(err) => {
            let error_msg = format!("Unable to parse configuration {:?}: {}", msg_config_path, err);
            error!("{error_msg}");
            panic!();
        }
    };
    msg_config
}

pub fn get_msg(msg_config: &HashMap<String, HashMap<String, String>>, msg_key: &str, lang: &String) -> String {
    match msg_config
    .get(msg_key)
    .and_then(|map| map.get(lang)) {
        Some(msg) => msg.clone().to_string(),
        None => msg_key.to_string(),
    }
}

pub fn archive(
    file_path: &Path,
    archive_path: &Path
) -> Result<()>{
    let result = match fs::rename(file_path, archive_path) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    };
    result
}

pub fn send_mail(config: &Config, msg_config: &HashMap<String, HashMap<String, String>>, body: String) {
    let email_settings = &config.settings.email.message;
    let mailer_settings = &config.settings.email.mailer;

    let from: Mailbox;
    match email_settings.from.parse::<Mailbox>() {
        Ok(mbox) => from = mbox,
        Err(err) => {
            let msg = get_msg(&msg_config, "from_error", &config.settings.lang);
            error!("{msg}: {err}");
            return;
        }
    }

    let reply_to_header: header::ReplyTo;
    match email_settings.reply_to.join(",").parse::<Mailboxes>() {
        Ok(mboxes) => reply_to_header = mboxes.into(),
        Err(err) => {
            let msg = get_msg(&msg_config, "reply_to_error", &config.settings.lang);
            error!("{msg}: {err}");
            return;
        }
    }
    
    let to_header: header::To;
    match email_settings.to.join(",").parse::<Mailboxes>() {
        Ok(mboxes) => to_header = mboxes.into(),
        Err(err) => {
            let msg = get_msg(&msg_config, "to_error", &config.settings.lang);
            error!("{msg}: {err}");
            return;
        }
    }

    let email: Message;
    match MessageBuilder::new()
    .from(from)
    .mailbox(reply_to_header)
    .mailbox(to_header)
    .subject(email_settings.subject.to_string())
    .header(ContentType::TEXT_PLAIN)
    .body(body) {
        Ok(message) => email = message,
        Err(err) => {
            let msg = get_msg(&msg_config, "message_building_error", &config.settings.lang);
            error!("{msg}: {err}");
            return;
        }
    }
    
    let mailer: SmtpTransport;

    if config.settings.email.mailer.auth {
        let smtp_user: String;
        let smtp_pw: String;
        match env::vars().find(|(k, _)| k == "SMTP_USER") {
            Some(entry) => {
                smtp_user = entry.1;
                if smtp_user.trim().is_empty() {
                    let msg = get_msg(&msg_config, "missing_smtp_user", &config.settings.lang);
                    error!("{msg}");
                    return;
                }
                match env::vars().find(|(k, _)| k == "SMTP_PW") {
                    Some(entry) => {
                        smtp_pw = entry.1;
                        if smtp_pw.trim().is_empty() {
                            let msg = get_msg(&msg_config, "missing_smtp_pw", &config.settings.lang);
                            error!("{msg}");
                            return;
                        }
                    }
                    None => {
                        let msg = get_msg(&msg_config, "missing_smtp_pw", &config.settings.lang);
                        error!("{msg}");
                        return;
                    }
                }
            }
            None => {
                let msg = get_msg(&msg_config, "missing_smtp_user", &config.settings.lang);
                error!("{msg}");
                return;
            }
        }

        let creds = Credentials::new(smtp_user, smtp_pw);

        mailer = SmtpTransport::starttls_relay(&mailer_settings.smtp).unwrap()
        .port(mailer_settings.port)
        .credentials(creds)
        .build();

    } else {
        mailer = SmtpTransport::starttls_relay(&mailer_settings.smtp).unwrap()
        .port(mailer_settings.port)
        .build();
    }

    match mailer.send(&email) {
        Ok(_) => {
            let msg = get_msg(&msg_config, "send_email_successful", &config.settings.lang);
            info!("{msg}");
        }
        Err(err) => {
            let msg = get_msg(&msg_config, "send_email_failed", &config.settings.lang);
            error!("{msg}: {err}");
        }
    }
}

use std::fs;
use std::io::Result;
use std::collections::HashMap;
use std::path::Path;
use log::{error, info};
use quick_xml::events::{Event, BytesText};
use quick_xml::reader::Reader;
use regex::Regex;
use crate::transformations::transformer::*;
use crate::utils::*;
use crate::config::*;

pub fn transform<'a>(
    file_path: &Path,
    config: &Config,
    msg_config: &HashMap<String, HashMap<String, String>>
) -> Result<HashMap<String, Vec<Event<'a>>>> {
    let mut reader: Reader<std::io::BufReader<fs::File>> = Reader::from_file(file_path).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    let mut split_element: Vec<Event<'_>> = Vec::new();
    let lang = &config.settings.lang;

    let mut transformers: Vec<Transformer> = config.transformations.clone().into_iter()
    .map(|t| Transformer::new(t)).collect();
 
    let mut groups = config.split.grouping
    .values().cloned().collect::<Vec<HashMap<String, String>>>()
    .into_iter().map(|h| h.values().cloned().collect::<Vec<String>>())
    .into_iter().flatten().collect::<Vec<String>>();
    groups.sort_unstable();
    groups.dedup();

    groups.push(config.filter.residue.clone());
    groups.push(config.split.default.clone());

    let mut current_group: &String = &config.split.default;
    let mut current_path: Vec<String> = Vec::new();
    let mut splitting: HashMap<String, Vec<Event<'_>>> = groups.into_iter().map(|g| (g, Vec::new())).collect(); 
    let split_path: Vec<&str> = config.element.split("/").collect();
    let mut include: bool = true;
    let mut keep: bool = true;
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(e)) => {
                if config.split.declaration {
                    for xml_event_list in splitting.values_mut() {
                        xml_event_list.push(Event::Decl(e.clone().into_owned()));
                    }
                }
            }

            Ok(Event::Start(e)) => {
                current_path.push(String::from_utf8(e.name().as_ref().to_owned()).unwrap());
                let current_path_string = current_path.join("/");
                for t in &mut transformers {
                    if let Some(elements) = t.transformation.preconditions.get("missing") {
                        if elements.contains(&current_path_string) {
                            *t.missing.entry(current_path_string.clone()).or_default() = false;
                        }
                    }
                    if let Some(elements) = t.transformation.preconditions.get("existing") {
                        if elements.contains(&current_path_string) {
                            *t.existing.entry(current_path_string.clone()).or_default() = true;
                        }
                    }
                    if t.transformation.target == current_path_string {
                        if !t.transformation.keep {
                            keep = false;
                        }
                    }
                }
                if !keep {
                    continue;
                }
                if superordinate(&current_path, &split_path).unwrap() {
                    for xml_event_list in splitting.values_mut() {
                        xml_event_list.push(Event::Start(e.clone().into_owned()));
                    }
                } else {
                    split_element.push(Event::Start(e.clone().into_owned()));
                    if current_path == split_path {
                        include = true;
                    }
                }
            }

            Ok(Event::Text(e)) => {
                if e.starts_with("\n".as_bytes()) || e.starts_with("\r".as_bytes()) {
                    continue;
                }
                if !keep {
                    continue;
                }
                let current_path_string = current_path.join("/");
                let text_from_event = e.unescape().unwrap().to_string();
                if let Some(hashmap) = config.split.grouping.get(&current_path_string) {
                    match hashmap.get(&text_from_event) {
                        Some(val) => {
                            current_group = val;   
                        }
                        None => {
                            current_group = &config.split.default;
                        }
                    }
                }
                if let Some(list) = config.filter.allowlist.get(&current_path_string) {
                    if list.iter().all(
                        |i| i.to_string() != text_from_event &&
                        // Regex::new(r"a^") does not match "a^", so it can most likely be used as a fallback that does not match anything:
                        !Regex::new(i).unwrap_or_else(|_err| Regex::new(r"a^").unwrap()).is_match(&text_from_event)
                    ) {
                        include = false;
                    }
                }
                if let Some(list) = config.filter.blocklist.get(&current_path_string) {
                    if list.iter().any(
                        |i| i.to_string() == text_from_event ||
                        Regex::new(i).unwrap_or_else(|_err| Regex::new(r"a^").unwrap()).is_match(&text_from_event)
                    ) {
                        include = false;
                    }
                }
                if superordinate(&current_path, &split_path).unwrap() {
                    for xml_event_list in splitting.values_mut() {
                        xml_event_list.push(Event::Text(e.clone().into_owned()));
                    }
                } else {
                    split_element.push(Event::Text(e.clone().into_owned()));
                    for t in &mut transformers {
                        t.check_value(&current_path_string, &text_from_event, &config, &msg_config);
                        t.precondition = (t.missing.is_empty() || t.missing.clone().into_values().all(|v| v == true)) &&
                                        (t.existing.is_empty() || t.existing.clone().into_values().all(|v| v == true));
                        if t.transformation.target == current_path_string &&
                            t.transformation.nodes.is_empty() &&
                            t.precondition {
                            split_element.pop();
                            split_element.push(Event::Text(BytesText::new(&t.value_transformed).into_owned()));
                        }
                    }
                }
            }
   
            Ok(Event::End(e)) => {
                let current_path_string = current_path.join("/");
                if !keep {
                    for t in &mut transformers {
                        if t.transformation.target == current_path_string {
                            keep = true;
                        }
                    }
                    current_path.pop();
                    continue;
                }
                if superordinate(&current_path, &split_path).unwrap() {
                    for xml_event_list in splitting.values_mut() {
                        xml_event_list.push(Event::End(e.clone().into_owned()));
                    }
                } else {
                    for t in &mut transformers {
                        if t.transformation.target == current_path_string {
                            if let Some(path) = t.transformation.nodes.get("insert") {
                                t.precondition = (t.missing.is_empty() || t.missing.clone().into_values().all(|v| v == true)) &&
                                                (t.existing.is_empty() || t.existing.clone().into_values().all(|v| v == true));
                                if t.precondition {
                                    split_element.append(&mut embed(t.value_transformed.to_owned(), path.to_owned()));
                                }
                            }
                        }
                    }
                    split_element.push(Event::End(e.clone().into_owned()));
                    for t in &mut transformers {
                        if t.transformation.target == current_path_string {
                            if let Some(path) = t.transformation.nodes.get("append") {
                                if t.precondition {
                                    split_element.append(&mut embed(t.value_transformed.to_owned(), path.to_owned()));
                                }
                            }
                            t.missing = HashMap::new();
                            t.existing = HashMap::new();
                            t.precondition = true;
                        }
                    }
                    if current_path == split_path {
                        if !include {
                            current_group = &config.filter.residue;
                        }
                        splitting.get_mut(current_group).unwrap().append(&mut split_element);
                    }  
                }
                current_path.pop();
            }
            Ok(Event::Eof) => {
                let msg = get_msg(&msg_config, "end_of_original_file", lang);
                info!("{msg}");
                break
            }
            Ok(e) => {
                if !keep {
                    continue;
                }
                if superordinate(&current_path, &split_path).unwrap() {
                    for xml_event_list in splitting.values_mut() {
                        xml_event_list.push(e.clone().into_owned());
                    }
                } else {
                    split_element.push(e.into_owned());
                }
            }
            Err(err) => {
                let msg = get_msg(&msg_config, "reading_xml_event_failed", lang);
                error!("{msg}: {err}");
                break
            }
        }
        buf.clear();
    }
    Ok(splitting)
}
use std::fs;
use std::io::Result;
use std::collections::HashMap;
use std::path::Path;
use log::{error, info};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
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
                    if !t.transformation.keep && t.transformation.target == current_path_string {
                        keep = false;
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
                    if list.iter().all(|i| i.to_string() != text_from_event) {
                        include = false;
                    }
                }
                if let Some(list) = config.filter.blocklist.get(&current_path_string) {
                    if list.iter().any(|i| i.to_string() == text_from_event) {
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
                        if t.transformation.target == current_path_string && t.transformation.append_element == "" {
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
                    split_element.push(Event::End(e.clone().into_owned()));
                    for t in &mut transformers {
                        if t.transformation.append_element != "" {
                            if t.transformation.target == current_path_string {
                                let start_tag = BytesStart::new( t.transformation.append_element.clone());
                                let end_tag = BytesEnd::new(t.transformation.append_element.clone());
                                split_element.push(Event::Start(start_tag));
                                split_element.push(Event::Text(BytesText::new(&t.value_transformed).into_owned()));
                                split_element.push(Event::End(end_tag));
                            }
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
use log::{error, warn};
use serde::Deserialize;
use std::collections::HashMap;
use evalexpr::*;

use crate::{get_msg, Config};

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Source {
    pub datafields: HashMap<String, String>,
    pub literals: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Transformation {
    pub target: String,
    pub keep: bool,
    pub value: String,
    pub nodes: HashMap<String, String>,
    pub source: Source,
    pub parameters: HashMap<String, String>,
    pub preconditions: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Transformer {
    pub transformation: Transformation,
    pub parameters: HashMap<String, String>,
    pub value_computed: bool,
    pub value_transformed: String,
    pub missing: HashMap<String, bool>,
    pub existing: HashMap<String, bool>,
    pub precondition: bool,
}

impl Transformer {
    pub fn new(transformation: Transformation) -> Self {
        Transformer {
            transformation,
            precondition: true,
            ..Default::default()
        }
    }
    
    pub fn eval_expr(&mut self, config: &Config, msg_config: &HashMap<String, HashMap<String, String>>) -> () {   
        let evaluation_error = get_msg(&msg_config, "evaluation_failed", &config.settings.lang);
                match build_operator_tree(&self.transformation.value) {
                    Ok(node) => {
                        let mut context = HashMapContext::new();
                        for (var, val) in &self.parameters {
                            let v = val.clone();
                            if let Ok(i) = v.parse::<i64>() {
                                context.set_value(var.as_str().into(), i.into()).unwrap();

                            } else if let Ok(f) = v.parse::<f64>() {
                                context.set_value(var.as_str().into(), f.into()).unwrap();
                            } else {
                                context.set_value(var.as_str().into(), v.to_string().into()).unwrap();
                            }
                        }

                        match node.eval_with_context_mut(&mut context) {
                            Ok(v) => {
                                match v {
                                    Value::Float(f) => {
                                        if let Some(num_str) = self.transformation.parameters.get("decimal_places") {
                                            match num_str.parse::<usize>() {
                                                Ok(num) => self.value_transformed = format!("{:.1$}", f, num),
                                                Err(err) => {
                                                    self.value_transformed = num_str.to_string();
                                                    let warning = get_msg(&msg_config, "decimal_places_not_parsable", &config.settings.lang);
                                                    warn!("{warning}: {num_str} - {err}"); 
                                                }
                                            }
                                        } else {
                                            self.value_transformed = f.to_string(); 
                                        }
                                    },
                                    Value::Int(i) => {
                                        if let Some(num_str) = self.transformation.parameters.get("decimal_places") {
                                            match num_str.parse::<usize>() {
                                                Ok(num) => self.value_transformed = format!("{:.1$}", i as f64, num),
                                                Err(err) => {
                                                    self.value_transformed = num_str.to_string();
                                                    let warning = get_msg(&msg_config, "decimal_places_not_parsable", &config.settings.lang);
                                                    warn!("{warning}: {num_str} - {err}");
                                                }
                                            }
                                        } else {
                                            self.value_transformed = i.to_string(); 
                                        }
                                    },
                                    Value::Empty => {
                                        self.value_transformed = "".to_string();
                                        let warning = get_msg(&msg_config, "empty_value", &config.settings.lang);
                                        warn!("{warning}: {v}");
                                    }
                                    _ => {
                                        let value = v.to_string();
                                        match value.strip_prefix('"') {
                                            Some(prefixless) => {
                                                match prefixless.strip_suffix('"') {
                                                    Some(suffixless) => {
                                                        self.value_transformed = suffixless.to_string();
                                                    }
                                                    None => self.value_transformed = value,
                                                }
                                            },
                                            None => self.value_transformed = value,
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                error!("{evaluation_error}: {err}");
                                panic!();
                            }
                        }
                    }
                    Err(err) => {
                        error!("{evaluation_error}: {err}");
                        panic!();
                    }
                }
    }

    pub fn check_value(
        &mut self,
        current_path_string: &String,
        text_from_event: &String,
        config: &Config,
        msg_config: &HashMap<String, HashMap<String, String>>
    ) -> () {
        let datafields = &self.transformation.source.datafields;
        let literals = &self.transformation.source.literals;
        // If no sources are specified for the assignment of the variables, it can be assumed
        // that the transformation value is to be inserted as a string literal:
        if datafields.is_empty() && literals.is_empty() {
            self.value_transformed = self.transformation.value.to_string();
            self.value_computed = true;
            return;
        }
        for (k, v) in literals {
            if self.parameters.get(k).is_none() {
                self.parameters.insert(k.to_string(), v.to_string());
            }
        }
        for (k, v) in datafields {
            if v == current_path_string {
                if self.parameters.get(k).is_none() {
                    self.parameters.insert(k.to_string(), text_from_event.clone());
                } else {
                    *self.parameters.get_mut(k).unwrap() = text_from_event.clone();
                }
            }
        }
        if self.parameters.len() >= datafields.len() + literals.len() {
            self.eval_expr(&config, &msg_config);
        }
    }
}
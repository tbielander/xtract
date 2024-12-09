use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Filter {
    pub residue: String,
    pub allowlist: HashMap<String, Vec<String>>,
    pub blocklist: HashMap<String, Vec<String>>,
}
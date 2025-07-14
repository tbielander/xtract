use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Filter {
    pub residue: String,
    pub allowlist: Allowlist,
    pub blocklist: Blocklist,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Allowlist {
    pub exact: HashMap<String, Vec<String>>,
    pub regex: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Blocklist {
    pub exact: HashMap<String, Vec<String>>,
    pub regex: HashMap<String, Vec<String>>,
}
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Split {
    pub declaration: bool,
    pub default: String,
    pub grouping: HashMap<String, HashMap<String, String>>,
}
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Service {
    pub cmd: String,
    pub watch: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub vars: HashMap<String, String>,
    #[serde(rename = "service")]
    pub services: HashMap<String, Service>,
}

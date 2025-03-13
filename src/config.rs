use serde::Deserialize;
use crate::transformations::transformer::*;
use crate::transformations::filter::*;
use crate::transformations::split::*;
use crate::upload::*;

#[derive(Deserialize, Debug, Clone)]
pub struct Dirs {
    pub original: String,
    pub transformed: String,
    pub history: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Timeformat {
    pub history_folder: String,
    pub files: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Mailer {
    pub smtp: String,
    pub port: u16,
    pub auth: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EmailMessage {
    pub from: String,
    pub reply_to: Vec<String>,
    pub to: Vec<String>,
    pub subject: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Email {
    pub mailer: Mailer,
    pub message: EmailMessage,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Settings {
    pub lang: String,
    pub history_size: usize,
    pub consistency_check: bool,
    pub inconsistency_notification: bool,
    pub dirs: Dirs,
    pub timeformats: Timeformat,
    pub email: Email,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub element: String,
    pub filter: Filter,
    pub split: Split,
    pub transformations: Vec<Transformation>,
    pub uploads: Vec<Upload>,
    pub settings: Settings,
}
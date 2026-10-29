//! The four lists. Filled in 0.1.2.
use crate::model::Project;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct List {
    pub kind: String,
}

pub fn build(_p: &Project, _kind: &str) -> Result<List, String> {
    Err("The lists arrive in Datum Schema 0.1.2.".into())
}
pub fn csv(_l: &List) -> String {
    String::new()
}
pub fn xlsx(_p: &Project, _kind: &str) -> Result<Vec<u8>, String> {
    Err("The lists arrive in Datum Schema 0.1.2.".into())
}

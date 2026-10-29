//! The worked examples. Filled in 0.1.3.
use crate::model::Project;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Example {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
}

pub fn list() -> Vec<Example> {
    vec![]
}
pub fn get(_id: &str) -> Option<Project> {
    None
}

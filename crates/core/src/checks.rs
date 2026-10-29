//! The live checks (brief §2.4). Filled in 0.1.2.
use crate::graph::Net;
use crate::hydraulics::Hyd;
use crate::model::Project;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Issue {
    pub rule: u8,
    pub level: &'static str,
    pub msg: String,
    pub ids: Vec<u32>,
    pub sheet: u32,
    pub at: [f64; 2],
}

pub fn run(_p: &Project, _net: &Net, _hyd: &[Option<Hyd>]) -> Vec<Issue> {
    vec![]
}

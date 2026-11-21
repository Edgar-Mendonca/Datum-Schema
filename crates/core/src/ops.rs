//! The named operations: every change to a project is one of these, whether it comes from the
//! window, the command line or an AI assistant. Each has a name, what it does and an example; ids
//! are the ids of symbols, lines, sheets and objects as `info`, `objects` and the lists give them.
//! A failed operation changes nothing; every operation can be undone.

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Debug, Clone)]
pub struct OpInfo {
    pub name: &'static str,
    pub about: &'static str,
    pub example: Value,
}

/// The operations scripts and assistants use most, with examples that run on the Pump transfer
/// example (sheet 1; see `objects` for ids).
pub fn catalog() -> Vec<OpInfo> {
    let o = |name, about, example| OpInfo { name, about, example };
    vec![
        o("AddItem", "Place a symbol (sym is a symbol key: gate, pump_cent, vvessel, inst_field…; see `symbols`) on a sheet at x, y mm with an optional tag; on: a line id to drop an in-line symbol onto (it splits the line).", json!({"AddItem": {"item": {"sheet": 1, "sym": "gate", "x": 100.0, "y": 60.0}, "on": null, "tag": "HV-201"}})),
        o("AddItem (virtual)", "Add a virtual instrument: in its loop, the lists, the I/O and its datasheet, not drawn (virt: true; instrument bubbles only). SetItem with virt false draws it.", json!({"AddItem": {"item": {"sheet": 1, "sym": "inst_dcs", "x": 0.0, "y": 0.0, "virt": true}, "on": null, "tag": "FY-101"}})),
        o("AddLine", "Draw a line between two ends: {\"at\":\"port\",\"item\":id,\"port\":n}, {\"at\":\"line\",\"line\":id,\"x\":..,\"y\":..} (a branch) or {\"at\":\"free\",\"x\":..,\"y\":..}; kind major, minor, utility, pneumatic, electric, data or capillary.", json!({"AddLine": {"line": {"sheet": 1, "kind": "major", "a": {"at": "free", "x": 40.0, "y": 60.0}, "b": {"at": "free", "x": 90.0, "y": 60.0}}}})),
        o("SetData", "Set one data value on symbols, lines or objects (by any of their ids): a number in SI base units (Pa, K, m³/s, m…) or text; val null removes it. Keys are the class attributes (size, service, design_p, design_t, flow, position, fail…).", json!({"SetData": {"ids": [11], "key": "service", "val": "Pump suction isolation"}})),
        o("SetTag", "Give a symbol's object a tag (it must be free).", json!({"SetTag": {"id": 11, "tag": "HV-101"}})),
        o("Move", "Move symbols, lines and notes by dx, dy mm.", json!({"Move": {"ids": [11], "dx": 5.0, "dy": 0.0}})),
        o("Rotate", "Turn symbols a quarter turn clockwise.", json!({"Rotate": {"ids": [11]}})),
        o("Flip", "Mirror symbols left to right.", json!({"Flip": {"ids": [11]}})),
        o("Delete", "Delete symbols, lines and notes (keep: true keeps their objects in the Plant panel, not drawn).", json!({"Delete": {"ids": [11], "keep": false}})),
        o("AddSheet", "Add a sheet (number, title, size A3, A1, ANSI B or ANSI D; pfd: true for a process flow diagram; legend: true for a legend sheet).", json!({"AddSheet": {"sheet": {"number": "PID-102", "title": "Cooling water", "size": "A3"}}})),
        o("DeleteSheet", "Delete a sheet and what is on it (it is kept in the sheet manager's bin).", json!({"DeleteSheet": {"id": 2}})),
        o("RestoreSheet", "Bring back a deleted sheet (at 0 is the most recent).", json!({"RestoreSheet": {"at": 0}})),
        o("Fix", "Apply a fix offered for a check (rule and the ids it is about, as `check` lists them; choice is the fix's number from 0).", json!({"Fix": {"rule": 8, "ids": [21], "choice": 0}})),
        o("PlaceLoop", "Place a control loop on a line or vessel in one step: var flow, level, pressure or temperature (control: false for an indicator only).", json!({"PlaceLoop": {"target": 40, "var": "flow", "control": true}})),
        o("Arrange", "Place a standard arrangement round equipment (name: one of the project's arrangements; empty: the one for its class).", json!({"Arrange": {"ids": [17], "name": ""}})),
        o("SwapSymbol", "Swap a symbol for another of its kind, keeping its tag, data and lines.", json!({"SwapSymbol": {"item": 11, "sym": "ball"}})),
        o("AddSpare", "Add a spare beside a piece of equipment: P-101 becomes P-101A and the spare P-101B.", json!({"AddSpare": {"item": 17}})),
        o("AddNozzle", "Add a nozzle to a piece of equipment at a point on its outline (sheet mm).", json!({"AddNozzle": {"item": 41, "x": 350.0, "y": 150.0, "name": ""}})),
        o("SetNozzle", "Set a nozzle's size, rating or service.", json!({"SetNozzle": {"item": 41, "name": "N1", "field": "size", "value": "2"}})),
        o("Register", "Add tags to the register before they are drawn (class optional: guessed from the letters).", json!({"Register": {"rows": [{"tag": "P-102", "class": "", "service": "Future pump", "status": "new", "note": ""}]}})),
        o("Develop", "Develop a first P&ID from a PFD sheet.", json!({"Develop": {"sheet": 1}})),
        o("MakePfd", "Make a PFD from the P&ID sheets (empty: all).", json!({"MakePfd": {"sheets": []}})),
        o("SetCase", "Switch the process case shown (Normal, Maximum…).", json!({"SetCase": {"case": "Maximum"}})),
        o("IssueRevision", "Issue a sheet at a revision (kept for comparison and clouds).", json!({"IssueRevision": {"sheet": 1, "rev": {"rev": "B", "date": "2026-11-21", "desc": "Issued for approval", "by": "EM"}}})),
        o("HazopNode", "Make a HAZOP node from symbols and lines.", json!({"HazopNode": {"ids": [17, 11], "name": "Duty pump"}})),
        o("HazopSuggest", "Add the deviations suggested from the drawing to a HAZOP node's worksheet.", json!({"HazopSuggest": {"node": 1}})),
        o("MakeTestPacks", "Make pressure test packs of the line numbers not yet in one.", json!("MakeTestPacks")),
        o("Comment", "Pin a review comment on a sheet at x, y mm (on: the ids it is about).", json!({"Comment": {"sheet": 1, "x": 120.0, "y": 80.0, "text": "Check the strainer size", "on": []}})),
        o("Accept", "Accept a check's problem with a reason, kept for audit (its rule, ids and message exactly as the checks give them). It comes back if the drawing changes what the check says.", json!({"Accept": {"rule": 27, "ids": [5], "msg": "T-101 has no PSH (high pressure) on the drawing: add it, or say it is not needed (Safety, not needed).", "reason": "Vents to atmosphere", "by": "", "date": "2026-11-21"}})),
        o("Unaccept", "Open an accepted problem again.", json!({"Unaccept": {"rule": 27, "ids": [5], "msg": "T-101 has no PSH (high pressure) on the drawing: add it, or say it is not needed (Safety, not needed)."}})),
        o("Renumber", "Renumber tags and line numbers in drawing order.", json!("Renumber")),
        o("Batch", "Several operations as one edit (one undo).", json!({"Batch": {"ops": [{"SetData": {"ids": [11], "key": "size", "val": "4"}}, {"SetTag": {"id": 11, "tag": "HV-111"}}]}})),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_example_is_an_operation() {
        for o in super::catalog() {
            let op: Result<crate::doc::Op, _> = serde_json::from_value(o.example.clone());
            assert!(op.is_ok(), "{}: {:?}", o.name, op.err());
            let first = o.example.as_object().and_then(|m| m.keys().next().cloned()).or_else(|| o.example.as_str().map(String::from)).unwrap();
            assert!(o.name.starts_with(&first), "{}", o.name);
        }
    }
}

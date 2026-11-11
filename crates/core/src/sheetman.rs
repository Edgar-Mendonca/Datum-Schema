//! The sheet manager: every sheet with its status, revision and contents in one table, and the
//! sheets deleted from the project kept in a bin, from which they come back whole (their symbols,
//! lines, notes and the objects only they held).

use crate::model::{End, Line, Note, Obj, Project, Sheet};
use crate::model::Item;
use serde::{Deserialize, Serialize};

/// A sheet's drawing status, in its usual order ("" counts as the first).
pub const STATUSES: [&str; 5] = ["Draft", "For review", "Issued for approval", "Issued for construction", "As-built"];
/// How many deleted sheets the bin keeps (the oldest goes first).
pub const BIN_MAX: usize = 20;

/// A deleted sheet, with what was on it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Binned {
    pub sheet: Sheet,
    /// Where it was in the sheet order.
    pub index: usize,
    pub items: Vec<Item>,
    pub lines: Vec<Line>,
    pub notes: Vec<Note>,
    /// The objects that went with it (drawn on no other sheet).
    pub objects: Vec<Obj>,
}

/// One row of the sheet manager.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Row {
    pub id: u32,
    pub number: String,
    pub title: String,
    pub size: String,
    pub rev: String,
    pub date: String,
    pub status: String,
    /// P&ID, PFD or Legend.
    pub kind: &'static str,
    pub symbols: usize,
    pub lines: usize,
    pub errors: usize,
    pub warnings: usize,
    /// The last issued revision ("" for none).
    pub issued: String,
}

/// A deleted sheet in the bin, as listed.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BinRow {
    pub number: String,
    pub title: String,
    pub symbols: usize,
    pub lines: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Manager {
    pub sheets: Vec<Row>,
    pub bin: Vec<BinRow>,
    pub statuses: Vec<&'static str>,
}

/// The sheet manager's table.
pub fn manager(p: &Project) -> Manager {
    let net = crate::graph::Net::new(p);
    let h = crate::hydraulics::all(p, &net);
    let issues = crate::checks::run(p, &net, &h);
    let sheets = p
        .sheets
        .iter()
        .map(|s| Row {
            id: s.id,
            number: s.number.clone(),
            title: s.title.clone(),
            size: s.size.clone(),
            rev: s.rev.clone(),
            date: s.date.clone(),
            status: if s.status.is_empty() { STATUSES[0].into() } else { s.status.clone() },
            kind: if s.legend { "Legend" } else if s.pfd { "PFD" } else { "P&ID" },
            symbols: p.items.iter().filter(|i| i.sheet == s.id).count(),
            lines: p.lines.iter().filter(|l| l.sheet == s.id).count(),
            errors: issues.iter().filter(|i| i.sheet == s.id && i.level == "error").count(),
            warnings: issues.iter().filter(|i| i.sheet == s.id && i.level == "warning").count(),
            issued: s.snaps.last().map(|x| x.rev.clone()).unwrap_or_default(),
        })
        .collect();
    let bin = p.bin.iter().rev().map(|b| BinRow { number: b.sheet.number.clone(), title: b.sheet.title.clone(), symbols: b.items.len(), lines: b.lines.len() }).collect();
    Manager { sheets, bin, statuses: STATUSES.to_vec() }
}

/// What a sheet holds, taken before it is deleted (the objects are kept by `binned`).
pub fn take(p: &Project, id: u32) -> Option<Binned> {
    let index = p.sheets.iter().position(|s| s.id == id)?;
    let mut sheet = p.sheets[index].clone();
    // Issued snapshots stay with the project's revisions, not in the bin.
    for x in &mut sheet.snaps {
        x.body = None;
    }
    let items: Vec<Item> = p.items.iter().filter(|i| i.sheet == id).cloned().collect();
    let objs: std::collections::HashSet<u32> = items.iter().map(|i| i.obj).chain(p.lines.iter().filter(|l| l.sheet == id).map(|l| l.obj)).filter(|o| *o != 0).collect();
    Some(Binned {
        sheet,
        index,
        items,
        lines: p.lines.iter().filter(|l| l.sheet == id).cloned().collect(),
        notes: p.notes.iter().filter(|n| n.sheet == id).cloned().collect(),
        objects: p.objects.iter().filter(|o| objs.contains(&o.id)).cloned().collect(),
    })
}

/// Puts a deleted sheet in the bin, keeping only the objects the delete took away.
pub fn binned(p: &mut Project, mut b: Binned) {
    b.objects.retain(|o| p.obj(o.id).is_none());
    p.bin.push(b);
    if p.bin.len() > BIN_MAX {
        let n = p.bin.len() - BIN_MAX;
        p.bin.drain(..n);
    }
}

/// Brings back a deleted sheet (`at` counts from the most recent, as listed) where it was.
pub fn restore(p: &mut Project, at: usize) -> Result<u32, String> {
    let i = p.bin.len().checked_sub(at + 1).ok_or("That sheet is no longer in the bin.")?;
    let b = p.bin[i].clone();
    for o in &b.objects {
        if !o.tag.is_empty() && p.objects.iter().any(|x| x.id != o.id && x.tag.eq_ignore_ascii_case(&o.tag)) {
            return Err(format!("{} on that sheet is now another object's tag: rename that one first.", o.tag));
        }
    }
    let taken = |p: &Project, id: u32| p.sheets.iter().any(|s| s.id == id) || p.items.iter().any(|x| x.id == id) || p.lines.iter().any(|x| x.id == id) || p.notes.iter().any(|x| x.id == id);
    let mut sheet = b.sheet.clone();
    if taken(p, sheet.id) {
        sheet.id = p.id();
    }
    if p.sheets.iter().any(|s| s.number.eq_ignore_ascii_case(&sheet.number)) {
        sheet.number = format!("{} (restored)", sheet.number);
    }
    let sid = sheet.id;
    p.sheets.insert(b.index.min(p.sheets.len()), sheet);
    for o in b.objects {
        if p.obj(o.id).is_none() {
            let at = p.objects.partition_point(|x| x.id < o.id);
            p.objects.insert(at, o);
        }
    }
    // Ids are never given out twice, but a damaged file may have: anything clashing gets a new id.
    let mut remap = std::collections::HashMap::new();
    let fresh = |p: &mut Project, remap: &mut std::collections::HashMap<u32, u32>, id: u32| if taken(p, id) { let n = p.id(); remap.insert(id, n); n } else { id };
    let mut items = vec![];
    for mut it in b.items {
        it.id = fresh(p, &mut remap, it.id);
        it.sheet = sid;
        if it.obj != 0 && p.obj(it.obj).is_none() {
            it.obj = 0;
        }
        items.push(it);
    }
    p.items.extend(items);
    let mut lines = vec![];
    for mut l in b.lines {
        l.id = fresh(p, &mut remap, l.id);
        l.sheet = sid;
        lines.push(l);
    }
    let ids: std::collections::HashSet<u32> = p.items.iter().map(|i| i.id).chain(p.lines.iter().map(|l| l.id)).chain(lines.iter().map(|l| l.id)).collect();
    for l in &mut lines {
        for e in [&mut l.a, &mut l.b] {
            match e {
                End::Port { item, .. } => *item = *remap.get(item).unwrap_or(item),
                End::Line { line, x, y } => {
                    *line = *remap.get(line).unwrap_or(line);
                    if !ids.contains(line) {
                        *e = End::Free { x: *x, y: *y };
                    }
                }
                End::Free { .. } => {}
            }
        }
    }
    p.lines.extend(lines);
    for mut n in b.notes {
        n.id = fresh(p, &mut remap, n.id);
        n.sheet = sid;
        p.notes.push(n);
    }
    p.bin.remove(i);
    Ok(sid)
}

#[cfg(test)]
mod tests {
    use crate::doc::{Doc, Op};

    #[test]
    fn a_deleted_sheet_comes_back_whole() {
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        let s1 = d.p.sheets[0].id;
        let s2 = d.apply(Op::AddSheet { sheet: crate::model::Sheet { number: "PID-002".into(), ..Default::default() } }).unwrap()[0];
        let before = d.p.clone();
        let m = super::manager(&d.p);
        assert_eq!(m.sheets.len(), 2);
        assert_eq!((m.sheets[0].status.as_str(), m.sheets[0].kind, m.sheets[0].errors), ("Draft", "P&ID", 0));
        assert!(m.sheets[0].symbols > 10 && m.sheets[0].lines > 5);
        // A status set and kept; an unknown one refused.
        let mut s = d.p.sheets[0].clone();
        s.status = "For review".into();
        d.apply(Op::SetSheet { sheet: s.clone() }).unwrap();
        assert_eq!(super::manager(&d.p).sheets[0].status, "For review");
        s.status = "Whatever".into();
        assert!(d.apply(Op::SetSheet { sheet: s }).is_err());
        // Delete the drawn sheet: its objects go, the bin keeps them.
        let tags = d.p.objects.len();
        d.apply(Op::DeleteSheet { id: s1 }).unwrap();
        assert!(d.p.objects.len() < tags && d.p.items.is_empty());
        let m = super::manager(&d.p);
        assert_eq!(m.bin.len(), 1);
        assert_eq!(m.bin[0].number, "PID-100");
        d.apply(Op::RestoreSheet { at: 0 }).unwrap();
        assert!(d.p.bin.is_empty());
        assert_eq!(d.p.sheets.iter().map(|s| s.id).collect::<Vec<_>>(), [s1, s2], "back where it was");
        assert_eq!(d.p.items, before.items);
        assert_eq!(d.p.lines, before.lines);
        assert_eq!(d.p.objects.len(), tags);
        assert!(crate::api::derived(&d.p, 0).issues.iter().all(|i| i.level != "error"));
        // Kept through save and open.
        d.apply(Op::DeleteSheet { id: s2 }).unwrap();
        let path = std::env::temp_dir().join(format!("bin-{}.dschema", std::process::id()));
        crate::file::write(&path, &d.p, &[]).unwrap();
        let (back, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(back.bin, d.p.bin);
        // A tag taken since refuses the restore; the bin empties.
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        d.apply(Op::AddSheet { sheet: crate::model::Sheet { number: "PID-002".into(), ..Default::default() } }).unwrap();
        let s1 = d.p.sheets[0].id;
        let s2 = d.p.sheets[1].id;
        d.apply(Op::DeleteSheet { id: s1 }).unwrap();
        d.apply(Op::AddItem { item: crate::model::Item { sheet: s2, sym: "pump_cent".into(), x: 100.0, y: 100.0, ..Default::default() }, on: None, tag: "P-101A".into(), obj: None }).unwrap();
        assert!(d.apply(Op::RestoreSheet { at: 0 }).unwrap_err().contains("P-101A"));
        d.apply(Op::EmptyBin).unwrap();
        assert!(d.p.bin.is_empty());
        // Undo of a delete needs no bin.
        let mut d = Doc::new(crate::examples::get("pump").unwrap());
        d.apply(Op::AddSheet { sheet: Default::default() }).unwrap();
        let n = d.p.items.len();
        d.apply(Op::DeleteSheet { id: d.p.sheets[0].id }).unwrap();
        d.undo();
        assert_eq!(d.p.items.len(), n);
    }
}

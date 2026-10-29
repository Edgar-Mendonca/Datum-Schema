//! An open project: edit operations, undo and redo, and file safety (size-limited reads, atomic
//! writes). Every edit runs on a copy and is kept only if the result is sound, so a failed edit
//! leaves the project exactly as it was. Undo keeps the project as it was before each edit.

use crate::graph::{self, end_pt};
use crate::model::{self, End, Item, Line, Note, Project, Settings, Sheet, Title, Val};
use crate::symbols::{self, Cat};
use crate::tags;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const UNDO_LIMIT: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Op {
    /// Place a symbol (an empty tag gets the next free one). In-line symbols dropped on a line
    /// (`on`) split it and join both halves.
    AddItem { item: Item, on: Option<u32> },
    /// Replace an item (position, turn, mirror, scale, tag, data, partner).
    SetItem { item: Item },
    AddLine { line: Line },
    SetLine { line: Line },
    AddNote { note: Note },
    SetNote { note: Note },
    /// Move items, notes, and lines' bend points and open ends.
    Move { ids: Vec<u32>, dx: f64, dy: f64 },
    /// Turn items a quarter clockwise.
    Rotate { ids: Vec<u32> },
    /// Mirror items left to right.
    Flip { ids: Vec<u32> },
    /// Delete items, lines and notes. An in-line item between two lines joins them again.
    Delete { ids: Vec<u32> },
    /// Set (or with `val` None, clear) one data field on items or lines.
    SetData { ids: Vec<u32>, key: String, val: Option<Val> },
    SetTag { id: u32, tag: String },
    /// A line's bend points (empty: route automatically).
    SetPts { id: u32, pts: Vec<[f64; 2]> },
    /// Put an in-line item into a line.
    Split { line: u32, item: u32 },
    /// Copies of items, lines and notes on a sheet, moved by (dx, dy), with the next free tags.
    Paste { sheet: u32, items: Vec<Item>, lines: Vec<Line>, notes: Vec<Note>, dx: f64, dy: f64 },
    /// Pair two off-page connectors (b = 0 unpairs a).
    Link { a: u32, b: u32 },
    AddSheet { sheet: Sheet },
    SetSheet { sheet: Sheet },
    DeleteSheet { id: u32 },
    MoveSheet { id: u32, to: usize },
    SetTitle { title: Title },
    SetProject { name: String, units: String },
    SetSettings { settings: Settings },
    /// Number every tag again (per prefix, per sheet, left to right).
    Renumber,
    Batch { ops: Vec<Op> },
}

pub struct Doc {
    pub p: Project,
    pub path: Option<String>,
    pub dirty: bool,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

impl Doc {
    pub fn new(p: Project) -> Self {
        Self { p, path: None, dirty: false, undo: vec![], redo: vec![] }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Applies an edit; returns the ids it created. On error nothing changes.
    pub fn apply(&mut self, op: Op) -> Result<Vec<u32>, String> {
        let mut q = self.p.clone();
        let mut created = vec![];
        exec(&mut q, op, &mut created)?;
        let w = q.normalize();
        if let Some(first) = w.first() {
            return Err(format!("That edit would leave the drawing unsound: {first}"));
        }
        if q == self.p {
            return Ok(created);
        }
        self.undo.push(std::mem::replace(&mut self.p, q));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        Ok(created)
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }

    pub fn open(path: &Path) -> Result<(Self, Vec<String>), String> {
        let text = read_limited(path)?;
        let (p, w) = model::from_json(&text)?;
        let mut d = Doc::new(p);
        d.path = Some(path.display().to_string());
        Ok((d, w))
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        write_atomic(path, model::to_json(&self.p).as_bytes())?;
        self.path = Some(path.display().to_string());
        self.dirty = false;
        Ok(())
    }
}

/// The largest file Datum Schema opens.
pub const MAX_FILE: u64 = 200 * 1024 * 1024;

/// Reads a text file of at most `MAX_FILE` bytes (invalid UTF-8 is replaced, not fatal).
pub fn read_limited(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?.len();
    if len > MAX_FILE {
        return Err(format!("{} is {} MB; Datum Schema opens files up to {} MB.", path.display(), len / 1_048_576, MAX_FILE / 1_048_576));
    }
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("Could not open {}: {e}", path.display()))
}

/// Writes a file so that a crash or a full disk never leaves it half-written: the new content goes
/// to a temporary file next to it, which then replaces the old one in one step.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("Could not save {}: {e}", path.display());
    let tmp = path.with_extension(format!("{}.saving", path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")));
    let mut f = std::fs::File::create(&tmp).map_err(fail)?;
    if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e));
    }
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fail(e)
    })
}

fn find_item(p: &mut Project, id: u32) -> Result<&mut Item, String> {
    p.items.iter_mut().find(|i| i.id == id).ok_or_else(|| "That symbol no longer exists.".to_string())
}
fn find_line(p: &mut Project, id: u32) -> Result<&mut Line, String> {
    p.lines.iter_mut().find(|i| i.id == id).ok_or_else(|| "That line no longer exists.".to_string())
}
fn finite(v: &[f64]) -> Result<(), String> {
    if v.iter().all(|x| x.is_finite() && x.abs() <= model::MAX_COORD) { Ok(()) } else { Err("A position is out of range.".into()) }
}

/// The tag a new item gets when it has none: the next free tag for its prefix, or for an
/// instrument given only its letters, the next free loop number.
fn auto_tag(p: &Project, it: &Item) -> String {
    let Some(s) = symbols::get(&it.sym) else { return it.tag.clone() };
    let t = it.tag.trim();
    if !t.is_empty() && t.chars().any(|c| c.is_ascii_digit()) {
        return t.to_string();
    }
    if (s.cat.instrument() || s.cat == Cat::Control) && !t.is_empty() {
        let lp = it.data.get("loop").map(|v| v.text()).filter(|x| !x.trim().is_empty());
        let n = lp.unwrap_or_else(|| next_loop(p, it.sheet));
        return format!("{}-{n}", t.to_uppercase());
    }
    if t.is_empty() { tags::next_free(p, s.prefix, it.sheet) } else { tags::next_free(p, t, it.sheet) }
}

/// The next loop number not used by any instrument or control valve on a sheet (101 … per sheet).
pub fn next_loop(p: &Project, sheet: u32) -> String {
    let used: HashSet<u64> = p.items.iter().filter(|i| symbols::get(&i.sym).is_some_and(|s| s.cat.instrument() || s.cat == Cat::Control)).filter_map(|i| tags::split(&i.tag).1).collect();
    let mut n = p.sheet_no(sheet).max(1) as u64 * 100 + 1;
    while used.contains(&n) {
        n += 1;
    }
    n.to_string()
}

fn exec(p: &mut Project, op: Op, created: &mut Vec<u32>) -> Result<(), String> {
    match op {
        Op::AddItem { mut item, on } => {
            let s = symbols::get(&item.sym).ok_or("There is no such symbol.")?;
            finite(&[item.x, item.y, item.scale])?;
            if p.sheet(item.sheet).is_none() {
                return Err("That sheet no longer exists.".into());
            }
            item.id = p.id();
            if item.scale == 0.0 {
                item.scale = 1.0;
            }
            if s.cat == Cat::Connector {
                item.link = 0;
            }
            item.tag = auto_tag(p, &item);
            let id = item.id;
            p.items.push(item);
            created.push(id);
            if let Some(line) = on.filter(|_| s.inline) {
                created.extend(split(p, line, id)?);
            }
        }
        Op::SetItem { item } => {
            finite(&[item.x, item.y, item.scale])?;
            symbols::get(&item.sym).ok_or("There is no such symbol.")?;
            let old = find_item(p, item.id)?;
            let (sheet, link) = (old.sheet, old.link);
            *old = Item { sheet, link, rot: item.rot % 4, ..item };
        }
        Op::AddLine { mut line } => {
            if p.sheet(line.sheet).is_none() {
                return Err("That sheet no longer exists.".into());
            }
            if line.a == line.b {
                return Err("A line needs two different ends.".into());
            }
            line.id = p.id();
            // New process lines on a sheet get its next sequence number.
            if line.kind.process() && !line.data.contains_key("seq") {
                line.data.insert("seq".into(), Val::Text(tags::next_seq(p, line.sheet)));
            }
            created.push(line.id);
            p.lines.push(line);
        }
        Op::SetLine { line } => {
            let old = find_line(p, line.id)?;
            let sheet = old.sheet;
            *old = Line { sheet, ..line };
        }
        Op::AddNote { mut note } => {
            if p.sheet(note.sheet).is_none() {
                return Err("That sheet no longer exists.".into());
            }
            note.id = p.id();
            created.push(note.id);
            p.notes.push(note);
        }
        Op::SetNote { note } => {
            let old = p.notes.iter_mut().find(|n| n.id == note.id).ok_or("That note no longer exists.")?;
            let sheet = old.sheet;
            *old = Note { sheet, ..note };
        }
        Op::Move { ids, dx, dy } => {
            finite(&[dx, dy])?;
            let set: HashSet<u32> = ids.iter().copied().collect();
            for it in p.items.iter_mut().filter(|i| set.contains(&i.id)) {
                it.x += dx;
                it.y += dy;
            }
            for n in p.notes.iter_mut().filter(|i| set.contains(&i.id)) {
                n.x += dx;
                n.y += dy;
                n.to[0] += dx;
                n.to[1] += dy;
            }
            // Lines move with the selection: bend points of selected lines, and open ends of
            // selected lines or of lines whose both ends are on moved items.
            for l in p.lines.iter_mut() {
                let moved = set.contains(&l.id) || (l.a.item().is_some_and(|x| set.contains(&x.0)) && l.b.item().is_some_and(|x| set.contains(&x.0)));
                if !moved {
                    continue;
                }
                for q in &mut l.pts {
                    q[0] += dx;
                    q[1] += dy;
                }
                for e in [&mut l.a, &mut l.b] {
                    match e {
                        End::Free { x, y } | End::Line { x, y, .. } => {
                            *x += dx;
                            *y += dy;
                        }
                        _ => {}
                    }
                }
            }
        }
        Op::Rotate { ids } => {
            for it in p.items.iter_mut().filter(|i| ids.contains(&i.id)) {
                it.rot = (it.rot + 1) % 4;
            }
        }
        Op::Flip { ids } => {
            for it in p.items.iter_mut().filter(|i| ids.contains(&i.id)) {
                it.flip = !it.flip;
            }
        }
        Op::Delete { ids } => delete(p, &ids),
        Op::SetData { ids, key, val } => {
            if key.is_empty() || key.len() > 64 {
                return Err("That is not a field.".into());
            }
            if let Some(Val::Num(v)) = &val {
                if !v.is_finite() {
                    return Err("That is not a number.".into());
                }
            }
            let set: HashSet<u32> = ids.iter().copied().collect();
            let mut n = 0;
            for d in p.items.iter_mut().filter(|i| set.contains(&i.id)).map(|i| &mut i.data).chain(p.lines.iter_mut().filter(|l| set.contains(&l.id)).map(|l| &mut l.data)) {
                n += 1;
                match &val {
                    Some(Val::Text(t)) if t.trim().is_empty() => {
                        d.remove(&key);
                    }
                    Some(v) => {
                        d.insert(key.clone(), v.clone());
                    }
                    None => {
                        d.remove(&key);
                    }
                }
            }
            if n == 0 {
                return Err("Nothing to change: select a symbol or a line.".into());
            }
        }
        Op::SetTag { id, tag } => {
            let t = crate::clip(tag.trim(), 60);
            find_item(p, id)?.tag = t;
        }
        Op::SetPts { id, pts } => {
            finite(&pts.iter().flatten().copied().collect::<Vec<_>>())?;
            find_line(p, id)?.pts = pts.into_iter().take(200).collect();
        }
        Op::Split { line, item } => created.extend(split(p, line, item)?),
        Op::Paste { sheet, items, lines, notes, dx, dy } => paste(p, sheet, items, lines, notes, dx, dy, created)?,
        Op::Link { a, b } => {
            for id in [a, b].into_iter().filter(|x| *x != 0) {
                if !p.item(id).and_then(|i| symbols::get(&i.sym)).is_some_and(|s| s.cat == Cat::Connector) {
                    return Err("Only off-page connectors can be paired.".into());
                }
            }
            if a == b {
                return Err("A connector cannot be its own partner.".into());
            }
            let old: Vec<u32> = [a, b].into_iter().filter_map(|x| p.item(x)).map(|i| i.link).collect();
            for it in p.items.iter_mut() {
                if old.contains(&it.id) || it.id == a || it.id == b {
                    it.link = 0;
                }
            }
            if b != 0 {
                find_item(p, a)?.link = b;
                find_item(p, b)?.link = a;
            }
        }
        Op::AddSheet { mut sheet } => {
            sheet.id = p.id();
            if sheet.number.trim().is_empty() || p.sheets.iter().any(|s| s.number == sheet.number) {
                let mut n = p.sheets.len() + 1;
                while p.sheets.iter().any(|s| s.number == format!("PID-{n:03}")) {
                    n += 1;
                }
                sheet.number = if sheet.legend { format!("PID-{n:03} (legend)") } else { format!("PID-{n:03}") };
            }
            created.push(sheet.id);
            p.sheets.push(sheet);
        }
        Op::SetSheet { sheet } => {
            let old = p.sheets.iter_mut().find(|s| s.id == sheet.id).ok_or("That sheet no longer exists.")?;
            *old = sheet;
        }
        Op::DeleteSheet { id } => {
            if p.sheets.len() < 2 {
                return Err("A project keeps at least one sheet.".into());
            }
            let ids: Vec<u32> = p.items.iter().filter(|i| i.sheet == id).map(|i| i.id).chain(p.lines.iter().filter(|l| l.sheet == id).map(|l| l.id)).chain(p.notes.iter().filter(|n| n.sheet == id).map(|n| n.id)).collect();
            delete(p, &ids);
            p.sheets.retain(|s| s.id != id);
        }
        Op::MoveSheet { id, to } => {
            let i = p.sheets.iter().position(|s| s.id == id).ok_or("That sheet no longer exists.")?;
            let s = p.sheets.remove(i);
            p.sheets.insert(to.min(p.sheets.len()), s);
        }
        Op::SetTitle { title } => p.title = title,
        Op::SetProject { name, units } => {
            if units != "SI" && units != "US" {
                return Err("Units are SI or US.".into());
            }
            p.name = crate::clip(name.trim(), 200);
            p.units = units;
        }
        Op::SetSettings { settings } => p.settings = settings,
        Op::Renumber => {
            for (id, t) in tags::renumber(p) {
                find_item(p, id)?.tag = t;
            }
        }
        Op::Batch { ops } => {
            for o in ops {
                exec(p, o, created)?;
            }
        }
    }
    Ok(())
}

/// Deletes items, lines and notes. Lines on a deleted item keep their end where the port was; an
/// in-line item with one process line on each side gives way to one line again.
fn delete(p: &mut Project, ids: &[u32]) {
    let set: HashSet<u32> = ids.iter().copied().collect();
    // Join lines across deleted in-line items first.
    for &id in ids {
        let Some(it) = p.item(id).cloned() else { continue };
        let Some(s) = symbols::get(&it.sym) else { continue };
        if !s.inline {
            continue;
        }
        let net = graph::Net::new(p);
        let ports: Vec<u8> = s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| k as u8).collect();
        let (on0, on1) = (net.on_port(id, ports[0]).to_vec(), net.on_port(id, ports[1]).to_vec());
        if on0.len() != 1 || on1.len() != 1 || on0[0].0 == on1[0].0 || set.contains(&on0[0].0) || set.contains(&on1[0].0) {
            continue;
        }
        let ((l1, b1), (l2, b2)) = (on0[0], on1[0]);
        let (Some(a), Some(b)) = (p.line(l1).cloned(), p.line(l2).cloned()) else { continue };
        if a.kind != b.kind {
            continue;
        }
        // Not when the far end, or the kept line's own start, is on one of the two halves (a loop).
        let far = if b2 { &b.a } else { &b.b };
        let start = if b1 { &a.a } else { &a.b };
        if [far, start].iter().any(|e| matches!(e, End::Line { line, .. } if *line == l1 || *line == l2)) {
            continue;
        }
        // Keep l1, oriented to end at the item, and give it l2's far end; data only l2 had is kept.
        let mut keep = a.clone();
        for (k, v) in &b.data {
            keep.data.entry(k.clone()).or_insert_with(|| v.clone());
        }
        if !b1 {
            std::mem::swap(&mut keep.a, &mut keep.b);
            keep.flip = !keep.flip;
            keep.pts.reverse();
        }
        keep.b = if b2 { b.a.clone() } else { b.b.clone() };
        keep.pts.clear();
        for l in p.lines.iter_mut() {
            for e in [&mut l.a, &mut l.b] {
                if let End::Line { line, .. } = e {
                    if *line == l2 {
                        *line = l1;
                    }
                }
            }
        }
        if let Some(x) = p.lines.iter_mut().find(|l| l.id == l1) {
            *x = keep;
        }
        p.lines.retain(|l| l.id != l2);
    }
    // Ends on deleted items or lines stay where they were, open.
    let pos: HashMap<u32, Vec<[f64; 2]>> = p.items.iter().filter(|i| set.contains(&i.id)).map(|i| (i.id, (0..symbols::get(&i.sym).map_or(0, |s| s.ports.len())).map(|k| graph::port_pos(i, k as u8).map_or([i.x, i.y], |x| x.0)).collect())).collect();
    for l in p.lines.iter_mut() {
        for e in [&mut l.a, &mut l.b] {
            let to = match e {
                End::Port { item, port } if set.contains(item) => pos.get(item).and_then(|v| v.get(*port as usize)).copied(),
                End::Line { line, x, y } if set.contains(line) => Some([*x, *y]),
                _ => None,
            };
            if let Some([x, y]) = to {
                *e = End::Free { x, y };
            }
        }
    }
    for it in p.items.iter_mut() {
        if set.contains(&it.link) {
            it.link = 0;
        }
    }
    p.items.retain(|i| !set.contains(&i.id));
    p.lines.retain(|l| !set.contains(&l.id));
    p.notes.retain(|n| !set.contains(&n.id));
}

/// Puts an in-line item into a line: the item is turned to the line's direction and moved onto
/// it, the line is cut there, and the two halves join the item's ports (port "in" upstream).
/// Branches stay on the half they were on. Returns the new half's id.
pub fn split(p: &mut Project, line_id: u32, item_id: u32) -> Result<Vec<u32>, String> {
    let line = p.line(line_id).cloned().ok_or("That line no longer exists.")?;
    let it = p.item(item_id).cloned().ok_or("That symbol no longer exists.")?;
    let s = symbols::get(&it.sym).ok_or("There is no such symbol.")?;
    if !s.inline {
        return Err(format!("A {} does not go in a line.", s.name.to_lowercase()));
    }
    if line.sheet != it.sheet {
        return Err("The line is on another sheet.".into());
    }
    if [&line.a, &line.b].iter().any(|e| e.item().is_some_and(|x| x.0 == item_id)) {
        return Err("The symbol is already on that line.".into());
    }
    let routes = crate::view::routes(p, line.sheet, Some(item_id));
    let path = routes.get(&line_id).cloned().unwrap_or_else(|| vec![end_pt(p, &line.a), end_pt(p, &line.b)]);
    let (at, seg, _) = graph::nearest_on(&path, [it.x, it.y]);
    let (a, b) = (path[seg], path[seg + 1.min(path.len() - 1 - seg)]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let horiz = dx.abs() >= dy.abs();
    // Flow direction along this segment, then the turn that points port "in" → "out" along it.
    let sign = if horiz { dx.signum() } else { dy.signum() } * if line.flip { -1.0 } else { 1.0 };
    let rot = match (horiz, sign >= 0.0) {
        (true, true) => 0,
        (false, true) => 1,
        (true, false) => 2,
        (false, false) => 3,
    };
    let g = p.settings.grid.max(0.5);
    let (x, y) = if horiz { ((at[0] / g).round() * g, a[1]) } else { (a[0], (at[1] / g).round() * g) };
    let ports: Vec<u8> = s.ports.iter().enumerate().filter(|(_, x)| !x.signal).map(|(k, _)| k as u8).collect();
    // The upstream half is the one at `a`: it takes port "in" when the flow runs a → b.
    let (pa, pb) = if line.flip { (ports[1], ports[0]) } else { (ports[0], ports[1]) };
    {
        let item = find_item(p, item_id)?;
        item.x = x;
        item.y = y;
        item.rot = rot;
        item.flip = false;
        if let (Some(size), true) = (line.data.get("size").cloned(), s.cat.valve() || s.cat == Cat::Primary || s.cat == Cat::Fitting) {
            item.data.entry("size".into()).or_insert(size);
        }
    }
    let id = p.id();
    let mut second = line.clone();
    second.id = id;
    second.a = End::Port { item: item_id, port: pb };
    second.pts.clear();
    {
        let first = find_line(p, line_id)?;
        first.b = End::Port { item: item_id, port: pa };
        first.pts.clear();
    }
    // A reducer changes the size: the downstream half takes the next size down if it had one.
    p.lines.push(second);
    // Branches past the cut move to the second half.
    let cut = seg as f64 + if horiz { (x - a[0]) / dx.abs().max(1e-9) * dx.signum() } else { (y - a[1]) / dy.abs().max(1e-9) * dy.signum() }.clamp(0.0, 1.0);
    let moves: Vec<(u32, bool)> = p
        .lines
        .iter()
        .flat_map(|l| [(l.id, false, &l.a), (l.id, true, &l.b)])
        .filter_map(|(lid, b, e)| match e {
            End::Line { line, x, y } if *line == line_id => {
                let (_, k, _) = graph::nearest_on(&path, [*x, *y]);
                let (q0, q1) = (path[k], path[(k + 1).min(path.len() - 1)]);
                let len = (q1[0] - q0[0]).hypot(q1[1] - q0[1]).max(1e-9);
                let t = k as f64 + ((x - q0[0]).hypot(y - q0[1]) / len).clamp(0.0, 1.0);
                (t > cut).then_some((lid, b))
            }
            _ => None,
        })
        .collect();
    for (lid, b) in moves {
        let l = find_line(p, lid)?;
        if let End::Line { line, .. } = if b { &mut l.b } else { &mut l.a } {
            *line = id;
        }
    }
    Ok(vec![id])
}

#[allow(clippy::too_many_arguments)]
fn paste(p: &mut Project, sheet: u32, items: Vec<Item>, lines: Vec<Line>, notes: Vec<Note>, dx: f64, dy: f64, created: &mut Vec<u32>) -> Result<(), String> {
    finite(&[dx, dy])?;
    if p.sheet(sheet).is_none() {
        return Err("That sheet no longer exists.".into());
    }
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    let old = p.clone();
    for mut it in items {
        symbols::get(&it.sym).ok_or("There is no such symbol.")?;
        let new = p.id();
        map.insert(it.id, new);
        it.id = new;
        it.sheet = sheet;
        it.x += dx;
        it.y += dy;
        it.link = 0;
        it.tag = tags::bump(p, &it.tag, &taken);
        if !it.tag.is_empty() {
            taken.insert(it.tag.to_uppercase());
        }
        finite(&[it.x, it.y])?;
        created.push(new);
        p.items.push(it);
    }
    let line_ids: HashSet<u32> = lines.iter().map(|l| l.id).collect();
    let mut seqs: HashMap<String, String> = HashMap::new();
    let mut new_lines = vec![];
    for mut l in lines {
        let new = p.id();
        map.insert(l.id, new);
        l.id = new;
        new_lines.push(l);
    }
    for mut l in new_lines {
        for e in [&mut l.a, &mut l.b] {
            let here = end_pt(&old, e);
            *e = match e.clone() {
                End::Port { item, port } if map.contains_key(&item) => End::Port { item: map[&item], port },
                End::Line { line, x, y } if line_ids.contains(&line) => End::Line { line: map[&line], x: x + dx, y: y + dy },
                End::Free { x, y } => End::Free { x: x + dx, y: y + dy },
                _ => End::Free { x: here[0] + dx, y: here[1] + dy },
            };
        }
        for q in &mut l.pts {
            q[0] += dx;
            q[1] += dy;
        }
        l.sheet = sheet;
        if l.kind.process() {
            // One new sequence number per pasted pipeline: pieces that shared one still do.
            let old = l.data.get("seq").map(|v| v.text()).unwrap_or_default();
            let new = match seqs.get(&old) {
                Some(s) => s.clone(),
                None => {
                    let s = tags::next_seq(p, sheet);
                    if !old.is_empty() {
                        seqs.insert(old, s.clone());
                    }
                    s
                }
            };
            l.data.insert("seq".into(), Val::Text(new));
        }
        created.push(l.id);
        p.lines.push(l);
    }
    for mut n in notes {
        n.id = p.id();
        n.sheet = sheet;
        n.x += dx;
        n.y += dy;
        n.to = [n.to[0] + dx, n.to[1] + dy];
        created.push(n.id);
        p.notes.push(n);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn item(d: &mut Doc, sym: &str, x: f64, y: f64) -> u32 {
        let sheet = d.p.sheets[0].id;
        d.apply(Op::AddItem { item: Item { sheet, sym: sym.into(), x, y, ..Item::default() }, on: None }).unwrap()[0]
    }
    pub fn connect(d: &mut Doc, a: (u32, u8), b: (u32, u8)) -> u32 {
        let sheet = d.p.sheets[0].id;
        d.apply(Op::AddLine { line: Line { sheet, a: End::Port { item: a.0, port: a.1 }, b: End::Port { item: b.0, port: b.1 }, ..Line::default() } }).unwrap()[0]
    }

    #[test]
    fn add_tags_and_undo() {
        let mut d = Doc::new(Project::new());
        let p1 = item(&mut d, "pump_cent", 50.0, 50.0);
        let p2 = item(&mut d, "pump_cent", 80.0, 50.0);
        assert_eq!(d.p.item(p1).unwrap().tag, "P-101");
        assert_eq!(d.p.item(p2).unwrap().tag, "P-102");
        let sheet = d.p.sheets[0].id;
        let i = d.apply(Op::AddItem { item: Item { sheet, sym: "inst_field".into(), tag: "lt".into(), x: 10.0, y: 10.0, ..Item::default() }, on: None }).unwrap()[0];
        assert_eq!(d.p.item(i).unwrap().tag, "LT-101");
        d.undo();
        assert!(d.p.item(i).is_none());
        d.redo();
        assert!(d.p.item(i).is_some());
    }

    #[test]
    fn a_failed_edit_changes_nothing() {
        let mut d = Doc::new(Project::new());
        let v = item(&mut d, "vvessel", 50.0, 50.0);
        let before = d.p.clone();
        assert!(d.apply(Op::Batch { ops: vec![Op::Move { ids: vec![v], dx: 5.0, dy: 0.0 }, Op::SetTag { id: 999_999, tag: "X".into() }] }).is_err());
        assert_eq!(d.p, before);
        assert!(d.apply(Op::Move { ids: vec![v], dx: f64::NAN, dy: 0.0 }).is_err());
        assert!(d.apply(Op::SetLine { line: Line { id: 77, ..Line::default() } }).is_err());
        assert_eq!(d.p, before);
    }

    #[test]
    fn split_and_merge() {
        let mut d = Doc::new(Project::new());
        let t = item(&mut d, "vvessel", 50.0, 100.0);
        let v = item(&mut d, "vvessel", 250.0, 100.0);
        // From the first vessel's right lower nozzle to the second's left lower nozzle: a straight run.
        let l = connect(&mut d, (t, 5), (v, 3));
        d.apply(Op::SetData { ids: vec![l], key: "size".into(), val: Some(Val::Text("3".into())) }).unwrap();
        let sheet = d.p.sheets[0].id;
        let made = d.apply(Op::AddItem { item: Item { sheet, sym: "gate".into(), x: 150.0, y: 109.0, ..Item::default() }, on: Some(l) }).unwrap();
        let (gv, l2) = (made[0], made[1]);
        let g = d.p.item(gv).unwrap();
        assert_eq!((g.y, g.rot), (108.0, 0), "moved onto the line and turned along it");
        let first = d.p.line(l).unwrap();
        let second = d.p.line(l2).unwrap();
        assert_eq!(first.b, End::Port { item: gv, port: 0 });
        assert_eq!(second.a, End::Port { item: gv, port: 1 });
        assert_eq!(second.b, End::Port { item: v, port: 3 });
        assert_eq!(second.data.get("size"), Some(&Val::Text("3".into())), "both halves keep the line's data");
        assert_eq!(d.p.item(gv).unwrap().data.get("size"), Some(&Val::Text("3".into())), "the valve takes the line size");
        // One run (same line number through the valve).
        let net = graph::Net::new(&d.p);
        assert_eq!(net.run_of[&l], net.run_of[&l2]);
        // Deleting the valve joins the halves again.
        d.apply(Op::Delete { ids: vec![gv] }).unwrap();
        assert_eq!(d.p.lines.len(), 1);
        let one = &d.p.lines[0];
        assert_eq!((one.a.clone(), one.b.clone()), (End::Port { item: t, port: 5 }, End::Port { item: v, port: 3 }));
    }

    #[test]
    fn split_follows_the_flow_and_moves_branches() {
        let mut d = Doc::new(Project::new());
        let a = item(&mut d, "vvessel", 250.0, 100.0);
        let b = item(&mut d, "vvessel", 50.0, 100.0);
        // Drawn right to left: flow runs from x = 240 to x = 60.
        let l = connect(&mut d, (a, 3), (b, 5));
        let sheet = d.p.sheets[0].id;
        let br = d.apply(Op::AddLine { line: Line { sheet, a: End::Line { line: l, x: 90.0, y: 108.0 }, b: End::Free { x: 90.0, y: 150.0 }, ..Line::default() } }).unwrap()[0];
        let made = d.apply(Op::AddItem { item: Item { sheet, sym: "check".into(), x: 150.0, y: 108.0, ..Item::default() }, on: Some(l) }).unwrap();
        let g = d.p.item(made[0]).unwrap();
        assert_eq!(g.rot, 2, "the check valve points along the flow (towards −x)");
        // The branch at x = 90 is downstream of the cut, on the second half.
        assert_eq!(d.p.line(br).unwrap().a, End::Line { line: made[1], x: 90.0, y: 108.0 });
    }

    #[test]
    fn paste_bumps_tags_and_keeps_internal_lines() {
        let mut d = Doc::new(Project::new());
        let p1 = item(&mut d, "pump_cent", 50.0, 50.0);
        d.apply(Op::SetTag { id: p1, tag: "P-101A".into() }).unwrap();
        let v = item(&mut d, "check", 80.0, 44.0);
        let l = connect(&mut d, (p1, 1), (v, 0));
        let sheet = d.p.sheets[0].id;
        let (items, lines) = (vec![d.p.item(p1).unwrap().clone(), d.p.item(v).unwrap().clone()], vec![d.p.line(l).unwrap().clone()]);
        let made = d.apply(Op::Paste { sheet, items, lines, notes: vec![], dx: 0.0, dy: 40.0 }).unwrap();
        assert_eq!(d.p.item(made[0]).unwrap().tag, "P-101B");
        let nl = d.p.line(made[2]).unwrap();
        assert_eq!(nl.a, End::Port { item: made[0], port: 1 });
        assert_eq!(nl.b, End::Port { item: made[1], port: 0 });
    }

    #[test]
    fn offpage_pairs_and_sheets() {
        let mut d = Doc::new(Project::new());
        let s2 = d.apply(Op::AddSheet { sheet: Sheet::default() }).unwrap()[0];
        assert_eq!(d.p.sheet(s2).unwrap().number, "PID-002");
        let a = item(&mut d, "offpage", 380.0, 100.0);
        let b = d.apply(Op::AddItem { item: Item { sheet: s2, sym: "offpage".into(), x: 30.0, y: 100.0, ..Item::default() }, on: None }).unwrap()[0];
        d.apply(Op::Link { a, b }).unwrap();
        assert_eq!((d.p.item(a).unwrap().link, d.p.item(b).unwrap().link), (b, a));
        assert!(d.apply(Op::Link { a, b: a }).is_err());
        d.apply(Op::DeleteSheet { id: s2 }).unwrap();
        assert_eq!(d.p.item(a).unwrap().link, 0);
        assert!(d.apply(Op::DeleteSheet { id: d.p.sheets[0].id }).is_err());
    }
}

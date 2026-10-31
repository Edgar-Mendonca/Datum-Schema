//! Review comments: notes from reviewers pinned to a sheet, with replies, resolved or open. Each
//! comment and reply has a unique id, so the comments made on several copies of a project (one
//! per reviewer) can be merged back into one without doubles.

use crate::doc::Op;
use crate::model::Project;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Reply {
    pub uid: String,
    pub by: String,
    /// Seconds since 1970 (UTC).
    pub time: u64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Comment {
    pub uid: String,
    pub sheet: u32,
    pub x: f64,
    pub y: f64,
    pub by: String,
    pub time: u64,
    pub text: String,
    /// The symbols, lines or notes it is about.
    pub on: Vec<u32>,
    pub replies: Vec<Reply>,
    pub resolved: bool,
    pub resolved_by: String,
    /// When it was last resolved or reopened (the latest wins when copies are merged).
    pub status_time: u64,
}

const MAX_COMMENTS: usize = 5000;
const MAX_REPLIES: usize = 200;
const MAX_TEXT: usize = 4000;

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A date and time as the lists show it: "2026-10-31 14:05" (UTC).
pub fn when(t: u64) -> String {
    if t == 0 {
        return String::new();
    }
    // Days to a civil date (Howard Hinnant's algorithm).
    let days = (t / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let s = t % 86_400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", s / 3600, s / 60 % 60)
}

fn text_of(t: &str) -> Result<String, String> {
    let t = t.trim();
    if t.is_empty() {
        return Err("Type the comment first.".into());
    }
    Ok(crate::clip(t, MAX_TEXT))
}

fn find<'a>(p: &'a mut Project, uid: &str) -> Result<&'a mut Comment, String> {
    p.comments.iter_mut().find(|c| c.uid == uid).ok_or_else(|| "That comment no longer exists.".into())
}

/// The review edits.
pub fn exec(p: &mut Project, op: Op) -> Result<(), String> {
    match op {
        Op::Comment { sheet, x, y, text, on, by } => {
            if p.sheet(sheet).is_none() {
                return Err("That sheet no longer exists.".into());
            }
            if !x.is_finite() || !y.is_finite() {
                return Err("That is not a place on the sheet.".into());
            }
            let uid = crate::model::uid(p.comments.len() as u32, &text);
            p.comments.push(Comment { uid, sheet, x, y, by: crate::clip(by.trim(), 80), time: now(), text: text_of(&text)?, on, ..Comment::default() });
        }
        Op::Reply { uid, text, by } => {
            let t = text_of(&text)?;
            let c = find(p, &uid)?;
            let r = crate::model::uid(c.replies.len() as u32, &t);
            c.replies.push(Reply { uid: r, by: crate::clip(by.trim(), 80), time: now(), text: t });
        }
        Op::Resolve { uid, resolved, by } => {
            let c = find(p, &uid)?;
            c.resolved = resolved;
            c.resolved_by = if resolved { crate::clip(by.trim(), 80) } else { String::new() };
            c.status_time = now();
        }
        Op::EditComment { uid, text } => find(p, &uid)?.text = text_of(&text)?,
        Op::MoveComment { uid, x, y } => {
            if !x.is_finite() || !y.is_finite() {
                return Err("That is not a place on the sheet.".into());
            }
            let c = find(p, &uid)?;
            (c.x, c.y) = (x, y);
        }
        Op::DeleteComment { uid } => {
            let n = p.comments.len();
            p.comments.retain(|c| c.uid != uid);
            if p.comments.len() == n {
                return Err("That comment no longer exists.".into());
            }
        }
        Op::MergeComments { comments } => {
            merge(p, comments);
        }
        _ => return Err("Not a review edit.".into()),
    }
    Ok(())
}

/// What a merge brought in.
#[derive(Clone, Debug, Serialize, PartialEq, Default)]
pub struct Merged {
    pub comments: usize,
    pub replies: usize,
    pub statuses: usize,
}

/// Takes in the comments of another copy of the project: new comments, new replies, and a
/// resolved or reopened status that is newer than ours. Comments on a sheet this copy does not
/// have go to the sheet with the same number, else the first sheet.
pub fn merge(p: &mut Project, theirs: Vec<Comment>) -> Merged {
    let mut m = Merged::default();
    let first = p.sheets.first().map_or(0, |s| s.id);
    for mut c in theirs.into_iter().take(MAX_COMMENTS) {
        if c.uid.trim().is_empty() {
            continue;
        }
        match p.comments.iter_mut().find(|x| x.uid == c.uid) {
            Some(ours) => {
                for r in c.replies {
                    if !r.uid.is_empty() && !ours.replies.iter().any(|x| x.uid == r.uid) {
                        ours.replies.push(r);
                        m.replies += 1;
                    }
                }
                ours.replies.sort_by_key(|r| r.time);
                if c.status_time > ours.status_time {
                    if c.resolved != ours.resolved {
                        m.statuses += 1;
                    }
                    ours.resolved = c.resolved;
                    ours.resolved_by = c.resolved_by;
                    ours.status_time = c.status_time;
                }
            }
            None => {
                if p.sheet(c.sheet).is_none() {
                    c.sheet = first;
                }
                // Things it points at that are not in this copy are let go.
                c.on.retain(|id| p.items.iter().any(|i| i.id == *id) || p.lines.iter().any(|l| l.id == *id) || p.notes.iter().any(|n| n.id == *id));
                p.comments.push(c);
                m.comments += 1;
            }
        }
    }
    p.comments.sort_by_key(|c| c.time);
    m
}

/// Comments made sound: on a sheet that exists, at a finite place, with clipped text, pointing
/// only at things that exist; one per uid.
pub fn normalize(p: &mut Project) {
    let first = p.sheets.first().map_or(0, |s| s.id);
    let sheets: std::collections::HashSet<u32> = p.sheets.iter().map(|s| s.id).collect();
    let ids: std::collections::HashSet<u32> = p.items.iter().map(|i| i.id).chain(p.lines.iter().map(|l| l.id)).chain(p.notes.iter().map(|n| n.id)).collect();
    let mut seen = std::collections::HashSet::new();
    p.comments.truncate(MAX_COMMENTS);
    p.comments.retain(|c| !c.uid.trim().is_empty() && seen.insert(c.uid.clone()));
    for c in &mut p.comments {
        if !sheets.contains(&c.sheet) {
            c.sheet = first;
        }
        if !c.x.is_finite() || !c.y.is_finite() {
            (c.x, c.y) = (0.0, 0.0);
        }
        c.x = c.x.clamp(-crate::model::MAX_COORD, crate::model::MAX_COORD);
        c.y = c.y.clamp(-crate::model::MAX_COORD, crate::model::MAX_COORD);
        c.text = crate::clip(&c.text, MAX_TEXT);
        c.by = crate::clip(&c.by, 80);
        c.resolved_by = crate::clip(&c.resolved_by, 80);
        c.uid = crate::clip(&c.uid, 64);
        c.on.retain(|i| ids.contains(i));
        c.on.truncate(50);
        c.replies.truncate(MAX_REPLIES);
        for r in &mut c.replies {
            r.text = crate::clip(&r.text, MAX_TEXT);
            r.by = crate::clip(&r.by, 80);
            r.uid = crate::clip(&r.uid, 64);
        }
    }
}

/// A comment's pin on a sheet, as the window draws it: its number (in the project's order).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Pin {
    pub uid: String,
    pub n: usize,
    pub x: f64,
    pub y: f64,
    pub resolved: bool,
}

pub fn pins(p: &Project, sheet: u32) -> Vec<Pin> {
    p.comments.iter().enumerate().filter(|(_, c)| c.sheet == sheet).map(|(i, c)| Pin { uid: c.uid.clone(), n: i + 1, x: c.x, y: c.y, resolved: c.resolved }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    #[test]
    fn dates_read_as_dates() {
        assert_eq!(when(0), "");
        assert_eq!(when(1_793_457_900), "2026-10-31 14:45");
        assert_eq!(when(951_782_400), "2000-02-29 00:00");
    }

    #[test]
    fn reviewers_comment_reply_resolve_and_merge() {
        let base = crate::examples::get("pump").unwrap();
        let s = base.sheets[0].id;
        let v = base.items.iter().find(|i| base.tag_of(i) == "V-101").unwrap().id;
        // The engineer's copy and a reviewer's copy of the same project.
        let mut a = Doc::new(base.clone());
        a.user = "EM".into();
        a.apply(Op::Comment { sheet: s, x: 100.0, y: 80.0, text: "Check the relief valve size".into(), on: vec![v], by: String::new() }).unwrap();
        assert!(a.apply(Op::Comment { sheet: s, x: 1.0, y: 1.0, text: "  ".into(), on: vec![], by: String::new() }).is_err());
        let first = a.p.comments[0].clone();
        assert_eq!((first.by.as_str(), first.on.as_slice()), ("EM", &[v][..]));
        let mut b = Doc::new(a.p.clone());
        b.user = "JS".into();
        b.apply(Op::Reply { uid: first.uid.clone(), text: "Sized for fire case; see calc 17".into(), by: String::new() }).unwrap();
        b.apply(Op::Resolve { uid: first.uid.clone(), resolved: true, by: String::new() }).unwrap();
        b.apply(Op::Comment { sheet: s, x: 200.0, y: 50.0, text: "Add a drain on the pump casing".into(), on: vec![], by: String::new() }).unwrap();
        // Meanwhile the engineer replies too.
        a.apply(Op::Reply { uid: first.uid.clone(), text: "Thanks".into(), by: String::new() }).unwrap();
        // The reviewer's copy comes back: one new comment, one new reply, the status.
        let m = merge(&mut a.p.clone(), b.p.comments.clone());
        assert_eq!(m, Merged { comments: 1, replies: 1, statuses: 1 });
        a.apply(Op::MergeComments { comments: b.p.comments.clone() }).unwrap();
        assert_eq!(a.p.comments.len(), 2);
        let c = &a.p.comments[0];
        assert!(c.resolved && c.resolved_by == "JS" && c.replies.len() == 2);
        // Merging again changes nothing; undo takes the merge back.
        let before = a.p.clone();
        a.apply(Op::MergeComments { comments: b.p.comments.clone() }).unwrap();
        assert_eq!(a.p, before);
        a.undo();
        assert_eq!(a.p.comments.len(), 1);
        a.redo();
        // Pins, the list, and deleting what a comment points at.
        assert_eq!(pins(&a.p, s).iter().map(|p| p.n).collect::<Vec<_>>(), [1, 2]);
        let l = crate::lists::build(&a.p, "comments").unwrap();
        assert_eq!(l.rows.len(), 2);
        assert!(l.rows[0].cells.iter().any(|x| x == "Resolved (JS)"));
        a.apply(Op::Delete { ids: vec![v], keep: false }).unwrap();
        assert!(a.p.comments[0].on.is_empty());
        // Saved and opened again.
        let path = std::env::temp_dir().join(format!("dschema-review-{}.dschema", std::process::id()));
        crate::file::write(&path, &a.p, &[]).unwrap();
        let (q, _, w) = crate::file::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(q.comments, a.p.comments);
        // Hostile input is made sound.
        let mut h = a.p.clone();
        h.comments.push(Comment { uid: first.uid.clone(), ..Comment::default() });
        h.comments.push(Comment { uid: "x".into(), sheet: 999_999, x: f64::NAN, text: "<script>".repeat(2000), ..Comment::default() });
        h.normalize();
        assert_eq!(h.comments.len(), 3);
        let x = h.comments.iter().find(|c| c.uid == "x").unwrap();
        assert!(x.sheet == h.sheets[0].id && x.x == 0.0 && x.text.chars().count() <= MAX_TEXT);
        a.apply(Op::DeleteComment { uid: first.uid }).unwrap();
        assert_eq!(a.p.comments.len(), 1);
    }
}

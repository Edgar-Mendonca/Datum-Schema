//! Lists and reports read back from Excel: a list exported to Excel carries a last column naming
//! the list and each row's objects ("Datum id (lines)"), so the workbook can be edited and read
//! back. Only the columns the list lets you edit are read; each changed cell becomes a change in
//! the plan (numbers in the column's unit, or in the unit typed with them), shown before anything
//! changes. Rows added or removed in the workbook are not made or deleted.

use crate::import::{norm, quantity, Change, Plan};
use crate::model::{Project, Val};
use crate::units;

/// The id column's header for a list kind.
pub fn id_header(kind: &str) -> String {
    format!("Datum id ({kind})")
}

/// The list kind an id column's header names.
fn kind_of(h: &str) -> Option<String> {
    let h = h.trim();
    let rest = h.strip_prefix("Datum id (")?.strip_suffix(')')?;
    (!rest.is_empty()).then(|| rest.to_string())
}

/// What reading an edited list back would change.
pub fn plan(p: &Project, bytes: &[u8]) -> Result<Plan, String> {
    let book = crate::import::read(bytes)?;
    let us = p.units == "US";
    let mut plan = Plan { sheets: book.len(), ..Plan::default() };
    for (sheet, grid) in &book {
        // The header row: the one with the id column.
        let Some((hr, idc, kind)) = grid.iter().take(20).enumerate().find_map(|(i, r)| r.iter().enumerate().find_map(|(c, h)| kind_of(h).map(|k| (i, c, k)))) else { continue };
        let list = crate::lists::build(p, &kind).map_err(|e| format!("{e} (worksheet {sheet})"))?;
        plan.used += 1;
        let head = &grid[hr];
        // Columns by their label as exported (unit included), else by label without the unit.
        let col_of = |h: &str| list.cols.iter().position(|c| c.label == h.trim()).or_else(|| list.cols.iter().position(|c| norm(&c.label).0 == norm(h).0));
        let cols: Vec<(usize, usize)> = head.iter().enumerate().filter(|(c, _)| *c != idc).filter_map(|(c, h)| col_of(h).map(|k| (c, k))).collect();
        for h in head.iter().enumerate().filter(|(c, h)| *c != idc && !h.trim().is_empty() && col_of(h).is_none()).map(|x| x.1) {
            if !plan.unknown.contains(h) {
                plan.unknown.push(crate::clip(h.trim(), 80));
            }
        }
        for row in grid.iter().skip(hr + 1) {
            let ids: Vec<u32> = row.get(idc).map_or(vec![], |s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect());
            if ids.is_empty() {
                continue;
            }
            let Some(cur) = list.rows.iter().find(|r| r.ids.first() == ids.first()) else {
                plan.unmatched.push(row.first().cloned().unwrap_or_else(|| ids[0].to_string()));
                continue;
            };
            let who = cur.cells.first().cloned().unwrap_or_default();
            for &(c, k) in &cols {
                let col = &list.cols[k];
                if !col.edit {
                    continue;
                }
                let text = row.get(c).map_or("", |s| s.trim());
                let now = cur.cells.get(k).map_or("", |s| s.trim());
                let mut ch = Change { obj: ids[0], tag: who.clone(), key: col.key.clone(), label: col.label.clone(), old: now.to_string(), new: text.to_string(), val: None, kind: String::new(), note: String::new(), sheet: sheet.clone() };
                if col.q.is_empty() {
                    if text == now {
                        continue;
                    }
                    ch.kind = "set".into();
                    ch.val = (!text.is_empty()).then(|| Val::Text(crate::clip(text, 1000)));
                    if col.key == "tag" && text.is_empty() {
                        (ch.kind, ch.note) = ("bad".into(), "A tag cannot be emptied here.".into());
                    }
                } else {
                    // Numbers: compared as numbers in the column's unit (the export wrote the shown value).
                    let unit = norm(&col.label).1;
                    if text.is_empty() {
                        if now.is_empty() {
                            continue;
                        }
                        ch.kind = "set".into();
                    } else {
                        match quantity(text, &unit, col.q, us) {
                            Ok(si) => {
                                let shown = units::to_display(si, col.q, us);
                                let same = now.parse::<f64>().is_ok_and(|n| (n - shown).abs() <= 1e-3 * n.abs().max(shown.abs()).max(1e-9));
                                if same {
                                    continue;
                                }
                                ch.new = units::show(si, col.q, us);
                                ch.kind = "set".into();
                                ch.val = Some(Val::Num(si));
                            }
                            Err(e) => (ch.kind, ch.note) = ("bad".into(), e),
                        }
                    }
                }
                plan.changes.push(ch);
            }
        }
    }
    if plan.used == 0 {
        return Err("This workbook is not a list exported from Datum Schema: its last column, Datum id, is missing. Export the list to Excel, edit it, and read that back.".into());
    }
    Ok(plan)
}

/// The edit for one change of a plan (a tag, or a data value).
pub fn op(c: &Change) -> crate::doc::Op {
    if c.key == "tag" {
        crate::doc::Op::SetTag { id: c.obj, tag: c.val.as_ref().map(|v| v.text()).unwrap_or_default() }
    } else {
        crate::doc::Op::SetData { ids: vec![c.obj], key: c.key.clone(), val: c.val.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    /// A list exported, edited cell by cell, and written back to a workbook.
    fn edited(p: &Project, kind: &str, edit: &dyn Fn(&str, &mut String)) -> Vec<u8> {
        let book = crate::lists::xlsx(p, kind).unwrap();
        let grid = crate::import::read(&book).unwrap().remove(0).1;
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet();
        let head = grid.iter().position(|r| r.iter().any(|c| c.starts_with("Datum id"))).unwrap();
        for (r, row) in grid.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let mut v = v.clone();
                if r > head {
                    edit(&format!("{}|{}", grid[head][c], row[0]), &mut v);
                }
                ws.write_string(r as u32, c as u16, &v).unwrap();
            }
        }
        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn a_line_list_edited_in_excel_comes_back() {
        let p = crate::examples::get("pump").unwrap();
        // Nothing edited: nothing to change.
        let same = edited(&p, "lines", &|_, _| {});
        let pl = plan(&p, &same).unwrap();
        assert!(pl.changes.is_empty(), "{:#?}", pl.changes);
        // A design pressure, a service and an insulation typed in Excel; a velocity (not editable) ignored.
        let book = edited(&p, "lines", &|k, v| {
            if k.starts_with("Design P") && k.ends_with("1003-A1A") {
                *v = "12".into();
            }
            if k.starts_with("Service|") && k.ends_with("1001-A1A") {
                *v = "Tank outlet header".into();
            }
            if k.starts_with("Velocity") {
                *v = "99".into();
            }
        });
        let pl = plan(&p, &book).unwrap();
        let sets: Vec<&Change> = pl.changes.iter().filter(|c| c.kind == "set").collect();
        assert_eq!(sets.len(), 2, "{:#?}", pl.changes);
        let dp = sets.iter().find(|c| c.key == "design_p").unwrap();
        assert!(matches!(dp.val, Some(Val::Num(x)) if (x - 12e5).abs() < 1e-6));
        // Applied: one undoable edit; read again, nothing left.
        let mut d = Doc::new(p);
        d.apply(Op::Batch { ops: sets.iter().map(|c| op(c)).collect() }).unwrap();
        assert!(plan(&d.p, &book).unwrap().changes.is_empty());
    }

    #[test]
    fn tags_reports_and_bad_values() {
        let mut p = crate::examples::get("pump").unwrap();
        let book = edited(&p, "valves", &|k, v| {
            if k.starts_with("Tag|HV-104") {
                *v = "HV-904".into();
            }
            if k.starts_with("Fail") && k.ends_with("LV-101") {
                *v = "FO".into();
            }
        });
        let pl = plan(&p, &book).unwrap();
        let tag = pl.changes.iter().find(|c| c.key == "tag").unwrap();
        assert_eq!(tag.new, "HV-904");
        let mut d = Doc::new(p.clone());
        d.apply(Op::Batch { ops: pl.changes.iter().filter(|c| c.kind == "set").map(op).collect() }).unwrap();
        assert!(d.p.objects.iter().any(|o| o.tag == "HV-904"));
        // A number that is not one is refused, with why.
        let book = edited(&p, "lines", &|k, v| {
            if k.starts_with("Design P") && k.ends_with("1003-A1A") {
                *v = "twelve".into();
            }
        });
        assert!(plan(&p, &book).unwrap().changes.iter().any(|c| c.kind == "bad" && c.note.contains("not a number")));
        // A report comes back the same way.
        p.settings.reports = vec![crate::reports::Report { name: "Pumps".into(), source: "class:Pump".into(), ..Default::default() }];
        let book = edited(&p, "report:Pumps", &|k, v| {
            if k.starts_with("Name or service|P-101A") {
                *v = "Charge pump A".into();
            }
        });
        let pl = plan(&p, &book).unwrap();
        assert!(pl.changes.iter().any(|c| c.key == "service" && c.new == "Charge pump A"), "{:#?}", pl.changes);
        // Not one of ours.
        assert!(plan(&p, &crate::datasheets::xlsx(&p).unwrap()).is_err());
        assert!(plan(&p, b"nonsense").is_err());
    }
}

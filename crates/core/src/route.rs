//! Orthogonal line routing around symbols, copied from Datum Sim's wire router and extended to ports
//! that face any of four directions (and to open ends with no direction). For each line, a sparse
//! grid is built from the interesting coordinates near it (symbol edges with a margin, the port
//! positions and their stubs), and the cheapest path is found with length + a penalty per bend + a
//! penalty for running along another line. Bend points the user placed are kept: the line goes
//! through them. Units are sheet millimetres.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

pub type Pt = (f64, f64);

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One line to route: from `a` (leaving its port along `dir_a`) to `b` (whose port faces `dir_b`;
/// the line arrives moving against it). `None`: an open end or a branch point, no stub.
pub struct Req {
    pub a: Pt,
    pub dir_a: Option<Pt>,
    pub b: Pt,
    pub dir_b: Option<Pt>,
    pub pts: Vec<Pt>,
    /// Lines of the same group may share segments.
    pub group: u64,
}

const STUB: f64 = 3.0;
const MARGIN: f64 = 3.5;
const BEND: f64 = 6.0;
const SHARE: f64 = 8.0;

fn inside(r: &Rect, x: f64, y: f64, pad: f64) -> bool {
    x > r.x - pad && x < r.x + r.w + pad && y > r.y - pad && y < r.y + r.h + pad
}

/// Does the segment cross a block (inflated by pad)?
fn blocked(rs: &[Rect], a: Pt, b: Pt, pad: f64) -> bool {
    rs.iter().any(|r| {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        x1 > r.x - pad && x0 < r.x + r.w + pad && y1 > r.y - pad && y0 < r.y + r.h + pad
    })
}

#[derive(PartialEq)]
struct St(f64, usize, u8);
impl Eq for St {}
impl Ord for St {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for St {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

type Used = HashMap<(i64, i64, i64, i64), u64>;

fn key(a: Pt, b: Pt) -> (i64, i64, i64, i64) {
    let (a, b) = if (a.0, a.1) <= (b.0, b.1) { (a, b) } else { (b, a) };
    (a.0.round() as i64, a.1.round() as i64, b.0.round() as i64, b.1.round() as i64)
}

/// Route from s to e (both outside blocks) on a sparse grid; None if there is no way.
fn search(rs: &[Rect], s: Pt, e: Pt, used: &Used, group: u64, window: f64) -> Option<Vec<Pt>> {
    let (wx0, wx1) = (s.0.min(e.0) - window, s.0.max(e.0) + window);
    let (wy0, wy1) = (s.1.min(e.1) - window, s.1.max(e.1) + window);
    let near: Vec<&Rect> = rs.iter().filter(|r| r.x + r.w > wx0 && r.x < wx1 && r.y + r.h > wy0 && r.y < wy1).collect();
    let mut xs = vec![s.0, e.0, (s.0 + e.0) / 2.0];
    let mut ys = vec![s.1, e.1, (s.1 + e.1) / 2.0];
    for r in &near {
        xs.extend([r.x - MARGIN, r.x + r.w + MARGIN]);
        ys.extend([r.y - MARGIN, r.y + r.h + MARGIN]);
    }
    for v in [&mut xs, &mut ys] {
        v.retain(|x| x.is_finite());
        v.sort_by(|a, b| a.total_cmp(b));
        v.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    }
    let (nx, ny) = (xs.len(), ys.len());
    if nx * ny > 400_000 {
        return None;
    }
    let near: Vec<Rect> = near.into_iter().cloned().collect();
    let pad = STUB - 1.0;
    let id = |i: usize, j: usize| i * ny + j;
    let free: Vec<bool> = (0..nx * ny).map(|k| !near.iter().any(|r| inside(r, xs[k / ny], ys[k % ny], pad))).collect();
    let find = |p: Pt| -> Option<usize> {
        let i = xs.iter().position(|x| (x - p.0).abs() < 0.5)?;
        let j = ys.iter().position(|y| (y - p.1).abs() < 0.5)?;
        Some(id(i, j))
    };
    let (si, ei) = (find(s)?, find(e)?);
    // Dijkstra over (node, direction): 0 +x, 1 −x, 2 +y, 3 −y, 4 none yet.
    let mut dist: HashMap<(usize, u8), f64> = HashMap::new();
    let mut prev: HashMap<(usize, u8), (usize, u8)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert((si, 4), 0.0);
    heap.push(St(0.0, si, 4));
    let mut end = None;
    while let Some(St(d, k, dir)) = heap.pop() {
        if d > *dist.get(&(k, dir)).unwrap_or(&f64::INFINITY) {
            continue;
        }
        if k == ei {
            end = Some((k, dir));
            break;
        }
        let (i, j) = (k / ny, k % ny);
        let nbrs: [(Option<usize>, u8); 4] = [
            ((i + 1 < nx).then(|| id(i + 1, j)), 0),
            ((i > 0).then(|| id(i - 1, j)), 1),
            ((j + 1 < ny).then(|| id(i, j + 1)), 2),
            ((j > 0).then(|| id(i, j - 1)), 3),
        ];
        for (n, nd) in nbrs {
            let Some(n) = n else { continue };
            if !free[n] && n != ei {
                continue;
            }
            let (a, b) = ((xs[i], ys[j]), (xs[n / ny], ys[n % ny]));
            if blocked(&near, a, b, pad) {
                continue;
            }
            let mut c = d + (a.0 - b.0).abs() + (a.1 - b.1).abs();
            if dir != 4 && dir != nd {
                c += BEND;
            }
            if let Some(g) = used.get(&key(a, b)) {
                if *g != group {
                    c += SHARE;
                }
            }
            if c < *dist.get(&(n, nd)).unwrap_or(&f64::INFINITY) {
                dist.insert((n, nd), c);
                prev.insert((n, nd), (k, dir));
                heap.push(St(c, n, nd));
            }
        }
    }
    let mut cur = end?;
    let mut path = vec![(xs[cur.0 / ny], ys[cur.0 % ny])];
    while let Some(p) = prev.get(&cur) {
        cur = *p;
        path.push((xs[cur.0 / ny], ys[cur.0 % ny]));
    }
    path.reverse();
    Some(path)
}

/// Keep only the corners of a polyline.
fn simplify(p: Vec<Pt>) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(p.len());
    for q in p {
        if out.last().is_some_and(|l| (l.0 - q.0).abs() < 0.01 && (l.1 - q.1).abs() < 0.01) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let col = ((a.0 - b.0).abs() < 0.01 && (b.0 - q.0).abs() < 0.01) || ((a.1 - b.1).abs() < 0.01 && (b.1 - q.1).abs() < 0.01);
            if col {
                out.pop();
            }
        }
        out.push(q);
    }
    out
}

/// A simple three-segment path when nothing better is found.
fn fallback(s: Pt, e: Pt) -> Vec<Pt> {
    let mx = (s.0 + e.0) / 2.0;
    vec![s, (mx, s.1), (mx, e.1), e]
}

/// Route every wire; returns one polyline per request.
pub fn route(rs: &[Rect], reqs: &[Req]) -> Vec<Vec<Pt>> {
    let mut used: Used = HashMap::new();
    let mut out = Vec::with_capacity(reqs.len());
    for r in reqs {
        let s = r.dir_a.map_or(r.a, |d| (r.a.0 + d.0 * STUB, r.a.1 + d.1 * STUB));
        let e = r.dir_b.map_or(r.b, |d| (r.b.0 + d.0 * STUB, r.b.1 + d.1 * STUB));
        let mut path = vec![r.a];
        if !r.pts.is_empty() {
            // Through the user's bend points: along the leaving direction first, then across.
            let mut prev = s;
            let mut horiz = r.dir_a.is_none_or(|d| d.1.abs() < 0.5);
            path.push(s);
            for p in r.pts.iter().chain(std::iter::once(&e)) {
                path.push(if horiz { (p.0, prev.1) } else { (prev.0, p.1) });
                path.push(*p);
                prev = *p;
                horiz = !horiz;
            }
        } else if ((s.0 - e.0).abs() < 1e-6 || (s.1 - e.1).abs() < 1e-6) && !blocked(rs, s, e, -0.5) {
            path.push(s);
            path.push(e);
        } else {
            let found = search(rs, s, e, &used, r.group, 30.0).or_else(|| search(rs, s, e, &used, r.group, 200.0));
            path.extend(found.unwrap_or_else(|| fallback(s, e)));
        }
        path.push(r.b);
        let path = simplify(path);
        for w in path.windows(2) {
            used.entry(key(w[0], w[1])).or_insert(r.group);
        }
        out.push(path);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(rs: &[Rect], p: &[Pt]) -> bool {
        p.windows(2).any(|w| blocked(rs, w[0], w[1], -0.2))
    }
    fn ortho(p: &[Pt]) -> bool {
        p.windows(2).all(|w| (w[0].0 - w[1].0).abs() < 1e-9 || (w[0].1 - w[1].1).abs() < 1e-9)
    }

    #[test]
    fn goes_around_a_symbol() {
        let rs = [Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 }, Rect { x: 60.0, y: 0.0, w: 10.0, h: 10.0 }, Rect { x: 30.0, y: -5.0, w: 10.0, h: 20.0 }];
        let r = Req { a: (10.0, 5.0), dir_a: Some((1.0, 0.0)), b: (60.0, 5.0), dir_b: Some((-1.0, 0.0)), pts: vec![], group: 1 };
        let p = &route(&rs, &[r])[0];
        assert!(!hits(&rs[2..], p), "{p:?}");
        assert_eq!(p[0], (10.0, 5.0));
        assert_eq!(*p.last().unwrap(), (60.0, 5.0));
        assert!(ortho(p), "orthogonal: {p:?}");
    }

    #[test]
    fn straight_when_clear_and_ports_facing_down() {
        let rs = [Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 }, Rect { x: 60.0, y: 0.0, w: 10.0, h: 10.0 }];
        let r = Req { a: (10.0, 5.0), dir_a: Some((1.0, 0.0)), b: (60.0, 5.0), dir_b: Some((-1.0, 0.0)), pts: vec![], group: 1 };
        assert_eq!(route(&rs, &[r])[0], vec![(10.0, 5.0), (60.0, 5.0)]);
        // From the bottom of one symbol to the left of another below and to the right.
        let r = Req { a: (5.0, 10.0), dir_a: Some((0.0, 1.0)), b: (60.0, 40.0), dir_b: Some((-1.0, 0.0)), pts: vec![], group: 2 };
        let p = &route(&rs, &[r])[0];
        assert!(ortho(p) && !hits(&rs, p), "{p:?}");
        assert!(p[1].1 > 10.0 && (p[1].0 - 5.0).abs() < 1e-9, "leaves downwards: {p:?}");
    }

    #[test]
    fn through_bend_points_and_to_open_ends() {
        let r = Req { a: (0.0, 0.0), dir_a: None, b: (40.0, 30.0), dir_b: None, pts: vec![(20.0, 10.0)], group: 1 };
        let p = &route(&[], &[r])[0];
        let on = |q: Pt| p.windows(2).any(|w| (w[0].0 - q.0) * (w[1].1 - q.1) == (w[1].0 - q.0) * (w[0].1 - q.1) && q.0 >= w[0].0.min(w[1].0) && q.0 <= w[0].0.max(w[1].0) && q.1 >= w[0].1.min(w[1].1) && q.1 <= w[0].1.max(w[1].1));
        assert!(ortho(p) && on((20.0, 10.0)), "{p:?}");
    }
}

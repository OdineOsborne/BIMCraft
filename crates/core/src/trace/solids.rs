//! What a plan draws standing in a building that is no wall: a pillar, drawn as a closed box of its own four sides
//! standing free, and the mass a wall's faces turn round — a flue in the corner of a room, its stack standing out of
//! the outer face — which is no doorway in the wall, though the wall's faces stop where it stands.
//!
//! A pillar drawn as a box pairs as a wall does: on the permesso di costruire this was written against, a pillar
//! 400 square in a cellar, under the corner of the floor above, paired its lower side with the outline of that floor
//! drawn in red over its upper side, and the cellar got a wall 400 thick from the pillar to the next wall, with a
//! door in it. So a closed box no bigger than [`PILLAR`] a side, standing free and with nothing drawn inside it, is a
//! pillar ([`pillars`]), and its four sides are no faces. What stands free is what the drawing does not carry on past
//! it: the pier between two openings is a box of its two faces and its two jambs as well, and the wall's faces carry
//! on in line with both of its long sides; a pillar has at most a line laid over one side.
//!
//! And a wall's faces stop where a flue stands in the corner of a room, as they stop at a doorway, and on that same
//! drawing the flue was read as two doors, one in each wall it stands against. What tells the two apart is what the
//! drawing draws at the ends of the gap ([`solid`]): a doorway is closed by its jambs, drawn across the wall from the
//! one face to the other, while the faces round a mass turn away from the wall, out round the mass. Where they turn
//! away and nothing is drawn across, the gap is no opening: the wall runs on through it, and the box the faces turn
//! round, where it is a box no bigger than a pillar, is what stands there.

use super::{Found, How, Squares, on};
use crate::geom::P2;

/// The longest side of a pillar, and of the mass a wall's faces turn round, in millimetres: a pillar or a flue is
/// smaller than this, and a box bigger than it is a room, a shower tray, a stair's landing.
pub const PILLAR: f64 = 600.0;

/// The shortest side of a pillar, in millimetres: nothing that bears a floor is slenderer, and a box slenderer than this
/// is a symbol — on that drawing a box 300 by 72 marks the entrance under the door it points at.
pub const SLENDEREST: f64 = 150.0;

/// Something standing in the plan that is no wall: its middle, its width and depth, and the angle of its width in
/// degrees, counter-clockwise from east.
#[derive(Clone, Debug, PartialEq)]
pub struct Pillar {
    pub at: P2,
    pub width: f64,
    pub depth: f64,
    pub angle: f64,
}

impl Pillar {
    /// The pillar of the box whose corner is `a`, whose width runs to `b` and whose depth runs from `b` to `c`.
    fn of(a: P2, b: P2, c: P2) -> Pillar {
        let along = b - a;
        Pillar { at: (a + c) * 0.5, width: along.length(), depth: b.dist(c), angle: along.1.atan2(along.0).to_degrees() }
    }

    /// Whether this is the same thing as `other`, found again from another side of it.
    fn same(&self, other: &Pillar) -> bool {
        let near = self.width.max(self.depth).max(other.width.max(other.depth)) / 4.0;
        self.at.dist(other.at) <= near
    }
}

/// How far apart two ends of a drawing's lines may be and still be the one corner of a box: a corner is drawn
/// exactly, so this is the drawing's own looseness, a tenth of the thinnest wall looked for.
fn corner(how: &How) -> f64 {
    how.thin / 10.0
}

/// The closed boxes among `marks` that stand free: the pillars, and, for each mark, whether it is a side of one, so
/// that it is no face of a wall.
pub(super) fn pillars(marks: &[(P2, P2)], how: &How) -> (Vec<Pillar>, Vec<bool>) {
    let squares = Squares::of(marks);
    let square = how.skew.to_radians().sin();
    let tol = corner(how);
    let short = |k: usize| marks.get(k).is_some_and(|(p, q)| (SLENDEREST..=PILLAR).contains(&p.dist(*q)));
    // The mark with an end at `at`, other than those in `not`, that `fits`: and its other end.
    let from = |at: P2, not: &[usize], fits: &dyn Fn(P2) -> bool| {
        squares.near(at, at, tol).into_iter().filter(|k| short(*k) && !not.contains(k)).find_map(|k| {
            let (p, q) = marks.get(k)?;
            let other = match (p.dist(at) <= tol, q.dist(at) <= tol) {
                (true, _) => *q,
                (_, true) => *p,
                _ => return None,
            };
            fits(other).then_some((k, other))
        })
    };
    // Every box, as its four sides, each found once.
    let mut boxes: Vec<([usize; 4], [P2; 4])> = Vec::new();
    for (i, &(a, b)) in marks.iter().enumerate() {
        if !short(i) {
            continue;
        }
        let u = (b - a).unit();
        let across = |p: P2, q: P2| (q - p).length() > 0.0 && u.dot((q - p).unit()).abs() <= square;
        let Some((j, c)) = from(b, &[i], &|c| across(b, c)) else { continue };
        let d_at = a + (c - b);
        let Some((k, d)) = from(c, &[i, j], &|d| d.dist(d_at) <= tol) else { continue };
        let Some((l, _)) = from(d, &[i, j, k], &|e| e.dist(a) <= tol) else { continue };
        let mut sides = [i, j, k, l];
        sides.sort_unstable();
        if !boxes.iter().any(|(s, _)| *s == sides) {
            boxes.push((sides, [a, b, c, d]));
        }
    }
    let of_a_box: Vec<bool> = (0..marks.len()).map(|k| boxes.iter().any(|(s, _)| s.contains(&k))).collect();
    let mut found: Vec<Pillar> = Vec::new();
    let mut sides = vec![false; marks.len()];
    for (four, [a, b, c, d]) in &boxes {
        // Carried on in line past the box: a line of the drawing, not a box's own, along the side's line and reaching
        // past its end, no farther off than an opening is wide.
        let carried = |p: P2, q: P2| {
            let len = p.dist(q);
            let (u, n) = ((q - p).unit(), (q - p).unit().perp());
            squares.near(p, q, super::WIDEST).into_iter().any(|k| {
                let Some(&(r, s)) = marks.get(k) else { return false };
                if of_a_box.get(k) == Some(&true) || n.dot(r - p).abs() > tol || n.dot(s - p).abs() > tol {
                    return false;
                }
                let (t0, t1) = (u.dot(r - p).min(u.dot(s - p)), u.dot(r - p).max(u.dot(s - p)));
                (t0 < -tol || t1 > len + tol) && t1 >= -super::WIDEST && t0 <= len + super::WIDEST
            })
        };
        let in_line = (carried(*a, *b) && carried(*c, *d)) || (carried(*b, *c) && carried(*d, *a));
        // Nothing drawn inside it: a basin drawn in a box is a sink, and a pillar is drawn empty, or hatched from side
        // to side.
        let (u, w, v, h) = ((*b - *a).unit(), a.dist(*b), (*d - *a).unit(), a.dist(*d));
        let inside = |p: P2| {
            let (s, t) = (u.dot(p - *a), v.dot(p - *a));
            s > tol && s < w - tol && t > tol && t < h - tol
        };
        let filled = squares
            .near(*a, *c, tol)
            .into_iter()
            .any(|k| !four.contains(&k) && marks.get(k).is_some_and(|(p, q)| inside(*p) || inside(*q)));
        if in_line || filled {
            continue;
        }
        let pillar = Pillar::of(*a, *b, *c);
        if found.iter().any(|f| f.same(&pillar)) {
            continue;
        }
        found.push(pillar);
        for k in four {
            if let Some(side) = sides.get_mut(*k) {
                *side = true;
            }
        }
    }
    (found, sides)
}

/// What the drawing draws across the stretch `gap` of `wall`'s axis, where the wall's faces stop: `None` where it is
/// an opening, closed by a jamb drawn across the wall from the one face to the other, or with nothing drawn at its
/// ends at all; and where a face turns away from the wall at an end that `ends` reaches, with nothing drawn across,
/// the boxes the faces turn round — no bigger than a pillar — for the mass standing there, which may be none.
pub(super) fn solid(wall: &Found, gap: (f64, f64), ends: (f64, f64), marks: &[(P2, P2)], how: &How) -> Option<Vec<Pillar>> {
    let (half, slack) = (wall.thickness / 2.0, how.slack(wall.thickness));
    let (dir, square) = (wall.dir(), how.skew.to_radians().sin());
    let (lo, hi) = (gap.0.min(gap.1) - slack, gap.0.max(gap.1) + slack);
    let (end_lo, end_hi) = (ends.0.min(ends.1) - slack, ends.0.max(ends.1) + slack);
    let at = |s: f64, off: f64| wall.start + dir * s + dir.perp() * off;
    // The lines drawn across the wall in the gap, as where along it they stand and the stretch across it they cover.
    let across: Vec<(f64, f64, f64)> = marks
        .iter()
        .filter(|(p, q)| (*q - *p).length() > 0.0 && dir.dot((*q - *p).unit()).abs() <= square)
        .map(|(p, q)| {
            let ((s0, o0), (s1, o1)) = (on(wall, *p), on(wall, *q));
            ((s0 + s1) / 2.0, o0.min(o1), o0.max(o1))
        })
        .filter(|(s, _, _)| (lo..=hi).contains(s))
        .collect();
    if across.iter().any(|(_, o0, o1)| *o0 <= -half + slack && *o1 >= half - slack) {
        return None;
    }
    // The faces turning away: a line across that starts on a face, at an end, and runs out from the wall.
    let mut legs: Vec<(f64, f64, f64)> = Vec::new();
    for (s, o0, o1) in &across {
        if !(end_lo..=end_hi).contains(s) {
            continue;
        }
        for side in [1.0, -1.0] {
            let (near, far) = ((o0 * side).min(o1 * side), (o0 * side).max(o1 * side));
            if near <= half + slack && near >= -half + slack && far >= half + how.thin {
                legs.push((*s, side, far - half));
            }
        }
    }
    if legs.is_empty() {
        return None;
    }
    // The box each leg is the side of: a line from its outer end back along the wall, within the gap, no longer than
    // a pillar is wide, and the leg no longer than a pillar is deep.
    let mut masses: Vec<Pillar> = Vec::new();
    for (s, side, depth) in legs {
        if depth > PILLAR {
            continue;
        }
        let out = at(s, side * (half + depth));
        let bar = marks.iter().find_map(|(p, q)| {
            let v = *q - *p;
            if v.length() <= 0.0 || dir.cross(v.unit()).abs() > square {
                return None;
            }
            let other = match (p.dist(out) <= slack, q.dist(out) <= slack) {
                (true, _) => *q,
                (_, true) => *p,
                _ => return None,
            };
            let (t, _) = on(wall, other);
            let width = (t - s).abs();
            ((how.thin..=PILLAR).contains(&width) && (lo..=hi).contains(&t)).then_some(t)
        });
        let Some(t) = bar else { continue };
        let pillar = Pillar::of(at(s, side * half), at(t, side * half), at(t, side * (half + depth)));
        if !masses.iter().any(|m| m.same(&pillar)) {
            masses.push(pillar);
        }
    }
    Some(masses)
}

//! Walls found in the drawing under a plan: the first step of making a model out of a survey drawn in two dimensions,
//! which is otherwise traced over by hand.
//!
//! A draughtsman draws a wall as its two faces, so a wall in a drawing is a pair of lines that run alongside one
//! another, a wall's thickness apart. That is the whole of the rule here: [`read`] takes every straight run of the
//! drawing long enough to be a wall's face, pairs each with the nearest face across it, and gives back the line down
//! the middle of each pair with the thickness it measured. What it finds is a reading, as the phases a layer's name
//! carries are: it says what it found, and the user's word is the last one.
//!
//! Three things keep it from finding walls where there are none. A pair closer together than [`How::thin`] is a line's
//! own thickness or a hatch, and one farther apart than [`How::thick`] is a room. The two faces of a wall keep the
//! same distance apart along the stretch they share, so a pair that splays is a kerb or a roof's pitch, not a wall.
//! And a face's wall is the *nearest* face across it: in a corridor 600 wide between two walls 100 thick, every
//! distance from 100 to 800 is there to be mistaken for a wall, and only the two 100s are walls.
//!
//! One thing more is not a wall, and it is the one a real survey is full of: a ladder. A stair's treads, a hatch, a
//! fence and a run of joists are parallel lines evenly spaced, and every neighbouring pair of them measures the same
//! thickness. A wall's face has a room on one side of it, so it is the asymmetry that tells them apart: a face with a
//! face across it on *both* sides, the same distance away and over the same stretch, is a rung, and no pair it is in
//! is a wall. What that costs is a wall with a cavity behind it exactly as wide as the wall itself, which reads as a
//! ladder of four and is left to be drawn by hand.
//!
//! Two tidying steps follow. Pieces of one wall, where the drawing broke a face in two, are joined where they touch —
//! a gap stays a gap, so a doorway is not bricked up. And an axis, which stops half a wall short of a corner because
//! the faces that made it do, is stretched to the axis it was going to meet.

use crate::geom::P2;
use crate::model::{Model, Underlay};

/// The most faces of a drawing weighed. Every pair of them is, so the work is the square of this: a drawing with more
/// lines as long as a wall than this is refused, rather than taking an afternoon over it.
pub const MAX_FACES: usize = 10_000;

/// What a wall looks like in a drawing, in the project's millimetres and in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct How {
    /// The thinnest wall looked for: a pair of lines closer than this is a line's own thickness, or a hatch.
    pub thin: f64,
    /// The thickest: a pair farther apart than this is a room, not a wall.
    pub thick: f64,
    /// The shortest face, and the shortest stretch two faces must run alongside one another, to make a wall.
    pub shortest: f64,
    /// How many degrees the two faces of one wall may differ in direction by.
    pub skew: f64,
}

impl Default for How {
    /// What a building is drawn with: walls from a 60 mm lining to an 800 mm stone wall, nothing shorter than a
    /// doorway's width, drawn square to a couple of degrees.
    fn default() -> How {
        How { thin: 60.0, thick: 800.0, shortest: 400.0, skew: 2.0 }
    }
}

impl How {
    /// Itself, where a drawing could be read with it; else what is wrong with it. Numbers come from the user, so one
    /// that is no measure of a wall is refused here rather than finding everything or nothing further down.
    fn checked(&self) -> Result<How, String> {
        let How { thin, thick, shortest, skew } = *self;
        if ![thin, thick, shortest, skew].iter().all(|v| v.is_finite()) {
            return Err("every measure of a wall must be a number".into());
        }
        if thin <= 0.0 || thick < thin {
            return Err(format!("a wall is between `thin` and `thick` mm thick, and {thin} to {thick} is no thickness"));
        }
        if shortest <= 0.0 {
            return Err(format!("`shortest` is how short a wall may be, in mm, and {shortest} is none"));
        }
        if !(0.0..=45.0).contains(&skew) {
            return Err(format!("`skew` is how many degrees out of parallel a wall's faces may be, 0 to 45, not {skew}"));
        }
        Ok(*self)
    }

    /// How far out of true the same wall may be drawn and still be itself: a tenth of what is measured, and never
    /// less than a tenth of the thinnest wall looked for.
    fn slack(&self, measured: f64) -> f64 {
        (self.thin / 10.0).max(measured.abs() / 10.0)
    }
}

/// A wall the drawing holds: the line down the middle of it, how thick it measured, and the layers its two faces are
/// drawn on — which is what says the moment of the project's history it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub start: P2,
    pub end: P2,
    pub thickness: f64,
    pub layers: [String; 2],
}

impl Found {
    fn length(&self) -> f64 {
        self.start.dist(self.end)
    }

    fn dir(&self) -> P2 {
        (self.end - self.start).unit()
    }
}

/// What a drawing was read for walls: the walls, how many of its lines were long enough to be a wall's face, and the
/// longest line in it. That last one is what tells a drawing that is not in the units it says it is: a survey whose
/// longest line is 13 mm is a survey drawn in metres, and the answer says so rather than guessing at a scale.
#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub walls: Vec<Found>,
    pub faces: usize,
    pub longest: f64,
}

/// One face of a wall: a straight run of the drawing, long enough to be one.
struct Face {
    /// Which of the drawing's layers it is on.
    layer: usize,
    at: P2,
    dir: P2,
    len: f64,
}

impl Face {
    fn end(&self) -> P2 {
        self.at + self.dir * self.len
    }
}

/// The walls the drawing `u` holds, in the project's millimetres, where it stands and turned as it is. A drawing that
/// is not loaded holds none.
pub fn read(m: &Model, u: &Underlay, how: &How) -> Result<Reading, String> {
    let how = how.checked()?;
    let mut names: Vec<String> = Vec::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut longest = 0.0f64;
    for (layer, points, closed) in crate::underlay::lines_by_layer(m, u) {
        let at = match names.iter().position(|n| *n == layer) {
            Some(at) => at,
            None => {
                names.push(layer);
                names.len() - 1
            }
        };
        let close = match (closed, points.len() > 2, points.first(), points.last()) {
            (true, true, Some(a), Some(b)) => Some((*b, *a)),
            _ => None,
        };
        let runs = points
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => Some((*a, *b)),
                _ => None,
            })
            .chain(close);
        for (a, b) in runs {
            let len = a.dist(b);
            if !len.is_finite() {
                continue;
            }
            longest = longest.max(len);
            if len < how.shortest {
                continue;
            }
            if faces.len() >= MAX_FACES {
                return Err(format!(
                    "this drawing has more than {MAX_FACES} lines as long as a wall: hide the layers that are not \
                     the building's, or look for longer walls with `shortest`"
                ));
            }
            faces.push(Face { layer: at, dir: (b - a).unit(), at: a, len });
        }
    }
    let walls = pair_up(&faces, &names, &how);
    Ok(Reading { walls, faces: faces.len(), longest })
}

/// What the two faces of one wall would measure: how far `b` lies to the left of `a`, and the stretch of `a` the two
/// run alongside one another. None where they are no wall: not parallel, too close or too far apart, splaying, or
/// barely passing one another.
fn across(a: &Face, b: &Face, how: &How) -> Option<(f64, f64, f64)> {
    if (a.dir.cross(b.dir)).abs() > how.skew.to_radians().sin() {
        return None;
    }
    let (along, off) = (|p: P2| a.dir.dot(p - a.at), |p: P2| a.dir.perp().dot(p - a.at));
    // The stretch of `a` that `b` runs alongside.
    let (t0, t1) = (along(b.at), along(b.end()));
    let (s0, s1) = (t0.min(t1).max(0.0), t0.max(t1).min(a.len));
    if s1 - s0 < how.shortest {
        return None;
    }
    // How far apart they are at both ends of that stretch: the faces of one wall keep the same distance, so a pair
    // that splays is a kerb or a roof's pitch.
    let (o0, o1) = (off(b.at), off(b.end()));
    let at = |s: f64| match (t1 - t0).abs() < f64::EPSILON {
        true => o0,
        false => o0 + (o1 - o0) * (s - t0) / (t1 - t0),
    };
    let (d0, d1) = (at(s0), at(s1));
    if (d0 - d1).abs() > how.slack(d0.abs().min(d1.abs())) {
        return None;
    }
    let d = (d0 + d1) / 2.0;
    if !(how.thin..=how.thick).contains(&d.abs()) {
        return None;
    }
    Some((d, s0, s1))
}

/// Every face paired with the nearest face across it, as the walls that makes, tidied.
fn pair_up(faces: &[Face], names: &[String], how: &How) -> Vec<Found> {
    let mut pairs: Vec<(usize, usize, f64, f64, f64)> = Vec::new();
    let mut nearest = vec![f64::INFINITY; faces.len()];
    for (i, a) in faces.iter().enumerate() {
        for (j, b) in faces.iter().enumerate().skip(i + 1) {
            let Some((d, s0, s1)) = across(a, b, how) else { continue };
            nearest[i] = nearest[i].min(d.abs());
            nearest[j] = nearest[j].min(d.abs());
            pairs.push((i, j, d, s0, s1));
        }
    }
    // A rung of a ladder: a face with a face across it on both sides, as far away as each other and over the same
    // stretch of it. A stair's treads, a hatch, a fence.
    let rung: Vec<bool> = (0..faces.len())
        .map(|i| {
            let flanks: Vec<(f64, f64)> = pairs
                .iter()
                .filter_map(|(a, b, d, s0, s1)| {
                    let side = match (*a == i, *b == i) {
                        (true, _) => *d,
                        (_, true) => -*d,
                        _ => return None,
                    };
                    Some((side, (s1 - s0) / faces[i].len))
                })
                .filter(|(_, over)| *over >= 0.9)
                .collect();
            flanks.iter().any(|(up, _)| *up > 0.0 && flanks.iter().any(|(down, _)| *down < 0.0 && (up + down).abs() <= how.slack(*up)))
        })
        .collect();
    let mut walls: Vec<Found> = Vec::new();
    for (i, j, d, s0, s1) in pairs {
        if rung[i] || rung[j] {
            continue;
        }
        // A face's wall is the nearest face across it. A farther one is the other side of a room — and the same wall,
        // drawn a little out of true, must not be told from itself, which is what the slack is for.
        let t = d.abs();
        if t > nearest[i] + how.slack(t) || t > nearest[j] + how.slack(t) {
            continue;
        }
        let (a, half) = (&faces[i], d / 2.0);
        let mid = |s: f64| a.at + a.dir * s + a.dir.perp() * half;
        walls.push(Found { start: mid(s0), end: mid(s1), thickness: t, layers: [name_of(names, a.layer), name_of(names, faces[j].layer)] });
    }
    join(&mut walls, how);
    meet(&mut walls, how);
    walls
}

fn name_of(names: &[String], at: usize) -> String {
    names.get(at).cloned().unwrap_or_default()
}

/// Pieces of one wall become one wall: alongside one another, as thick as each other, and touching. A gap stays a
/// gap, so a doorway is not bricked up.
fn join(walls: &mut Vec<Found>, how: &How) {
    let mut again = true;
    while again {
        again = false;
        'pairs: for i in 0..walls.len() {
            for j in (i + 1)..walls.len() {
                let (a, b) = (&walls[i], &walls[j]);
                if (a.thickness - b.thickness).abs() > how.slack(a.thickness) {
                    continue;
                }
                if a.dir().cross(b.dir()).abs() > how.skew.to_radians().sin() {
                    continue;
                }
                let slack = how.slack(a.thickness);
                let off = |p: P2| a.dir().perp().dot(p - a.start);
                if off(b.start).abs() > slack || off(b.end).abs() > slack {
                    continue;
                }
                // Along the one, where the other lies: touching, or overlapping, is one wall.
                let along = |p: P2| a.dir().dot(p - a.start);
                let (mine, theirs) = ([0.0, a.length()], [along(b.start), along(b.end)]);
                let (t0, t1) = (theirs[0].min(theirs[1]), theirs[0].max(theirs[1]));
                if t0 > mine[1] + how.thin || t1 < mine[0] - how.thin {
                    continue;
                }
                let (lo, hi) = (mine[0].min(t0), mine[1].max(t1));
                let (dir, at) = (a.dir(), a.start);
                let (thickness, layers) = ((a.thickness + b.thickness) / 2.0, a.layers.clone());
                walls[i] = Found { start: at + dir * lo, end: at + dir * hi, thickness, layers };
                walls.remove(j);
                again = true;
                break 'pairs;
            }
        }
    }
}

/// An axis stretched to the one it was going to meet. The faces that made a wall stop short of a corner, by half the
/// wall they run into, and so does the axis between them; an end within the thickness of another wall's axis, which
/// would cross it, is taken to the crossing.
fn meet(walls: &mut [Found], how: &How) {
    let was: Vec<Found> = walls.to_vec();
    for (i, wall) in walls.iter_mut().enumerate() {
        for end in [false, true] {
            let mine = &was[i];
            let p = if end { mine.end } else { mine.start };
            let mut best: Option<(f64, P2)> = None;
            for (j, other) in was.iter().enumerate() {
                if j == i || mine.dir().cross(other.dir()).abs() <= how.skew.to_radians().sin() {
                    continue;
                }
                let Some(at) = crate::geom::line_intersection(mine.start, mine.end, other.start, other.end) else {
                    continue;
                };
                // Near this end of mine, and within the other's own stretch, grown by half of each wall.
                let reach = (mine.thickness + other.thickness) / 2.0;
                if p.dist(at) > other.thickness / 2.0 + how.slack(other.thickness) {
                    continue;
                }
                let s = other.dir().dot(at - other.start);
                if s < -reach || s > other.length() + reach {
                    continue;
                }
                if best.is_none_or(|(near, _)| p.dist(at) < near) {
                    best = Some((p.dist(at), at));
                }
            }
            // Stretched, never shortened to nothing: an end taken to a crossing that leaves less than half the
            // wall, or turns it round, is left where it was.
            if let Some((_, at)) = best {
                let (start, end_at) = match end {
                    true => (wall.start, at),
                    false => (at, wall.end),
                };
                if (end_at - start).dot(mine.dir()) >= mine.length() / 2.0 {
                    (wall.start, wall.end) = (start, end_at);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::{Drawing, Layer, Shape};
    use std::sync::Arc;

    /// A model with one underlay of the lines given, each on the layer it names.
    fn with(lines: &[(&str, [f64; 4])]) -> (Model, Underlay) {
        let mut m = Model::new("Test");
        let mut layers: Vec<Layer> = Vec::new();
        for (name, [x0, y0, x1, y1]) in lines {
            let shape = Shape::Line([P2(*x0, *y0), P2(*x1, *y1)]);
            match layers.iter_mut().find(|l| l.name == *name) {
                Some(l) => l.shapes.push(shape),
                None => layers.push(Layer { name: (*name).into(), shapes: vec![shape] }),
            }
        }
        m.drawings.insert("flat.dxf".into(), Arc::new(Drawing { layers, scale: 1.0, ..Drawing::default() }));
        let u = Underlay { name: "Survey".into(), path: "flat.dxf".into(), scale: 1.0, ..Underlay::default() };
        (m, u)
    }

    /// The walls found in those lines, by the default rule.
    fn found(lines: &[(&str, [f64; 4])]) -> Vec<Found> {
        let (m, u) = with(lines);
        read(&m, &u, &How::default()).expect("it reads").walls
    }

    #[test]
    fn a_pair_of_lines_a_walls_thickness_apart_is_a_wall_down_the_middle_of_them() {
        let walls = found(&[("MURI", [0.0, 0.0, 5000.0, 0.0]), ("MURI", [0.0, 300.0, 5000.0, 300.0])]);
        let [wall] = &walls[..] else { panic!("one wall, not {}", walls.len()) };
        assert!((wall.thickness - 300.0).abs() < 1e-9, "{}", wall.thickness);
        assert_eq!((wall.start.1, wall.end.1), (150.0, 150.0), "down the middle");
        assert_eq!((wall.start.0.min(wall.end.0), wall.start.0.max(wall.end.0)), (0.0, 5000.0));
    }

    #[test]
    fn the_faces_of_a_wall_keep_the_same_distance_apart() {
        // 300 at one end, 900 at the other: a kerb, a ditch, a roof's pitch — no wall.
        assert!(found(&[("A", [0.0, 0.0, 5000.0, 0.0]), ("A", [0.0, 300.0, 5000.0, 900.0])]).is_empty());
        // Out of true by a tenth of itself is the same wall still.
        assert_eq!(found(&[("A", [0.0, 0.0, 5000.0, 0.0]), ("A", [0.0, 300.0, 5000.0, 320.0])]).len(), 1);
    }

    #[test]
    fn a_wall_the_drawing_broke_in_two_is_one_wall_and_a_doorway_is_two() {
        let touching = found(&[("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, 300.0, 2500.0, 300.0]), ("A", [2500.0, 300.0, 6000.0, 300.0])]);
        let [one] = &touching[..] else { panic!("one wall, not {}", touching.len()) };
        assert!((one.length() - 6000.0).abs() < 1e-6, "{}", one.length());
        let doorway = found(&[
            ("A", [0.0, 0.0, 2500.0, 0.0]),
            ("A", [3400.0, 0.0, 6000.0, 0.0]),
            ("A", [0.0, 300.0, 2500.0, 300.0]),
            ("A", [3400.0, 300.0, 6000.0, 300.0]),
        ]);
        assert_eq!(doorway.len(), 2, "the doorway is not bricked up");
    }

    #[test]
    fn an_axis_is_stretched_to_the_one_it_was_going_to_meet() {
        // Two walls 300 thick in an L: the faces of each stop 300 short of the other, so the axes would miss by 150.
        let walls = found(&[
            ("A", [0.0, 0.0, 6000.0, 0.0]),
            ("A", [300.0, 300.0, 6000.0, 300.0]),
            ("A", [0.0, 0.0, 0.0, 5000.0]),
            ("A", [300.0, 300.0, 300.0, 5000.0]),
        ]);
        assert_eq!(walls.len(), 2, "{walls:?}");
        let corner = P2(150.0, 150.0);
        for wall in &walls {
            assert!(wall.start.dist(corner).min(wall.end.dist(corner)) < 1e-6, "{wall:?} reaches the corner");
        }
    }

    #[test]
    fn a_ladder_is_no_wall_however_much_each_pair_of_its_rungs_looks_like_one() {
        // A flight of five treads 280 apart, 1200 wide: four pairs, every one of them a wall's thickness apart.
        let mut lines: Vec<(&str, [f64; 4])> = Vec::new();
        for n in 0..5 {
            let y = 1000.0 + 280.0 * n as f64;
            lines.push(("SCALA", [0.0, y, 1200.0, y]));
        }
        assert!(found(&lines).is_empty(), "{:?}", found(&lines));
        // And the wall beside it is a wall still.
        lines.push(("MURI", [3000.0, 0.0, 3000.0, 5000.0]));
        lines.push(("MURI", [3300.0, 0.0, 3300.0, 5000.0]));
        let walls = found(&lines);
        let [wall] = &walls[..] else { panic!("one wall, not {}", walls.len()) };
        assert!((wall.thickness - 300.0).abs() < 1e-9);
    }

    #[test]
    fn an_axis_is_never_stretched_away_to_nothing() {
        // A stub of wall in a corner of three: every end of it has a crossing to be taken to, and it must still be a
        // wall afterwards.
        let walls = found(&[
            ("A", [0.0, 0.0, 6000.0, 0.0]),
            ("A", [300.0, 300.0, 6000.0, 300.0]),
            ("A", [0.0, 0.0, 0.0, 5000.0]),
            ("A", [300.0, 300.0, 300.0, 5000.0]),
            ("A", [0.0, 5000.0, 6000.0, 5000.0]),
            ("A", [300.0, 4700.0, 6000.0, 4700.0]),
        ]);
        for wall in &walls {
            assert!(wall.length() > 1.0, "{wall:?} is no wall at all");
        }
    }

    #[test]
    fn no_measure_of_a_wall_that_could_find_nothing_or_everything_is_taken() {
        let (m, u) = with(&[("A", [0.0, 0.0, 5000.0, 0.0])]);
        for how in [
            How { thin: 0.0, ..How::default() },
            How { thin: 800.0, thick: 60.0, ..How::default() },
            How { shortest: -1.0, ..How::default() },
            How { skew: 90.0, ..How::default() },
            How { thick: f64::NAN, ..How::default() },
        ] {
            assert!(read(&m, &u, &how).is_err(), "{how:?}");
        }
    }
}

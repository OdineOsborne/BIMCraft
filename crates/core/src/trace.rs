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
//! Two things more are not walls, and a real survey is full of both. One is a ladder: a stair's treads, a hatch, a
//! fence and a run of joists are parallel lines evenly spaced, and every neighbouring pair of them measures the same
//! thickness. A wall's face has a room on one side of it, so it is the asymmetry that tells them apart: a face with a
//! face across it on *both* sides, the same distance away and over the same stretch, is a rung, and no pair it is in
//! is a wall.
//!
//! The other is a mark drawn *inside* a wall. A survey draws a window by carrying the wall's two faces across the
//! opening and drawing the glass down the middle of it, and the glass is nearer to a face than the far face is, so
//! the nearest-face rule takes glass and face for a wall 130 thick and loses the 300 wall it is drawn in. A wall's
//! face has a room on one side of it, so again it is the asymmetry that tells them apart: a face with a face close up
//! on *both* sides, over the whole of it, is inside something ([`INSIDE`]), and it is the pair that has it between
//! them that is the wall. Hatching, a cavity's lining and a duct read the same way, and a wall with a cavity behind
//! it now comes in as one wall as thick as both.
//!
//! Third, and the same thing a step further out, is the line between a wall's two leaves. A survey draws a wall of a
//! hundred of facing on three hundred of wall as three lines, and the middle one has a face 100 away on the one side
//! and 300 on the other, which is too far for [`INSIDE`] and too near for the pair to be a room. The nearest-face
//! rule then takes the facing for the wall and leaves the inside of the wall over, and a line left over is the sign
//! of a wrong reading: the wall is the two outer lines and the one between them is the leaves' own. It is taken as
//! that only where neither of the two has anything beyond it that would make a thinner wall with it — otherwise that
//! is the wall and this line is a face like any other — and where it does not lie halfway between them, which is a
//! ladder's rung. A lining and a cavity's inner leaf read the same way.
//!
//! Pieces of one wall, where the drawing broke a face in two, are joined where they touch, or where what is left
//! between them is too narrow to walk through: that is where the drawing broke the line and not where it left a
//! doorway, and the two halves of one wall drawn on two layers break at different places. A gap that is left is a
//! doorway, and [`Hole`] is what becomes of it: the two pieces are made one wall with an opening in it, which is what
//! a building is — a wall with a door in it, not two walls with a space between them. A survey marks an opening the
//! other way too, carrying the wall's faces across it and drawing the glass down the middle between two lines at the
//! jambs, and that is read as well: a jamb is drawn from the one face of the wall to the other, so it is as long as
//! the wall is thick, and a mark half as long again is the survey's own — which way the door opens, where the sill
//! is. Glass drawn in an opening makes it a window; an opening with nothing in it is a door, because a hole you can
//! walk through is a door. What runs the length of the wall is the wall's own line and no glass, or the pier between
//! two windows would be a window itself. A plan says nothing of heights, so what is built takes the project's own
//! door and window sizes.
//!
//! A wall that stops short of the wall it runs into has a doorway at its end ([`open_ends`]). The piece of wall
//! between a door and the corner is a few centimetres long, which is shorter than anything drawn as a wall's face,
//! so the drawing shows the wall ending at the door — and a building is a wall with a door in it, not a wall that
//! stops in mid-air. An end that holds on to nothing and stands within a doorway's width of the wall it was going
//! to meet is taken to it, and what is left between where it stopped and the face of that wall is the opening.
//! What says the wall carries on is the drawing itself: one of its own two faces drawn across the gap, where the
//! wall between the two faces was too short to be read. Without that nothing is built across a room that nothing
//! was drawn across.
//!
//! Last, an axis, which stops half a wall short of a corner because the faces that made it do, is stretched to the
//! axis it was going to meet — the one the two of them have least to go to meet, and not simply the crossing
//! nearest the end, which a third wall whose axis runs through that end may hold it at while the corner stays
//! open. How far short of a corner an axis may stop and still be taken to it is a gap nobody could walk through
//! ([`NARROWEST`]), because a gap at a corner that no one can pass is no gap but a corner drawn carelessly. And
//! because a corner is two axes both stopping short of it, taking the one to the other moves the mark the next is
//! measured from, so it is done again until nothing moves.
//!
//! Then what is left is tidied of three things that pair as a wall does and are none ([`tidy`]). A block shorter than
//! it is thick is no wall, because nothing is a wall across itself: a pier drawn as a box, a plinth, the two jambs of
//! an opening taken for a pair of faces. Two walls cannot be in the same place, so where one axis runs inside
//! another the longer of the two is the wall and the shorter is what the drawing draws in it — the frame of a window,
//! drawn across the opening it fills from the outside of the wall to the inside. And a survey draws the building and,
//! around it, the land: the boundary of the plot, the kerb of the road, the lines the surveyor took his levels along.
//! A pair of those measures a wall's thickness like any other pair, and what tells them apart is that the building
//! holds together: a wall meets another wall at a corner or runs into one, and the land's lines hold on to nothing.
//! So a wall that meets none of the others is left out and counted ([`Reading::alone`]), which [`How::alone`] asks
//! for back — unless nothing in the drawing meets anything, when the drawing is of a wall or two rather than of a
//! building and every one of them is taken.

use crate::geom::P2;
use crate::model::{Model, Underlay};

/// The widest space between two lines that is inside a wall rather than a room between two walls: a cavity, the
/// glass a window is drawn with, a duct, the gap in hatching. Anything wider is somewhere a person can be, and the
/// two lines are two walls' faces. It is a measure of buildings and not of drawings, so it is not the user's to set.
const INSIDE: f64 = 200.0;

/// The narrowest and widest opening looked for in a wall: a gap narrower than this is where the drawing broke a
/// line, and one wider is where a wall stops. Measures of buildings, like [`INSIDE`], not the user's to set.
pub const NARROWEST: f64 = 500.0;
pub const WIDEST: f64 = 3000.0;

/// The widest gap a wall may stop short of the wall it runs into and still be taken to it: a double door, which is
/// the widest thing a building calls a doorway. Where a wall stops wider of another than that, the one room is open
/// to the next, and an opening is not what that is.
const DOORWAY: f64 = 1200.0;

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
    /// Whether to look for the openings in the walls as well. Without it a doorway leaves two walls with a gap
    /// between them, which is what the drawing shows and not what the building is.
    pub openings: bool,
    /// Whether to take the walls that stand alone too: a line that meets no wall of the building is the land around
    /// it, so they are left out, and a survey of a garden wall or a boundary to be built asks for them back.
    pub alone: bool,
}

impl Default for How {
    /// What a building is drawn with: walls from a 60 mm lining to an 800 mm stone wall, nothing shorter than a
    /// doorway's width, drawn square to a couple of degrees.
    fn default() -> How {
        How { thin: 60.0, thick: 800.0, shortest: 400.0, skew: 2.0, openings: true, alone: false }
    }
}

impl How {
    /// Itself, where a drawing could be read with it; else what is wrong with it. Numbers come from the user, so one
    /// that is no measure of a wall is refused here rather than finding everything or nothing further down.
    fn checked(&self) -> Result<How, String> {
        let How { thin, thick, shortest, skew, openings: _, alone: _ } = *self;
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

/// An opening in a wall the drawing holds: which of the walls found it is in, how far along that wall's axis its
/// middle is, how wide it measured, and whether it is a window — glass drawn in an opening makes it one.
#[derive(Clone, Debug, PartialEq)]
pub struct Hole {
    pub wall: usize,
    pub at: f64,
    pub width: f64,
    pub window: bool,
}

/// What a drawing was read for walls: the walls, the openings in them, how many of its lines were long enough to be a
/// wall's face, and the longest line in it. That last one is what tells a drawing that is not in the units it says it is: a survey whose
/// longest line is 13 mm is a survey drawn in metres, and the answer says so rather than guessing at a scale.
#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub walls: Vec<Found>,
    pub holes: Vec<Hole>,
    pub faces: usize,
    pub longest: f64,
    /// How many of the walls it found met no other wall: the land around the building rather than the building. They
    /// are left out of `walls` unless [`How::alone`] asked for them, and counted here either way.
    pub alone: usize,
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
    // Every run of the drawing, however short: a wall's faces are the long ones, and what marks an opening in a wall
    // is drawn with the short ones.
    let mut marks: Vec<(P2, P2)> = Vec::new();
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
            marks.push((a, b));
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
    let (walls, holes, alone) = pair_up(&faces, &names, &how, &marks);
    Ok(Reading { walls, holes, faces: faces.len(), longest, alone })
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

/// How far `b` lies to the left of `a`, and how far along `a` the two run alongside one another, whatever the
/// distance between them: what [`inside`] needs, where [`across`] would have thrown the pair away for being too
/// close or too far.
fn alongside(a: &Face, b: &Face, how: &How) -> Option<(f64, f64)> {
    if (a.dir.cross(b.dir)).abs() > how.skew.to_radians().sin() {
        return None;
    }
    let (along, off) = (|p: P2| a.dir.dot(p - a.at), |p: P2| a.dir.perp().dot(p - a.at));
    let (t0, t1) = (along(b.at), along(b.end()));
    let (s0, s1) = (t0.min(t1).max(0.0), t0.max(t1).min(a.len));
    if s1 - s0 <= 0.0 || a.len <= 0.0 {
        return None;
    }
    Some(((off(b.at) + off(b.end())) / 2.0, s1 - s0))
}

/// The faces within `reach` across `a`, leaving out those `skip` says to: how far each lies to the left of it, and
/// how far along `a` they run alongside it. Faces at the same distance are one face the drawing broke in two — at an
/// opening, at a corner, where another wall runs into it — so what they run is counted together. A line drawn on top
/// of another is to neither side of it and says nothing. Only what is near enough to be looked at is weighed, or a
/// plan of a thousand rooms would weigh every face against the whole row it stands in.
fn across_from(faces: &[Face], i: usize, how: &How, skip: &[bool], reach: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (j, b) in faces.iter().enumerate() {
        if j == i || skip.get(j) == Some(&true) {
            continue;
        }
        let Some((d, along)) = alongside(&faces[i], b, how) else { continue };
        if d == 0.0 || d.abs() > reach {
            continue;
        }
        match out.iter_mut().find(|(at, _)| (*at - d).abs() <= how.slack(d)) {
            Some((_, run)) => *run += along,
            None => out.push((d, along)),
        }
    }
    out
}

/// Which faces are no faces of a wall but lines drawn inside one, in two passes.
///
/// First the marks: a face with a face closer than [`INSIDE`] on both sides of it, over the whole of it — a window's
/// glass, hatching, a duct's lining. Then, with the marks out of the way, the line between a wall's two leaves. It
/// lies between two faces no farther apart than a wall is thick, neither of which has anything beyond it to be a
/// wall with, so that taking the near pair for the wall would leave the far face over — and a line over is the sign
/// of a wrong reading. It must not lie halfway between the two, because that is a ladder's rung and not a leaf: the
/// asymmetry again. What counts as being alongside here is a stretch as long as a wall ([`How::shortest`]) and not a
/// share of the face's length, because the inside of a wall stops short of the corners it turns and so is shorter
/// than the line between the leaves, which runs the length of the wall.
fn inside(faces: &[Face], how: &How) -> Vec<bool> {
    let nothing = Vec::new();
    let marks: Vec<bool> = (0..faces.len())
        .map(|i| {
            let across = across_from(faces, i, how, &nothing, INSIDE);
            let near = |up: bool| {
                across.iter().filter(|(d, run)| (*d > 0.0) == up && *run >= faces[i].len * 0.9).map(|(d, _)| d.abs()).min_by(f64::total_cmp)
            };
            matches!((near(true), near(false)), (Some(up), Some(down)) if up <= INSIDE && down <= INSIDE)
        })
        .collect();
    (0..faces.len())
        .map(|i| {
            if marks[i] {
                return true;
            }
            let across = across_from(faces, i, how, &marks, how.thick * 2.0);
            let along = |d: &f64, run: &f64, up: bool| (*d > 0.0) == up && *run >= how.shortest;
            let near = |up: bool| across.iter().filter(|(d, run)| along(d, run, up)).map(|(d, _)| d.abs()).min_by(f64::total_cmp);
            let (Some(up), Some(down)) = (near(true), near(false)) else { return false };
            if up + down > how.thick || (up - down).abs() <= how.slack(up.min(down)) {
                return false;
            }
            // Beyond either of the two, a face that would make a thinner wall with it than this reading does: then
            // that is its wall, by the same rule that takes the nearest face across, nothing is left over, and this
            // line is a face of a wall itself.
            let wall = up + down;
            let beyond = |at: f64, side: f64| {
                across.iter().any(|(d, run)| {
                    let pair = d.abs() - at;
                    *run >= how.shortest && d * side > at && pair <= wall && (how.thin..=how.thick).contains(&pair)
                })
            };
            !beyond(up, 1.0) && !beyond(down, -1.0)
        })
        .collect()
}

/// Every face paired with the nearest face across it, as the walls that makes, tidied, with the openings in them and
/// how many of them stood alone.
fn pair_up(faces: &[Face], names: &[String], how: &How, marks: &[(P2, P2)]) -> (Vec<Found>, Vec<Hole>, usize) {
    let inside = inside(faces, how);
    let mut pairs: Vec<(usize, usize, f64, f64, f64)> = Vec::new();
    let mut nearest = vec![f64::INFINITY; faces.len()];
    for (i, a) in faces.iter().enumerate() {
        for (j, b) in faces.iter().enumerate().skip(i + 1) {
            if inside[i] || inside[j] {
                continue;
            }
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
    let mut holes = match how.openings {
        true => open_up(&mut walls, marks, how),
        false => Vec::new(),
    };
    if how.openings {
        holes.extend(open_ends(&mut walls, marks, how));
        holes.extend(glazed_openings(&walls, marks, how));
    }
    meet(&mut walls, how);
    let alone = tidy(&mut walls, how);
    let holes = placed(&walls, &holes, how);
    (walls, holes, alone)
}

/// What a survey draws that pairs as a wall does and is no wall of the building, taken out of the walls found: a
/// block shorter than it is thick, a wall drawn over a longer one, and a line that meets no wall at all. Gives back
/// how many stood alone, which is what the user is told, because that one is theirs to ask back for.
fn tidy(walls: &mut Vec<Found>, how: &How) -> usize {
    // Nothing is a wall across itself: a pier drawn as a box, a plinth, the two jambs of an opening read as a pair
    // of faces. The thickness is what the pair measured, so it is the length that has to answer for itself.
    walls.retain(|w| w.length() > w.thickness);
    // Two walls cannot be in the same place. The longer is the wall; the shorter is what the drawing draws in it.
    let mut over = vec![false; walls.len()];
    for i in 0..walls.len() {
        for j in (i + 1)..walls.len() {
            if over[i] || over[j] || !same_place(&walls[i], &walls[j], how) {
                continue;
            }
            over[if walls[i].length() <= walls[j].length() { i } else { j }] = true;
        }
    }
    *walls = kept(walls, &over.iter().map(|over| !over).collect::<Vec<bool>>());
    // A wall of a building meets another wall. A line that meets none of them is the land around the building —
    // unless nothing in the drawing meets anything, when the drawing is of a wall or two and not of a building.
    let met: Vec<bool> = (0..walls.len()).map(|i| walls.iter().enumerate().any(|(j, w)| j != i && joined(&walls[i], w))).collect();
    let alone = met.iter().filter(|met| !**met).count();
    if met.iter().any(|met| *met) && !how.alone {
        *walls = kept(walls, &met);
    }
    alone
}

/// The walls their place in `keep` says to keep.
fn kept(walls: &[Found], keep: &[bool]) -> Vec<Found> {
    walls.iter().zip(keep).filter(|(_, keep)| **keep).map(|(wall, _)| wall.clone()).collect()
}

/// Whether these two walls meet: an end of either within half of the two thicknesses of the other's own stretch,
/// which is where [`meet`] has already taken the axes that cross. A corner, a T, or one running into another.
fn joined(a: &Found, b: &Found) -> bool {
    // Within half of each of their thicknesses of one another, as the true distance and not a box about the wall,
    // or two lines that pass a corner wide of each other hold one another up.
    let reach = (a.thickness + b.thickness) / 2.0;
    [a.start, a.end].iter().any(|p| away(b, *p) <= reach) || [b.start, b.end].iter().any(|p| away(a, *p) <= reach)
}

/// How far `p` lies from a wall: its distance to the stretch of the wall's axis, not to the line through it.
fn away(w: &Found, p: P2) -> f64 {
    let (along, off) = on(w, p);
    off.hypot((-along).max(along - w.length()).max(0.0))
}

/// Whether one of these walls is drawn in the place of the other: alongside it over most of the shorter of them, with
/// the one axis inside the other's thickness, so that the two would stand in one another.
fn same_place(a: &Found, b: &Found, how: &How) -> bool {
    if a.dir().cross(b.dir()).abs() > how.skew.to_radians().sin() {
        return false;
    }
    let ((s0, o0), (s1, o1)) = (on(a, b.start), on(a, b.end));
    let (lo, hi) = (s0.min(s1).max(0.0), s0.max(s1).min(a.length()));
    if hi - lo < a.length().min(b.length()) * 0.8 {
        return false;
    }
    let apart = ((o0 + o1) / 2.0).abs();
    [a, b].iter().any(|w| apart <= w.thickness / 2.0 + how.slack(w.thickness))
}

/// How far along a wall's axis a point lies, and how far off it.
fn on(wall: &Found, p: P2) -> (f64, f64) {
    let d = wall.dir();
    (d.dot(p - wall.start), d.perp().dot(p - wall.start))
}

/// A line of the drawing as the wall sees it: how far it lies off the axis at each end, and the stretch of the axis
/// it runs along. None where it is not drawn alongside the wall at all.
fn axiswise(wall: &Found, p: P2, q: P2, how: &How) -> Option<(f64, f64, f64, f64)> {
    if wall.dir().cross((q - p).unit()).abs() > how.skew.to_radians().sin() {
        return None;
    }
    let ((t0, o0), (t1, o1)) = (on(wall, p), on(wall, q));
    Some((o0, o1, t0.min(t1), t0.max(t1)))
}

/// Whether the drawing draws glass in the stretch `a..b` of `wall`: a line alongside the wall, inside its thickness
/// and not on a face of it, over most of that stretch. A window is drawn with its glass and a doorway is drawn empty,
/// which is the whole of what tells the two apart in a plan.
///
/// A line drawn the length of the wall at that distance off its axis is the wall's own and not glass: the line
/// between a wall's two leaves lies inside its thickness from end to end, broken only at the openings, so without
/// this every pier between two windows is read as a window itself.
fn glazed(wall: &Found, marks: &[(P2, P2)], a: f64, b: f64, how: &How) -> bool {
    let room = wall.thickness / 2.0 - how.slack(wall.thickness);
    marks.iter().any(|(p, q)| {
        let Some((o0, o1, lo, hi)) = axiswise(wall, *p, *q, how) else { return false };
        if o0.abs() > room || o1.abs() > room || hi.min(b) - lo.max(a) < (b - a) * 0.8 {
            return false;
        }
        runs_along(wall, marks, (o0 + o1) / 2.0, how) <= wall.length() / 2.0
    })
}

/// How much of the wall the drawing draws a line along, `off` from its axis: every stretch at that distance, counted
/// once however many times it is drawn.
fn runs_along(wall: &Found, marks: &[(P2, P2)], off: f64, how: &How) -> f64 {
    let mut spans: Vec<(f64, f64)> = marks
        .iter()
        .filter_map(|(r, s)| {
            let (f0, f1, from, to) = axiswise(wall, *r, *s, how)?;
            let at = (f0 + f1) / 2.0;
            ((at - off).abs() <= how.slack(wall.thickness)).then(|| (from.max(0.0), to.min(wall.length())))
        })
        .filter(|(from, to)| to > from)
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut run, mut far) = (0.0, f64::NEG_INFINITY);
    for (from, to) in spans {
        run += (to - from.max(far)).max(0.0);
        far = far.max(to);
    }
    run
}

/// Whether the drawing carries one of the wall's faces across the stretch `from..to` of its axis, which may lie
/// beyond the wall's own ends: the face of a wall is drawn past the door at the end of it, though the piece of wall
/// left there is too short to be read as a face of its own.
fn face_across(wall: &Found, marks: &[(P2, P2)], from: f64, to: f64, how: &How) -> bool {
    let half = wall.thickness / 2.0;
    let slack = how.slack(wall.thickness);
    [half, -half].iter().any(|face| {
        let mut spans: Vec<(f64, f64)> = marks
            .iter()
            .filter_map(|(p, q)| {
                let (o0, o1, lo, hi) = axiswise(wall, *p, *q, how)?;
                (((o0 + o1) / 2.0 - face).abs() <= slack).then(|| (lo.max(from), hi.min(to)))
            })
            .filter(|(lo, hi)| hi > lo)
            .collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut drawn, mut far) = (0.0, f64::NEG_INFINITY);
        for (lo, hi) in spans {
            drawn += (hi - lo.max(far)).max(0.0);
            far = far.max(hi);
        }
        drawn >= (to - from) * 0.8
    })
}

/// Pieces of one wall with a doorway between them, made one wall with an opening in it. What it found: the middle of
/// each opening, how wide, and whether it is a window.
fn open_up(walls: &mut Vec<Found>, marks: &[(P2, P2)], how: &How) -> Vec<(P2, f64, bool)> {
    let mut holes: Vec<(P2, f64, bool)> = Vec::new();
    let square = how.skew.to_radians().sin();
    let mut again = true;
    while again {
        again = false;
        'pairs: for i in 0..walls.len() {
            for j in 0..walls.len() {
                if i == j {
                    continue;
                }
                let (a, b) = (&walls[i], &walls[j]);
                let slack = how.slack(a.thickness);
                if (a.thickness - b.thickness).abs() > slack || a.dir().cross(b.dir()).abs() > square {
                    continue;
                }
                let ((t0, o0), (t1, o1)) = (on(a, b.start), on(a, b.end));
                if o0.abs() > slack || o1.abs() > slack {
                    continue;
                }
                let (lo, hi) = (t0.min(t1), t0.max(t1));
                let gap = lo - a.length();
                if !(NARROWEST..=WIDEST).contains(&gap) {
                    continue;
                }
                // Nothing else of the same wall stands in the gap.
                let between = walls.iter().enumerate().any(|(k, w)| {
                    if k == i || k == j {
                        return false;
                    }
                    let ((u0, f0), (u1, f1)) = (on(a, w.start), on(a, w.end));
                    f0.abs() <= slack && f1.abs() <= slack && u0.max(u1) > a.length() && u0.min(u1) < lo
                });
                if between {
                    continue;
                }
                let (mid, end) = (a.start + a.dir() * (a.length() + gap / 2.0), a.start + a.dir() * hi);
                holes.push((mid, gap, glazed(a, marks, a.length(), lo, how)));
                walls[i].end = end;
                walls.remove(j);
                again = true;
                break 'pairs;
            }
        }
    }
    holes
}

/// A doorway at the end of a wall, where the drawing leaves the wall stopping short of the wall it runs into. The
/// piece of wall between a door and the corner is a few centimetres long, which is shorter than anything drawn as a
/// wall's face, so the drawing shows the wall ending at the door; and a building is a wall with a door in it, not a
/// wall that stops in mid-air. So an end that holds on to nothing and stands within an opening's width of the wall
/// it was going to meet is taken to it, and what is left between where it stopped and the face of that wall is the
/// opening. The gap between two pieces of one wall is [`open_up`]'s; this is the gap at a corner. What it found: the
/// middle of each opening, how wide, and whether it is a window.
fn open_ends(walls: &mut [Found], marks: &[(P2, P2)], how: &How) -> Vec<(P2, f64, bool)> {
    let was: Vec<Found> = walls.to_vec();
    let square = how.skew.to_radians().sin();
    let mut holes = Vec::new();
    for (i, wall) in walls.iter_mut().enumerate() {
        for far in [false, true] {
            let mine = &was[i];
            let (p, out) = match far {
                true => (mine.end, mine.dir()),
                false => (mine.start, mine.dir() * -1.0),
            };
            // An end that holds on to a wall is a corner already, and no doorway.
            if was.iter().enumerate().any(|(j, other)| j != i && away(other, p) <= (mine.thickness + other.thickness) / 2.0) {
                continue;
            }
            // The nearest wall it was going to meet, a doorway's width away and within that wall's own stretch.
            let mut best: Option<(f64, P2, f64)> = None;
            for (j, other) in was.iter().enumerate() {
                if j == i || mine.dir().cross(other.dir()).abs() <= square {
                    continue;
                }
                let Some(at) = crate::geom::line_intersection(mine.start, mine.end, other.start, other.end) else {
                    continue;
                };
                let gap = out.dot(at - p);
                // Within the other's own stretch, grown by half of each wall, because its axis stops short of its
                // own corners exactly as this one does.
                let reach = (mine.thickness + other.thickness) / 2.0;
                let s = other.dir().dot(at - other.start);
                if !(NARROWEST..=DOORWAY).contains(&gap) || s < -reach || s > other.length() + reach {
                    continue;
                }
                if best.is_none_or(|(near, _, _)| gap < near) {
                    best = Some((gap, at, other.thickness));
                }
            }
            let Some((gap, at, thick)) = best else { continue };
            let width = gap - thick / 2.0;
            if width < how.thin {
                continue;
            }
            let t = on(mine, p).0;
            let (from, to) = match far {
                true => (t, t + width),
                false => (t - width, t),
            };
            // What says the wall carries on is the drawing: a face of it drawn across the gap, where the wall
            // between the two faces was too short to be read. Without that the wall stops where it was drawn, and
            // nothing is built across a room that nothing was drawn across.
            if !face_across(mine, marks, from, to, how) {
                continue;
            }
            holes.push((p + out * (width / 2.0), width, glazed(mine, marks, from, to, how)));
            match far {
                true => wall.end = at,
                false => wall.start = at,
            }
        }
    }
    holes
}

/// The windows a drawing marks without breaking the wall: two lines drawn across it at the jambs, with the glass
/// between them. A pair with no glass between is not taken, because the space between two windows is a pair too.
fn glazed_openings(walls: &[Found], marks: &[(P2, P2)], how: &How) -> Vec<(P2, f64, bool)> {
    let mut holes = Vec::new();
    let square = how.skew.to_radians().sin();
    for wall in walls {
        let (t, len) = (wall.thickness, wall.length());
        let mut cuts: Vec<f64> = marks
            .iter()
            .filter_map(|(p, q)| {
                let v = *q - *p;
                // A jamb is drawn across the wall, from the one face to the other, so it is as long as the wall is
                // thick. A mark half as long again is the survey's own: which way the door opens, where the sill is.
                if !(t * 0.8..=t * 1.3).contains(&v.length()) || v.unit().cross(wall.dir().perp()).abs() > square {
                    return None;
                }
                let (along, off) = on(wall, *p + v * 0.5);
                (off.abs() <= t / 2.0 && (0.0..=len).contains(&along)).then_some(along)
            })
            .collect();
        cuts.sort_by(f64::total_cmp);
        for (a, b) in cuts.iter().zip(cuts.iter().skip(1)) {
            let width = b - a;
            if (NARROWEST..=WIDEST).contains(&width) && glazed(wall, marks, *a, *b, how) {
                holes.push((wall.start + wall.dir() * ((a + b) / 2.0), width, true));
            }
        }
    }
    holes
}

/// Each opening in the wall it is in, once the walls are tidied: where it falls along that wall's axis. One that no
/// longer fits in a wall is dropped, and one already found another way is not found twice.
fn placed(walls: &[Found], found: &[(P2, f64, bool)], how: &How) -> Vec<Hole> {
    let mut holes: Vec<Hole> = Vec::new();
    for (mid, width, window) in found {
        let mut best: Option<(usize, f64, f64)> = None;
        for (i, w) in walls.iter().enumerate() {
            let (along, off) = on(w, *mid);
            let slack = how.slack(w.thickness);
            if off.abs() > w.thickness / 2.0 + slack {
                continue;
            }
            if along - width / 2.0 < -slack || along + width / 2.0 > w.length() + slack {
                continue;
            }
            if best.is_none_or(|(_, _, d)| off.abs() < d) {
                best = Some((i, along, off.abs()));
            }
        }
        let Some((wall, at, _)) = best else { continue };
        let twice = holes.iter().any(|h| h.wall == wall && (h.at - at).abs() <= (h.width + width) / 4.0);
        if !twice {
            holes.push(Hole { wall, at, width: *width, window: *window });
        }
    }
    holes.sort_by(|a, b| (a.wall, a.at).partial_cmp(&(b.wall, b.at)).unwrap_or(std::cmp::Ordering::Equal));
    holes
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
                // Along the one, where the other lies: touching, overlapping, or with a gap too narrow to walk
                // through — where the drawing broke the line rather than left a doorway — is one wall.
                let along = |p: P2| a.dir().dot(p - a.start);
                let (mine, theirs) = ([0.0, a.length()], [along(b.start), along(b.end)]);
                let (t0, t1) = (theirs[0].min(theirs[1]), theirs[0].max(theirs[1]));
                if t0 - mine[1] >= NARROWEST || mine[0] - t1 >= NARROWEST {
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
    // A corner is two axes both stopping short of it, so taking the one to the other moves the mark that the next
    // is measured from: it is done again until nothing moves. The bound is there because nothing here may run away
    // with a drawing, not because a drawing is expected to need it.
    let mut settled = vec![[false; 2]; walls.len()];
    for _ in 0..ROUNDS {
        if !met(walls, how, &mut settled) {
            return;
        }
    }
}

/// The most rounds of [`meet`].
const ROUNDS: usize = 8;

/// One round of [`meet`]: whether any axis moved. An end that has found its crossing is settled and is not weighed
/// again, because stretching an axis leaves the line it runs along where it was, so the crossings do not move; what
/// the next round is for is the ends that found none, which a wall stretched since may now reach.
fn met(walls: &mut [Found], how: &How, settled: &mut [[bool; 2]]) -> bool {
    let was: Vec<Found> = walls.to_vec();
    let mut moved = false;
    for (i, wall) in walls.iter_mut().enumerate() {
        for end in [false, true] {
            if settled.get(i).is_some_and(|ends| ends[usize::from(end)]) {
                continue;
            }
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
                // Near this end of mine, and within the other's own stretch, grown by half of each wall. How near
                // is near: the faces that made the wall stop half the wall they run into short of the corner, and
                // where the drawing is looser than that, a gap at a corner that nobody could walk through is no
                // gap but a corner drawn carelessly.
                let reach = (mine.thickness + other.thickness) / 2.0;
                if p.dist(at) > (other.thickness / 2.0 + how.slack(other.thickness)).max(NARROWEST) {
                    continue;
                }
                let s = other.dir().dot(at - other.start);
                if s < -reach || s > other.length() + reach {
                    continue;
                }
                // A corner is where the two of them have least to go to meet, and not simply the crossing nearest
                // this end: a third wall whose axis runs through the end exactly, and which stops short of it, is
                // no corner, and holding the end there leaves the building open.
                let cost = p.dist(at) + (-s).max(s - other.length()).max(0.0);
                if best.is_none_or(|(near, _)| cost < near) {
                    best = Some((cost, at));
                }
            }
            // Stretched, never shortened to nothing: an end taken to a crossing that leaves less than half the
            // wall, or turns it round, is left where it was.
            if let Some((_, at)) = best {
                if let Some(ends) = settled.get_mut(i) {
                    ends[usize::from(end)] = true;
                }
                let (start, end_at) = match end {
                    true => (wall.start, at),
                    false => (at, wall.end),
                };
                if (end_at - start).dot(mine.dir()) >= mine.length() / 2.0 && (wall.start, wall.end) != (start, end_at) {
                    (wall.start, wall.end) = (start, end_at);
                    moved = true;
                }
            }
        }
    }
    moved
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

    /// What those lines were read as, by the default rule.
    fn reading(lines: &[(&str, [f64; 4])]) -> Reading {
        let (m, u) = with(lines);
        read(&m, &u, &How::default()).expect("it reads")
    }

    /// The walls found in those lines, by the default rule.
    fn found(lines: &[(&str, [f64; 4])]) -> Vec<Found> {
        reading(lines).walls
    }

    /// The four sides of a rectangle, as a draughtsman draws them.
    fn rect([x0, y0, x1, y1]: [f64; 4]) -> Vec<(&'static str, [f64; 4])> {
        vec![("A", [x0, y0, x1, y0]), ("A", [x1, y0, x1, y1]), ("A", [x1, y1, x0, y1]), ("A", [x0, y1, x0, y0])]
    }

    /// A room drawn as a survey draws one: four walls `t` thick, from the outside of it.
    fn room([x0, y0, x1, y1]: [f64; 4], t: f64) -> Vec<(&'static str, [f64; 4])> {
        rect([x0, y0, x1, y1]).into_iter().chain(rect([x0 + t, y0 + t, x1 - t, y1 - t])).collect()
    }

    /// One wall 6 m long and 300 thick, its faces broken from `a` to `b`: a doorway, as a drawing leaves one.
    fn doorway(a: f64, b: f64) -> Vec<(&'static str, [f64; 4])> {
        vec![("A", [0.0, 0.0, a, 0.0]), ("A", [b, 0.0, 6000.0, 0.0]), ("A", [0.0, 300.0, a, 300.0]), ("A", [b, 300.0, 6000.0, 300.0])]
    }

    #[test]
    fn a_gap_too_narrow_to_walk_through_is_where_the_drawing_broke_the_line_and_no_opening() {
        let r = reading(&doorway(2910.0, 3090.0));
        let [wall] = &r.walls[..] else { panic!("one wall, not {}: {:?}", r.walls.len(), r.walls) };
        assert!((wall.length() - 6000.0).abs() < 1e-6, "{}", wall.length());
        assert!(r.holes.is_empty(), "180 is no doorway: {:?}", r.holes);
        // The two halves of a wall drawn on two layers break at different places, and that is the same thing.
        let staggered = vec![
            ("A", [0.0, 0.0, 2910.0, 0.0]),
            ("B", [3090.0, 0.0, 6000.0, 0.0]),
            ("A", [0.0, 300.0, 3010.0, 300.0]),
            ("B", [3200.0, 300.0, 6000.0, 300.0]),
        ];
        let walls = found(&staggered);
        let [wall] = &walls[..] else { panic!("one wall, not {}: {walls:?}", walls.len()) };
        assert!((wall.length() - 6000.0).abs() < 1e-6, "{}", wall.length());
    }

    #[test]
    fn a_corner_is_where_the_two_walls_have_least_to_go_to_meet() {
        // A wall running east that stops 60 short of the wall it turns into, with a third wall whose axis passes
        // through its end exactly and which stops 200 short of it: the corner is the first, not the third.
        let lines = [
            ("A", [0.0, -150.0, 2000.0, -150.0]),
            ("A", [0.0, 150.0, 2000.0, 150.0]),
            ("A", [1860.0, -3000.0, 1860.0, 0.0]),
            ("A", [2260.0, -3000.0, 2260.0, 0.0]),
            ("A", [1850.0, 200.0, 1850.0, 3000.0]),
            ("A", [2150.0, 200.0, 2150.0, 3000.0]),
        ];
        let walls = found(&lines);
        assert_eq!(walls.len(), 3, "{walls:?}");
        let east = walls.iter().find(|w| w.dir().1.abs() < 0.5).expect("the wall running east");
        let far = east.start.0.max(east.end.0);
        assert!((far - 2060.0).abs() < 1e-6, "it should reach the wall it turns into, not stop at 2000: {far}");
    }

    #[test]
    fn a_wall_that_stops_a_doorway_short_of_the_wall_it_runs_into_is_taken_to_it_with_a_door_in_it() {
        // A room with a partition across it: its one face is drawn to the wall opposite and the other stops 850
        // short of it, because the piece of wall between the door and the corner is too short to draw as a face.
        let mut lines = room([0.0, 0.0, 6000.0, 4000.0], 300.0);
        lines.push(("A", [300.0, 1850.0, 5000.0, 1850.0]));
        lines.push(("A", [300.0, 1950.0, 5700.0, 1950.0]));
        let r = reading(&lines);
        assert_eq!(r.walls.len(), 5, "{:?}", r.walls);
        let part = r.walls.iter().position(|w| (w.thickness - 100.0).abs() < 1.0).expect("the partition");
        let wall = &r.walls[part];
        assert!((wall.length() - 5700.0).abs() < 1e-6, "it should reach the wall opposite: {}", wall.length());
        let [hole] = &r.holes[..] else { panic!("one opening, not {}: {:?}", r.holes.len(), r.holes) };
        assert_eq!(hole.wall, part);
        assert!(!hole.window, "nothing is drawn in it, so it is a door");
        assert!((hole.width - 700.0).abs() < 1e-6, "to the face of the wall it meets: {}", hole.width);
        assert!((hole.at - 5200.0).abs() < 1e-6, "at the end of the wall: {}", hole.at);
        // Asked for walls alone, the drawing is left as it is: the partition stops where it was drawn.
        let (m, u) = with(&lines);
        let bare = read(&m, &u, &How { openings: false, ..How::default() }).expect("it reads");
        let part = bare.walls.iter().find(|w| (w.thickness - 100.0).abs() < 1.0).expect("the partition");
        assert!((part.length() - 4850.0).abs() < 1e-6, "{}", part.length());
        // The wall it runs into stops half a wall short of its own corners, so the crossing may lie a little past
        // the end of it and still be the corner the two make.
        let mut l = room([0.0, 0.0, 6000.0, 4000.0], 300.0);
        l.push(("A", [300.0, 1850.0, 3850.0, 1850.0]));
        l.push(("A", [300.0, 1950.0, 3000.0, 1950.0]));
        l.push(("A", [3850.0, 1950.0, 3850.0, 3700.0]));
        l.push(("A", [3950.0, 1950.0, 3950.0, 3700.0]));
        let r = reading(&l);
        assert_eq!(r.walls.len(), 6, "{:?}", r.walls);
        let [hole] = &r.holes[..] else { panic!("one opening, not {}: {:?}", r.holes.len(), r.holes) };
        assert!((hole.width - 850.0).abs() < 1e-6, "{}", hole.width);
        // And where neither face is carried across, the wall stops in the room, because that is what was drawn.
        let mut stops = room([0.0, 0.0, 6000.0, 4000.0], 300.0);
        stops.push(("A", [300.0, 1850.0, 5000.0, 1850.0]));
        stops.push(("A", [300.0, 1950.0, 5000.0, 1950.0]));
        let r = reading(&stops);
        let part = r.walls.iter().find(|w| (w.thickness - 100.0).abs() < 1.0).expect("the partition");
        assert!((part.length() - 4850.0).abs() < 1e-6, "nothing was drawn across the gap: {}", part.length());
        assert!(r.holes.is_empty(), "{:?}", r.holes);
    }

    #[test]
    fn a_corner_is_closed_though_both_of_its_walls_stop_short_of_it() {
        // A wall running east that stops 240 short of the axis of the wall it turns into, which itself stops short
        // of the corner: taking the one to the other moves the mark, so it is done again until nothing moves.
        let lines = [
            ("A", [0.0, 0.0, 2000.0, 0.0]),
            ("A", [0.0, 300.0, 2000.0, 300.0]),
            ("A", [2180.0, 300.0, 2180.0, 2000.0]),
            ("A", [2300.0, 300.0, 2300.0, 2000.0]),
        ];
        let walls = found(&lines);
        assert_eq!(walls.len(), 2, "{walls:?}");
        let east = walls.iter().find(|w| w.dir().1.abs() < 0.5).expect("the wall running east");
        assert!((east.length() - 2240.0).abs() < 1e-6, "it reaches the axis it turns into: {}", east.length());
        let up = walls.iter().find(|w| w.dir().1.abs() > 0.5).expect("the wall running north");
        let low = up.start.1.min(up.end.1);
        assert!((low - 150.0).abs() < 1e-6, "and it comes down to meet it: {low}");
    }

    #[test]
    fn a_doorway_becomes_one_wall_with_a_door_in_it() {
        let r = reading(&doorway(2550.0, 3450.0));
        let [wall] = &r.walls[..] else { panic!("one wall, not {}: {:?}", r.walls.len(), r.walls) };
        assert!((wall.length() - 6000.0).abs() < 1e-6, "the wall runs past its doorway: {}", wall.length());
        let [hole] = &r.holes[..] else { panic!("one opening, not {}", r.holes.len()) };
        assert!(!hole.window, "nothing is drawn in it, so it is a door");
        assert!((hole.width - 900.0).abs() < 1e-6, "{}", hole.width);
        assert!((hole.at - 3000.0).abs() < 1e-6, "in the middle of the wall: {}", hole.at);
    }

    #[test]
    fn glass_drawn_in_an_opening_makes_it_a_window() {
        let mut lines = doorway(2550.0, 3450.0);
        lines.push(("A", [2550.0, 140.0, 3450.0, 140.0]));
        let r = reading(&lines);
        let [hole] = &r.holes[..] else { panic!("one opening, not {}", r.holes.len()) };
        assert!(hole.window, "glass inside it, so it is a window");
        // A line on the wall's face is the face, not glass: the doorway stays a door.
        let mut face = doorway(2550.0, 3450.0);
        face.push(("A", [2550.0, 300.0, 3450.0, 300.0]));
        assert!(!reading(&face).holes[0].window);
    }

    #[test]
    fn a_window_the_wall_runs_past_is_found_by_the_lines_at_its_jambs_and_the_glass_between_them() {
        // The other way a survey draws a window: the faces carry on across the opening, two lines are drawn across
        // the wall at the jambs, and the glass runs between them.
        let mut lines = vec![("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, 300.0, 6000.0, 300.0])];
        lines.extend([("A", [2600.0, 0.0, 2600.0, 300.0]), ("A", [3400.0, 0.0, 3400.0, 300.0])]);
        lines.extend([("A", [2600.0, 140.0, 3400.0, 140.0]), ("A", [2600.0, 160.0, 3400.0, 160.0])]);
        let r = reading(&lines);
        let [wall] = &r.walls[..] else { panic!("one wall, not {}", r.walls.len()) };
        assert!((wall.length() - 6000.0).abs() < 1e-6);
        let [hole] = &r.holes[..] else { panic!("one opening, not {}: {:?}", r.holes.len(), r.holes) };
        assert!(hole.window && (hole.width - 800.0).abs() < 1e-6 && (hole.at - 3000.0).abs() < 1e-6, "{hole:?}");
        // Without the glass the pair of lines says nothing: the space between two windows is a pair too.
        let bare: Vec<(&str, [f64; 4])> = lines[..4].to_vec();
        assert!(reading(&bare).holes.is_empty());
    }

    #[test]
    fn the_pier_between_two_windows_is_no_window_though_the_wall_draws_a_line_through_it() {
        // A wall 400 thick of two leaves, its faces carried across two windows, with the jambs drawn across it and
        // the glass between them. The line between the leaves lies inside the thickness and runs the whole of the
        // pier between the two windows, so it would be read as glass there and the pier as a third window.
        let mut lines = vec![("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, 400.0, 6000.0, 400.0])];
        for [from, to] in [[0.0, 1000.0], [2000.0, 3500.0], [4500.0, 6000.0]] {
            lines.push(("A", [from, 100.0, to, 100.0]));
        }
        for at in [1000.0, 2000.0, 3500.0, 4500.0] {
            lines.push(("A", [at, 0.0, at, 400.0]));
        }
        for [from, to] in [[1000.0, 2000.0], [3500.0, 4500.0]] {
            lines.push(("A", [from, 180.0, to, 180.0]));
            lines.push(("A", [from, 220.0, to, 220.0]));
        }
        let r = reading(&lines);
        let [wall] = &r.walls[..] else { panic!("one wall, not {}: {:?}", r.walls.len(), r.walls) };
        assert!((wall.thickness - 400.0).abs() < 1e-9, "{}", wall.thickness);
        let widths: Vec<f64> = r.holes.iter().map(|h| h.width).collect();
        assert_eq!(widths.len(), 2, "two windows and no pier between them: {:?}", r.holes);
        assert!(r.holes.iter().all(|h| h.window && (h.width - 1000.0).abs() < 1e-6), "{:?}", r.holes);
    }

    #[test]
    fn a_gap_too_narrow_or_too_wide_to_walk_through_is_no_opening() {
        // Too narrow to walk through is where the drawing broke the line, so it is one wall and no opening...
        let r = reading(&doorway(2850.0, 3150.0));
        assert_eq!((r.walls.len(), r.holes.len()), (1, 0), "{r:?}");
        // ...and too wide is where a wall stops, so it is two walls and no opening.
        let r = reading(&doorway(1000.0, 5000.0));
        assert_eq!((r.walls.len(), r.holes.len()), (2, 0), "{r:?}");
    }

    #[test]
    fn without_openings_a_doorway_is_left_as_the_drawing_has_it() {
        let (m, u) = with(&doorway(2550.0, 3450.0));
        let r = read(&m, &u, &How { openings: false, ..How::default() }).expect("it reads");
        assert_eq!(r.walls.len(), 2);
        assert!(r.holes.is_empty());
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
    fn a_window_drawn_in_a_wall_leaves_the_wall_whole_and_makes_no_wall_of_its_glass() {
        // A survey draws a window by carrying the wall's faces across its opening, breaking them at the jambs, and
        // drawing the glass down the middle: this is one 300 wall 6 m long with an 800-wide window in it. The glass
        // is 125 from a face and the far face is 300, so the nearest-face rule alone takes the two for a wall 130
        // thick and loses the wall they are drawn in.
        let wall = [
            ("A", [0.0, 0.0, 2600.0, 0.0]),
            ("A", [2600.0, 0.0, 3400.0, 0.0]),
            ("A", [3400.0, 0.0, 6000.0, 0.0]),
            ("A", [0.0, 300.0, 2600.0, 300.0]),
            ("A", [2600.0, 300.0, 3400.0, 300.0]),
            ("A", [3400.0, 300.0, 6000.0, 300.0]),
        ];
        let glass = [("A", [2600.0, 125.0, 3400.0, 125.0]), ("A", [2600.0, 175.0, 3400.0, 175.0])];
        let lines: Vec<(&str, [f64; 4])> = wall.into_iter().chain(glass).collect();
        let walls = found(&lines);
        let [one] = &walls[..] else { panic!("one wall, not {}: {walls:?}", walls.len()) };
        assert!((one.thickness - 300.0).abs() < 1e-9, "{}", one.thickness);
        assert!((one.length() - 6000.0).abs() < 1e-6, "{}", one.length());
    }

    #[test]
    fn a_space_wider_than_a_mark_inside_a_wall_is_two_walls_with_a_room_between_them() {
        // What leaves a window's glass out must not swallow a room: a corridor 600 wide between two walls 100 thick
        // is two walls, and so is a gap of 250, wider than anything drawn inside a wall.
        let two = |gap: f64| {
            found(&[
                ("A", [0.0, 0.0, 5000.0, 0.0]),
                ("A", [0.0, 100.0, 5000.0, 100.0]),
                ("A", [0.0, 100.0 + gap, 5000.0, 100.0 + gap]),
                ("A", [0.0, 200.0 + gap, 5000.0, 200.0 + gap]),
            ])
        };
        assert_eq!(two(600.0).len(), 2, "a corridor");
        assert_eq!(two(250.0).len(), 2, "a gap too wide to be inside a wall");
        // What it costs: a gap narrower than that is read as one wall as thick as all of it.
        let [one] = &two(150.0)[..] else { panic!("one wall") };
        assert!((one.thickness - 350.0).abs() < 1e-9, "{}", one.thickness);
    }

    #[test]
    fn a_wall_the_drawing_broke_in_two_is_one_wall_and_a_doorway_is_a_door_in_it() {
        let touching = found(&[("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, 300.0, 2500.0, 300.0]), ("A", [2500.0, 300.0, 6000.0, 300.0])]);
        let [one] = &touching[..] else { panic!("one wall, not {}", touching.len()) };
        assert!((one.length() - 6000.0).abs() < 1e-6, "{}", one.length());
        // A gap where both faces stop is a doorway, and a doorway is a door in a wall, not two walls.
        let r = reading(&doorway(2500.0, 3400.0));
        assert_eq!((r.walls.len(), r.holes.len()), (1, 1), "{r:?}");
        assert!((r.holes[0].width - 900.0).abs() < 1e-6, "{}", r.holes[0].width);
        // Asked for walls alone, the drawing is left as it is and the gap stays a gap.
        let (m, u) = with(&doorway(2500.0, 3400.0));
        let bare = read(&m, &u, &How { openings: false, ..How::default() }).expect("it reads");
        assert_eq!(bare.walls.len(), 2, "the doorway is not bricked up");
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
    fn a_wall_of_two_leaves_is_one_wall_as_thick_as_both_of_them() {
        // Three lines: the outside of the wall, the line between its leaves, and the inside of it.
        let three = |a: f64, b: f64| found(&[("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, a, 6000.0, a]), ("A", [0.0, b, 6000.0, b])]);
        // A hundred of facing on three hundred of wall, which is how the survey this was written against draws
        // every wall of its outside: one wall 400 thick, and the line between the leaves is the face of neither.
        let walls = three(100.0, 400.0);
        let [wall] = &walls[..] else { panic!("one wall, not {}: {walls:?}", walls.len()) };
        assert!((wall.thickness - 400.0).abs() < 1e-9, "{}", wall.thickness);
        assert!((wall.start.1 - 200.0).abs() < 1e-9, "down the middle of both: {}", wall.start.1);
        // The same the other way about: a wall 300 thick with a 50 lining against it is one wall 350 thick.
        let walls = three(300.0, 350.0);
        let [wall] = &walls[..] else { panic!("one wall, not {}: {walls:?}", walls.len()) };
        assert!((wall.thickness - 350.0).abs() < 1e-9, "{}", wall.thickness);
        // Evenly spaced, the three are a ladder and no wall of two leaves: it is the asymmetry that tells them
        // apart, here as with a stair's treads.
        assert!(three(280.0, 560.0).is_empty(), "{:?}", three(280.0, 560.0));
        // And a duct between two walls is two walls: the far face has a face of its own beyond it, so taking the
        // near pair leaves nothing over and there is nothing to put right.
        let duct = found(&[
            ("A", [0.0, 0.0, 6000.0, 0.0]),
            ("A", [0.0, 100.0, 6000.0, 100.0]),
            ("A", [0.0, 400.0, 6000.0, 400.0]),
            ("A", [0.0, 500.0, 6000.0, 500.0]),
        ]);
        assert_eq!(duct.len(), 2, "{duct:?}");
        assert!(duct.iter().all(|w| (w.thickness - 100.0).abs() < 1e-9), "{duct:?}");
    }

    #[test]
    fn a_line_that_meets_no_wall_is_the_land_round_the_building_and_no_wall_of_it() {
        // A survey draws the building and, around it, the land: the boundary of the plot, the kerb of the road, the
        // lines the surveyor took his levels along. Drawn as a pair they measure a wall's thickness like any other
        // pair, and what tells them from the building is that the building holds together and they hold on to
        // nothing.
        let mut lines = room([0.0, 0.0, 8000.0, 6000.0], 300.0);
        lines.extend([("CONFINE", [-9000.0, -5000.0, 14000.0, -5000.0]), ("CONFINE", [-9000.0, -4800.0, 14000.0, -4800.0])]);
        let r = reading(&lines);
        assert_eq!(r.walls.len(), 4, "the room's four walls and not the boundary: {:?}", r.walls);
        assert_eq!(r.alone, 1, "and it says one line stood alone");
        // Asked for them, it gives them: what it finds is a reading, and the user's word is the last one.
        let (m, u) = with(&lines);
        let all = read(&m, &u, &How { alone: true, ..How::default() }).expect("it reads");
        assert_eq!((all.walls.len(), all.alone), (5, 1), "{:?}", all.walls);
    }

    #[test]
    fn a_drawing_of_a_wall_on_its_own_is_that_wall() {
        // Nothing in it meets anything, so it is a drawing of a wall and not of a building, and the wall is taken.
        let one = found(&[("A", [0.0, 0.0, 5000.0, 0.0]), ("A", [0.0, 300.0, 5000.0, 300.0])]);
        assert_eq!(one.len(), 1, "{one:?}");
        // Two that keep apart are two walls still: a corridor is not a building that one of them is outside of.
        let two = found(&[
            ("A", [0.0, 0.0, 5000.0, 0.0]),
            ("A", [0.0, 300.0, 5000.0, 300.0]),
            ("A", [0.0, 2000.0, 5000.0, 2000.0]),
            ("A", [0.0, 2300.0, 5000.0, 2300.0]),
        ]);
        assert_eq!(two.len(), 2, "{two:?}");
    }

    #[test]
    fn a_block_shorter_than_it_is_thick_is_no_wall() {
        // Two lines 500 long and 700 apart: as far apart as a stone wall is thick and shorter than that, which is a
        // pier drawn as a box, a plinth, or the two jambs of an opening read as a pair of faces. Nothing is a wall
        // across itself.
        assert!(found(&[("A", [0.0, 0.0, 500.0, 0.0]), ("A", [0.0, 700.0, 500.0, 700.0])]).is_empty());
        // The same two lines turned the other way about are a wall: 700 long and 500 thick.
        let walls = found(&[("A", [0.0, 0.0, 700.0, 0.0]), ("A", [0.0, 500.0, 700.0, 500.0])]);
        let [wall] = &walls[..] else { panic!("one wall, not {}: {walls:?}", walls.len()) };
        assert!((wall.thickness - 500.0).abs() < 1e-9, "{}", wall.thickness);
        assert!((wall.length() - 700.0).abs() < 1e-6, "{}", wall.length());
    }

    #[test]
    fn two_walls_in_the_same_place_are_one_wall_and_the_longer_of_them_is_it() {
        // A survey draws a window's frame across the opening it fills, from the outside of the wall to the inside:
        // the frame's two lines are 400 apart where the wall is 100 thick, and the wall's own inner face stops at
        // the jambs, so the frame has nothing nearer and reads as a wall 400 thick lying over a wall 100 thick.
        let wall = [("A", [0.0, 0.0, 6000.0, 0.0]), ("A", [0.0, 100.0, 2600.0, 100.0]), ("A", [3400.0, 100.0, 6000.0, 100.0])];
        let frame = [("F", [2600.0, 0.0, 3400.0, 0.0]), ("F", [2600.0, 400.0, 3400.0, 400.0])];
        let lines: Vec<(&str, [f64; 4])> = wall.into_iter().chain(frame).collect();
        let r = reading(&lines);
        let [one] = &r.walls[..] else { panic!("one wall, not {}: {:?}", r.walls.len(), r.walls) };
        assert!((one.thickness - 100.0).abs() < 1e-9, "the wall, not the frame over it: {}", one.thickness);
        assert!((one.length() - 6000.0).abs() < 1e-6, "{}", one.length());
        let [hole] = &r.holes[..] else { panic!("the opening the frame fills, not {}", r.holes.len()) };
        assert!((hole.width - 800.0).abs() < 1e-6, "{}", hole.width);
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

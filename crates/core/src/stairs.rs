//! Stairs and ramps as runs and landings, and the railings along them: where each run goes and how
//! high, the landings between runs, the walk line up the middle, and the line a railing follows,
//! in from a side.

use crate::geom::{self, P2};
use crate::model::{Element, MOST_RISERS, Model, RAILING_INSET, Railing, Ramp, Side, Spiral, Stair, heading};
use crate::walls::Plane;

/// A run: from the middle of its foot, `start`, along unit `u`, `length` long, from height `z.0`
/// up to `z.1`. A stair's has `risers` risers, its first at its start and its last at its end. A
/// `bend` carries it round a centre instead of straight on, and then `length` is how far it runs
/// along its walk line and `u` only where it sets off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub start: P2,
    pub u: P2,
    pub length: f64,
    pub z: (f64, f64),
    pub risers: u32,
    pub bend: Option<Bend>,
}

/// How a run bends: the centre it is drawn about, and how far round it goes from its foot, degrees
/// counter-clockwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    pub c: P2,
    pub sweep: f64,
}

impl Run {
    pub fn end(&self) -> P2 {
        match self.bend {
            Some(b) => b.c + turned(self.start - b.c, b.sweep),
            None => self.start + self.u * self.length,
        }
    }

    /// Its outline, `width` wide, counter-clockwise: the rectangle of a straight run, or the ring
    /// between the two arcs of a bent one, drawn as a plan draws an arc.
    pub fn ring(&self, width: f64) -> Vec<P2> {
        let n = self.u.perp() * (width / 2.0);
        let Some(b) = self.bend else {
            return vec![self.start - n, self.end() - n, self.end() + n, self.start + n];
        };
        let mut ring = bow(b.c, self.start - n, 0.0, b.sweep);
        ring.extend(bow(b.c, self.start + n, b.sweep, 0.0));
        ccw(ring)
    }

    /// The centre a bent run is drawn about and how far out its walk line is from it, for its two
    /// curved sides to read as one surface each.
    pub fn round(&self) -> Option<(P2, f64)> {
        self.bend.map(|b| (b.c, b.c.dist(self.start)))
    }

    /// Its outline in plan, stretch by stretch, with the height of its deck along each, so that a
    /// plan can dash what stands over its cut plane: a straight run's four sides all stand at its
    /// foot, as a run of one height is drawn, while a bent one's two arcs climb as they go round and
    /// only its two ends stand at one height each.
    pub fn edges(&self, width: f64) -> Vec<(P2, P2, f64)> {
        let n = self.u.perp() * (width / 2.0);
        let Some(b) = self.bend else {
            return geom::ring_edges(&self.ring(width)).map(|(a, c)| (a, c, self.z.0)).collect();
        };
        let up = |deg: f64| self.z.0 + (self.z.1 - self.z.0) * if b.sweep == 0.0 { 0.0 } else { deg / b.sweep };
        let mut out = Vec::new();
        for side in [-1.0, 1.0] {
            let arc = bow(b.c, self.start + n * side, 0.0, b.sweep);
            let steps = arc.len().saturating_sub(1).max(1) as f64;
            for (i, w) in arc.windows(2).enumerate() {
                match w {
                    [a, c] => out.push((*a, *c, up(b.sweep * (i as f64 + 0.5) / steps))),
                    _ => continue,
                }
            }
        }
        let round = |p: P2| b.c + turned(p - b.c, b.sweep);
        out.push((self.start - n, self.start + n, self.z.0));
        out.push((round(self.start - n), round(self.start + n), self.z.1));
        out
    }

    /// Its deck in pieces, each a ring in plan and the plane its top lies in, `width` wide: one for
    /// a straight run, whose top is one sloping plane; for a bent one, as many as a plan draws its
    /// arc in, because the top of it is a helicoid and no plane holds more of that than a piece.
    pub fn slabs(&self, width: f64) -> Vec<(Vec<P2>, Plane)> {
        // The plane level across the run that holds `zp` at `p` and `zq` at `q`.
        let plane = |p: P2, q: P2, zp: f64, zq: f64| {
            let d = q - p;
            let g = if d.dot(d) > 1e-12 { d * ((zq - zp) / d.dot(d)) } else { P2(0.0, 0.0) };
            Plane { z: zp - g.dot(p), g }
        };
        let n = self.u.perp() * (width / 2.0);
        let Some(b) = self.bend else {
            return vec![(self.ring(width), plane(self.start, self.end(), self.z.0, self.z.1))];
        };
        let steps = ((b.sweep.abs() / ARC_STEP).ceil() as usize).clamp(1, ARC_MOST);
        (0..steps)
            .map(|i| {
                let (from, to) = (b.sweep * i as f64 / steps as f64, b.sweep * (i + 1) as f64 / steps as f64);
                let mut ring = bow(b.c, self.start - n, from, to);
                ring.extend(bow(b.c, self.start + n, to, from));
                let at = |deg: f64| b.c + turned(self.start - b.c, deg);
                let up = |deg: f64| self.z.0 + (self.z.1 - self.z.0) * if b.sweep == 0.0 { 0.0 } else { deg / b.sweep };
                (ccw(ring), plane(at(from), at(to), up(from), up(to)))
            })
            .collect()
    }
}

/// A landing: its outline, counter-clockwise, and its height.
#[derive(Clone, Debug, PartialEq)]
pub struct Pad {
    pub ring: Vec<P2>,
    pub z: f64,
}

/// A stair's or a ramp's runs and landings, how wide they are, and its walk line: up the middle of
/// each run, from its foot to its top, and across each landing, with the height of each point.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub runs: Vec<Run>,
    pub pads: Vec<Pad>,
    pub walk: Vec<(P2, f64)>,
    pub width: f64,
}

/// What a stair or a ramp does between two of its runs: how far round it turns, on how many treads
/// it turns there (none for a flat landing), how far from the centre of the turn the inside of them
/// is drawn (none for a fan, which fills the square of the landing instead), and how far it climbs
/// while it turns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub turn: f64,
    pub winders: u32,
    pub radius: f64,
    pub rise: f64,
}

/// Runs of `(length, heights, risers)` from `start` toward `angle` degrees, `width` wide, and a
/// landing after each but the last, as deep as they're wide, the next run going on as `turns` say:
/// straight on (0), left or right (90, -90), or back beside it, to its left or right (180, -180).
/// A quarter turn on fanned treads has that square cut into wedges instead, each a riser higher;
/// with a radius it is a curved run between two arcs about the centre it turns about.
fn chain(start: P2, angle: f64, width: f64, runs: Vec<(f64, (f64, f64), u32)>, turns: &[Turn]) -> Layout {
    let (mut at, mut u) = (start, heading(angle));
    let mut out = Layout { runs: Vec::new(), pads: Vec::new(), walk: Vec::new(), width };
    for (i, (length, z, risers)) in runs.into_iter().enumerate() {
        let run = Run { start: at, u, length, z, risers, bend: None };
        out.walk.extend([(run.start, z.0), (run.end(), z.1)]);
        out.runs.push(run);
        let Some(&Turn { turn, winders, radius, rise }) = turns.get(i) else { break };
        let (e, n, w) = (run.end(), u.perp(), width);
        // Across the landing from the run's right edge to its left, and how far it reaches past the
        // run: to the next run's far edge, beside it when it turns back.
        let side = n * if turn > 0.0 { 1.0 } else { -1.0 };
        // The treads a turn is carried on, where it is carried on treads at all: a curved run between two arcs
        // where it is drawn about a centre, and a fan in the square of the landing where it is not. Held to what a
        // stair is built with whoever asks: a count out of a file or a command is checked after the model is
        // changed and not before it is drawn, and a tread for each of four billion is no drawing.
        let winders = winders.min(crate::model::MOST_WINDERS);
        if winders >= 2 && radius > 0.0 && matches!(turn.abs(), 90.0 | 180.0) {
            (at, u) = (curve(&mut out, e, side, w, winders, radius, rise, turn, z.1), turned(u, turn));
            continue;
        }
        if winders >= 2 && turn.abs() == 90.0 {
            (at, u) = (fan(&mut out, e, u, side, w, winders, rise, z.1), side);
            continue;
        }
        let (ring, via, next, dir) = if turn == 0.0 {
            (vec![e - n * (w / 2.0), e - n * (w / 2.0) + u * w, e + n * (w / 2.0) + u * w, e + n * (w / 2.0)], vec![], e + u * w, u)
        } else if turn.abs() == 90.0 {
            let ring = vec![e - n * (w / 2.0), e - n * (w / 2.0) + u * w, e + n * (w / 2.0) + u * w, e + n * (w / 2.0)];
            (ring, vec![e + u * (w / 2.0)], e + u * (w / 2.0) + side * (w / 2.0), side)
        } else {
            let ring = vec![e - side * (w / 2.0), e - side * (w / 2.0) + u * w, e + side * (1.5 * w) + u * w, e + side * (1.5 * w)];
            (ring, vec![e + u * (w / 2.0), e + u * (w / 2.0) + side * w], e + side * w, u * -1.0)
        };
        out.pads.push(Pad { ring: ccw(ring), z: z.1 });
        out.walk.extend(via.into_iter().map(|p| (p, z.1)));
        (at, u) = (next, dir);
    }
    out
}

/// The fanned treads of a quarter turn, added to `out`: the square the landing would stand in, from
/// the run's end `e` along `u` and over to `side`, cut into `winders` wedges by rays out from the
/// corner on the inside of the turn, each wedge a share of `rise` above the run's top `z`, and the
/// walk line through the middle of each. Where the run above it leaves, which is where a flat
/// landing would have left it.
#[allow(clippy::too_many_arguments)]
fn fan(out: &mut Layout, e: P2, u: P2, side: P2, w: f64, winders: u32, rise: f64, z: f64) -> P2 {
    let winders = winders.max(2);
    let (pivot, a, b) = (e + side * (w / 2.0), side * -1.0, u);
    let (k, far) = (f64::from(winders), pivot + a * w + b * w);
    // Where a ray out from the corner, `t` of the way round the turn, meets the edge of the square,
    // and where the walk line crosses it, half the width out.
    let ray = |t: f64| {
        let (c, s) = ((t * std::f64::consts::FRAC_PI_2).cos(), (t * std::f64::consts::FRAC_PI_2).sin());
        (a * c + b * s, if s <= c { a * w + b * (w * s / c) } else { a * (w * c / s) + b * w })
    };
    for j in 0..winders {
        let (t0, t1) = (f64::from(j) / k, f64::from(j + 1) / k);
        let (first, last) = (ray(t0), ray(t1));
        let mut ring = vec![pivot, pivot + first.1];
        // Past the far corner of the square the wedge has four sides and not three.
        if t0 < 0.5 && t1 > 0.5 {
            ring.push(far);
        }
        ring.push(pivot + last.1);
        let up = z + rise * f64::from(j) / (k - 1.0);
        out.pads.push(Pad { ring: ccw(ring), z: up });
        out.walk.push((pivot + (first.0 + last.0).unit() * (w / 2.0), up));
    }
    pivot + b * (w / 2.0)
}

/// The curved run of a turn, added to `out`: `winders` treads between an arc `radius` from the
/// centre the turn is drawn about and one the stair's width beyond it, carrying the run from its end
/// `e` round by `turn` degrees, each tread a share of `rise` above that run's top `z`, and the walk
/// line through the middle of each. Where the run above it leaves, which is as far round as it
/// turns.
#[allow(clippy::too_many_arguments)]
fn curve(out: &mut Layout, e: P2, side: P2, w: f64, winders: u32, radius: f64, rise: f64, turn: f64, z: f64) -> P2 {
    // Held to what a stair is built with whoever asks: a count out of a file or a command is checked after the
    // model is changed and not before it is drawn, and a tread for each of four billion is no drawing. One tread
    // alone climbs nothing, so the share of the rise it carries is its own and not a part of none.
    let winders = winders.clamp(1, MOST_RISERS);
    // The centre it turns about stands past the edge of the run on the inside of the turn, and the two ends of
    // the run's last riser stand on the two arcs: the near one `radius` out, the far one the width beyond.
    let (k, c) = (f64::from(winders), e + side * (radius + w / 2.0));
    let (near, far) = (e + side * (w / 2.0), e - side * (w / 2.0));
    let round = |p: P2, deg: f64| c + turned(p - c, deg);
    for j in 0..winders {
        let (a, b) = (turn * f64::from(j) / k, turn * f64::from(j + 1) / k);
        let mut ring = bow(c, near, a, b);
        ring.extend(bow(c, far, b, a));
        let up = z + rise * f64::from(j) / (k - 1.0).max(1.0);
        out.pads.push(Pad { ring: ccw(ring), z: up });
        // Up the middle of the run, which is the middle of the end of the run below it carried round.
        out.walk.push((round(e, (a + b) / 2.0), up));
    }
    round(e, turn)
}

/// A spiral stair: the one curved run of it, from the middle of its foot `start` heading `angle` degrees,
/// `width` wide, climbing `risers` risers of `each` from `base`, its going at the walk line `going`. Every
/// riser of it is a tread of the turn, so it has no straight run at all: `risers - 1` treads, the last of
/// them a riser below the floor above, and the walk line from the middle of its foot on one floor to the
/// middle of its top riser on the other.
///
/// How far round it goes follows from the rest, as [`Spiral`] says: each tread turns as far as the going
/// takes it at the walk line, half the width out past `radius` from the centre it turns about.
#[allow(clippy::too_many_arguments)]
fn helix(start: P2, angle: f64, width: f64, going: f64, risers: u32, each: f64, base: f64, sp: Spiral) -> Layout {
    let mut out = Layout { runs: Vec::new(), pads: Vec::new(), walk: Vec::new(), width };
    let u = heading(angle);
    let side = u.perp() * if matches!(sp.to, Side::Left) { 1.0 } else { -1.0 };
    let treads = risers.saturating_sub(1).clamp(1, MOST_RISERS);
    // How far one tread turns: its going, as an angle at the walk line. A width of none and a centre on the walk
    // line are no stair, and the model refuses both, but a drawing is asked for before a model is checked.
    let out_to = sp.radius + width / 2.0;
    let step = if out_to > 0.0 { (going / out_to).to_degrees() } else { 0.0 };
    let turn = step.clamp(0.0, 360.0) * f64::from(treads) * if matches!(sp.to, Side::Left) { 1.0 } else { -1.0 };
    out.walk.push((start, base));
    let top = curve(&mut out, start, side, width, treads, sp.radius, each * f64::from(treads.saturating_sub(1)), turn, base + each);
    out.walk.push((top, base + each * f64::from(risers)));
    out
}

/// A helical ramp: the one curved run of it, from the middle of its foot `start` heading `angle`
/// degrees, `width` wide, rising from `z.0` to `z.1` at `slope` percent, about the centre [`Spiral`]
/// puts to one side of its foot. It has no landing and no straight run at all, and its deck slopes
/// the whole way round, so a plan draws it as the one ring between two arcs.
///
/// How far round it goes follows from the rest, as [`Spiral`] says: the slope says how long the run
/// is, and that length at the walk line, half the width out past `radius`, is how far round it
/// reaches.
fn coil(start: P2, angle: f64, width: f64, z: (f64, f64), slope: f64, sp: Spiral) -> Layout {
    let u = heading(angle);
    let way = if matches!(sp.to, Side::Left) { 1.0 } else { -1.0 };
    // How long the run is, as its slope makes it; a slope of none is no ramp, and the model refuses it, but a
    // drawing is asked for before a model is checked.
    let k = if slope > 0.0 { slope / 100.0 } else { 1.0 };
    let length = ((z.1 - z.0) / k).max(0.0);
    // How far round that length reaches at the walk line. A width of none and a centre on the walk line are
    // no ramp either, and however far round a refused one would go, it is drawn as far as a drawing can.
    let out_to = sp.radius + width / 2.0;
    let sweep = if out_to > 0.0 { (length / out_to).to_degrees().clamp(0.0, COIL_DRAWN) * way } else { 0.0 };
    let c = start + u.perp() * way * out_to;
    let run = Run { start, u, length, z, risers: 0, bend: Some(Bend { c, sweep }) };
    // Up the middle of it, point by point as the arc is drawn, each as far up as it is round.
    let steps = ((sweep.abs() / ARC_STEP).ceil() as usize).clamp(1, ARC_MOST);
    let walk = (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            (c + turned(start - c, sweep * t), z.0 + (z.1 - z.0) * t)
        })
        .collect();
    Layout { runs: vec![run], pads: Vec::new(), walk, width }
}

/// How far round a helical ramp is drawn at most, in degrees: the model refuses one that goes
/// further than [`crate::model::COIL_MOST`], but a drawing is asked for before a model is checked,
/// and a coil of four billion turns is no drawing.
const COIL_DRAWN: f64 = 1440.0;

/// How many degrees apart the points of an arc are drawn, and how many of them there may be.
const ARC_STEP: f64 = 5.0;
const ARC_MOST: usize = 144;

/// An arc about `c`, drawn as a plan draws one: `p` carried from `from` round to `to` degrees, the
/// points along it no more than [`ARC_STEP`] apart and no more than [`ARC_MOST`] of them.
fn bow(c: P2, p: P2, from: f64, to: f64) -> Vec<P2> {
    let steps = (((to - from).abs() / ARC_STEP).ceil() as usize).clamp(1, ARC_MOST);
    (0..=steps).map(|i| c + turned(p - c, from + (to - from) * i as f64 / steps as f64)).collect()
}

/// `p` turned `deg` degrees counter-clockwise about the origin.
fn turned(p: P2, deg: f64) -> P2 {
    let (sin, cos) = deg.to_radians().sin_cos();
    P2(p.0 * cos - p.1 * sin, p.0 * sin + p.1 * cos)
}

fn ccw(ring: Vec<P2>) -> Vec<P2> {
    if geom::area(&ring) < 0.0 { ring.into_iter().rev().collect() } else { ring }
}

/// A stair's runs: its risers split at its landings, each run a tread shorter than its risers, and
/// the risers a fan climbs turned on between two runs instead of walked along either.
pub fn stair(m: &Model, st: &Stair) -> Layout {
    let (n, each, _) = m.flight(st);
    let base = m.elevation(st.level);
    if let Some(sp) = st.spiral {
        return helix(st.start, st.angle, st.width, st.tread, n, each, base, sp);
    }
    let mut spans: Vec<(u32, u32)> = Vec::new();
    let mut from = 0u32;
    // A fan's own risers, held to what a stair is built with as [`fan`] holds them, so the runs it
    // leaves between them are the runs it draws between them.
    let fanned = |l: &crate::model::Landing| match l.winders {
        0 | 1 => 0,
        on => on.min(crate::model::MOST_WINDERS) - 1,
    };
    for l in &st.landings {
        let at = (l.at.max(0.0).min(f64::from(n))) as u32;
        spans.push((from.min(at), at));
        from = at.saturating_add(fanned(l));
    }
    spans.push((from.min(n), n));
    let runs = spans
        .iter()
        .map(|&(a, b)| {
            (
                f64::from(b.saturating_sub(a).saturating_sub(1)) * st.tread,
                (base + f64::from(a) * each, base + f64::from(b) * each),
                b.saturating_sub(a),
            )
        })
        .collect();
    let turns: Vec<Turn> = st
        .landings
        .iter()
        .map(|l| Turn { turn: l.turn, winders: l.winders, radius: l.radius, rise: f64::from(fanned(l)) * each })
        .collect();
    chain(st.start, st.angle, st.width, runs, &turns)
}

/// A ramp's runs: its rise split at its landings' heights, each run as long as its slope makes it;
/// or, where it is helical, the one curved run of it.
pub fn ramp(m: &Model, r: &Ramp) -> Layout {
    let (base, top) = m.span(r.level, 0.0, r.top_level, r.height);
    if let Some(sp) = r.spiral {
        return coil(r.start, r.angle, r.width, (base, top), r.slope, sp);
    }
    let k = if r.slope > 0.0 { r.slope / 100.0 } else { 1.0 };
    let mut ups: Vec<f64> = vec![0.0];
    ups.extend(r.landings.iter().map(|l| l.at));
    ups.push(top - base);
    let runs = ups
        .windows(2)
        .filter_map(|w| match *w {
            [a, b] => Some(((b - a) / k, (base + a, base + b), 0)),
            _ => None,
        })
        .collect();
    let turns: Vec<Turn> = r.landings.iter().map(|l| Turn { turn: l.turn, winders: 0, radius: 0.0, rise: 0.0 }).collect();
    chain(r.start, r.angle, r.width, runs, &turns)
}

/// Element `e`'s layout, a stair's or a ramp's.
pub fn layout(m: &Model, e: &Element) -> Option<Layout> {
    match e {
        Element::Stair(st) => Some(stair(m, st)),
        Element::Ramp(r) => Some(ramp(m, r)),
        _ => None,
    }
}

/// The line a railing follows, in plan, with the height of what it runs along at each point: on a
/// stair or a ramp, its walk line moved out to the side, [`RAILING_INSET`] in from its edge; else
/// its path, on its level and `base_offset` above it.
pub fn railing(m: &Model, rl: &Railing) -> Vec<(P2, f64)> {
    let host = rl.host.and_then(|h| m.elements.get(h)).and_then(|e| layout(m, e));
    match (host, rl.side) {
        (Some(l), Some(side)) => {
            let d = (l.width / 2.0 - RAILING_INSET).max(0.0) * if side == Side::Left { 1.0 } else { -1.0 };
            offset(&l.walk, d)
        }
        _ => {
            let z = m.elevation(rl.level) + rl.base_offset;
            rl.path.iter().map(|p| (*p, z)).collect()
        }
    }
}

/// A railing's solids, each a ring in plan between two planes: its rail along each stretch of its
/// line, 50 mm square, its type's height over the line; and its balusters, 20 mm square and its
/// type's `spacing` apart, from the line up under the rail.
pub fn railing_parts(m: &Model, rl: &Railing) -> Vec<(Vec<P2>, Plane, Plane)> {
    let Some(t) = m.types.railing.get(&rl.type_name) else { return Vec::new() };
    let mut out = Vec::new();
    // How far along the next stretch the next baluster stands.
    let mut next = 0.0;
    for w in railing(m, rl).windows(2) {
        let &[(p, zp), (q, zq)] = w else { continue };
        let len = p.dist(q);
        if len < 1e-9 {
            continue;
        }
        let u = (q - p) * (1.0 / len);
        let (n, g) = (u.perp() * 25.0, u * ((zq - zp) / len));
        let top = Plane { z: zp + t.height - g.dot(p), g };
        out.push((vec![p - n, q - n, q + n, p + n], Plane { z: top.z - 50.0, g }, top));
        if t.spacing > 0.0 {
            while next <= len {
                let (at, z) = (p + u * next, zp + (zq - zp) * next / len);
                let (a, b) = (u * 10.0, u.perp() * 10.0);
                out.push((vec![at - a - b, at + a - b, at + a + b, at - a + b], Plane::flat(z), Plane::flat(z + t.height - 50.0)));
                next += t.spacing;
            }
            next -= len;
        }
    }
    out
}

/// `line` moved `d` to its left, its corners mitred, each point keeping its height.
fn offset(line: &[(P2, f64)], d: f64) -> Vec<(P2, f64)> {
    let mut pts: Vec<(P2, f64)> = line.to_vec();
    pts.dedup_by(|b, a| a.0.dist(b.0) < 1e-9);
    let dirs: Vec<P2> = pts.windows(2).filter_map(|w| Some((w.get(1)?.0 - w.first()?.0).unit())).collect();
    pts.iter()
        .enumerate()
        .map(|(i, &(p, z))| {
            let before = i.checked_sub(1).and_then(|j| dirs.get(j)).copied();
            let after = dirs.get(i).copied();
            let shift = match (before, after) {
                (Some(a), Some(b)) if a.dot(b) > -1.0 + 1e-9 => (a.perp() + b.perp()) * (d / (1.0 + a.dot(b))),
                (Some(a), _) | (None, Some(a)) => a.perp() * d,
                (None, None) => P2(0.0, 0.0),
            };
            (p + shift, z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_moved_aside_keeps_its_corners_square() {
        let line = [(P2(0.0, 0.0), 0.0), (P2(0.0, 1000.0), 500.0), (P2(1000.0, 1000.0), 500.0)];
        // Going north, then east: to the left (west, then north), the corner moves out diagonally.
        let left = offset(&line, 100.0);
        assert_eq!(left.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [P2(-100.0, 0.0), P2(-100.0, 1100.0), P2(1000.0, 1100.0)]);
        assert_eq!(left.iter().map(|(_, z)| *z).collect::<Vec<_>>(), [0.0, 500.0, 500.0]);
        let right = offset(&line, -100.0);
        assert_eq!(right[1].0, P2(100.0, 900.0), "the inside corner moves in");
    }

    #[test]
    fn landings_turn_the_next_run_square_or_back() {
        let runs = vec![(1000.0, (0.0, 500.0), 0), (1000.0, (500.0, 1000.0), 0)];
        let near = |a: P2, b: P2| a.dist(b) < 1e-9;
        let l = chain(P2(0.0, 0.0), 90.0, 1000.0, runs.clone(), &[Turn { turn: -90.0, winders: 0, radius: 0.0, rise: 0.0 }]);
        // North, then a metre square landing, then east from its middle.
        assert!(near(l.runs[1].start, P2(500.0, 1500.0)) && near(l.runs[1].u, P2(1.0, 0.0)), "{:?}", l.runs[1]);
        assert!((geom::area(&l.pads[0].ring) - 1e6).abs() < 1e-6);
        let back = chain(P2(0.0, 0.0), 90.0, 1000.0, runs.clone(), &[Turn { turn: 180.0, winders: 0, radius: 0.0, rise: 0.0 }]);
        assert!(near(back.runs[1].start, P2(-1000.0, 1000.0)) && near(back.runs[1].u, P2(0.0, -1.0)), "{:?}", back.runs[1]);
        assert!((geom::area(&back.pads[0].ring) - 2e6).abs() < 1e-6, "across both runs");
        let on = chain(P2(0.0, 0.0), 90.0, 1000.0, runs.clone(), &[Turn { turn: 0.0, winders: 0, radius: 0.0, rise: 0.0 }]);
        assert!(near(on.runs[1].start, P2(0.0, 2000.0)));
        assert_eq!(on.walk.len(), 4);
        // The same quarter turn carried on three fanned treads: the same square, cut into three from the corner
        // on the inside of the turn, each a share of the rise above the run below it, and the run above leaving
        // where the flat landing left it.
        let fan = chain(P2(0.0, 0.0), 90.0, 1000.0, runs, &[Turn { turn: -90.0, winders: 3, radius: 0.0, rise: 200.0 }]);
        assert!(near(fan.runs[1].start, P2(500.0, 1500.0)) && near(fan.runs[1].u, P2(1.0, 0.0)), "{:?}", fan.runs[1]);
        assert_eq!(fan.pads.len(), 3);
        assert!((fan.pads.iter().map(|p| geom::area(&p.ring)).sum::<f64>() - 1e6).abs() < 1e-6, "the whole square and no more");
        assert_eq!(fan.pads.iter().map(|p| p.z).collect::<Vec<_>>(), vec![500.0, 600.0, 700.0]);
        // The walk line goes through the middle of each of them, half the width out from the corner.
        let turning: Vec<P2> = fan.walk.iter().skip(2).take(3).map(|(p, _)| *p).collect();
        assert!(turning.iter().all(|p| (p.dist(P2(500.0, 1000.0)) - 500.0).abs() < 1e-6), "{turning:?}");
    }

    #[test]
    fn a_radius_makes_the_treads_of_a_turn_a_curved_run_between_two_arcs() {
        let runs = vec![(1000.0, (0.0, 500.0), 0), (1000.0, (500.0, 1000.0), 0)];
        let near = |a: P2, b: P2| a.dist(b) < 1e-6;
        // North, then a quarter turn to the right on three treads drawn about a centre 500 inside the run's
        // right edge: the centre stands a metre east of the end of the run, and the run above leaves a metre
        // north of it, going east.
        let turn = Turn { turn: -90.0, winders: 3, radius: 500.0, rise: 200.0 };
        let l = chain(P2(0.0, 0.0), 90.0, 1000.0, runs.clone(), &[turn]);
        assert!(near(l.runs[1].start, P2(1000.0, 2000.0)) && near(l.runs[1].u, P2(1.0, 0.0)), "{:?}", l.runs[1]);
        assert_eq!(l.pads.len(), 3);
        // The three of them cover the quarter of the ring between the two arcs and no more, give or take what a
        // curve drawn as the chords of it falls short by, which is a part in a thousand of it either side.
        let ring = std::f64::consts::PI * (1500.0 * 1500.0 - 500.0 * 500.0) / 4.0;
        let laid: f64 = l.pads.iter().map(|p| geom::area(&p.ring).abs()).sum();
        assert!(ring - laid > 0.0 && ring - laid < ring / 500.0, "{laid} of {ring}");
        assert_eq!(l.pads.iter().map(|p| p.z).collect::<Vec<_>>(), vec![500.0, 600.0, 700.0]);
        // The walk line goes up the middle of the run, which is the mid radius of the two arcs.
        let turning: Vec<P2> = l.walk.iter().skip(2).take(3).map(|(p, _)| *p).collect();
        assert!(turning.iter().all(|p| (p.dist(P2(1000.0, 1000.0)) - 1000.0).abs() < 1e-6), "{turning:?}");
        // A half turn about the same centre brings it back beside the run, the two walk lines 2000 apart.
        let back = chain(P2(0.0, 0.0), 90.0, 1000.0, runs.clone(), &[Turn { turn: 180.0, ..turn }]);
        assert!(near(back.runs[1].start, P2(-2000.0, 1000.0)) && near(back.runs[1].u, P2(0.0, -1.0)), "{:?}", back.runs[1]);
        // With no radius at all the same quarter turn is the fan, and the run above it leaves where that left it.
        let fan = chain(P2(0.0, 0.0), 90.0, 1000.0, runs, &[Turn { radius: 0.0, ..turn }]);
        assert!(near(fan.runs[1].start, P2(500.0, 1500.0)), "{:?}", fan.runs[1]);
    }

    #[test]
    fn a_spiral_stair_is_one_curved_run_of_treads_from_its_foot_to_its_top() {
        let near = |a: P2, b: P2| a.dist(b) < 1e-6;
        // Five risers of 200 from the middle of its foot at the origin, heading north, a metre wide, the inside of
        // its treads 500 from the centre it turns about: the centre stands a metre west of its foot, that and half
        // its width, and the walk line up the middle of it is a metre out from there.
        let sp = Spiral { radius: 500.0, to: Side::Left };
        let l = helix(P2(0.0, 0.0), 90.0, 1000.0, 300.0, 5, 200.0, 0.0, sp);
        assert!(l.runs.is_empty(), "a spiral stair has no straight run");
        // Four treads for five risers, each a riser above the last and the top one a riser below the floor above.
        assert_eq!(l.pads.len(), 4);
        assert_eq!(l.pads.iter().map(|p| p.z).collect::<Vec<_>>(), vec![200.0, 400.0, 600.0, 800.0]);
        // Each turns as far as its going takes it a metre out: 300 mm of arc, which is 0.3 of a radian. Together
        // they cover that much of the ring between the two arcs and no more, give or take what a curve drawn as
        // the chords of it falls short by.
        let step = 0.3f64.to_degrees();
        let sector = 4.0 * 0.3 / 2.0 * (1500.0 * 1500.0 - 500.0 * 500.0);
        let laid: f64 = l.pads.iter().map(|p| geom::area(&p.ring).abs()).sum();
        assert!(sector - laid > 0.0 && sector - laid < sector / 500.0, "{laid} of {sector}");
        // Its walk line runs from the middle of its foot on the floor to the middle of its top riser on the floor
        // above, every point of it a metre out from the centre.
        let c = P2(-1000.0, 0.0);
        assert_eq!(l.walk.first().copied(), Some((P2(0.0, 0.0), 0.0)));
        assert_eq!(l.walk.last().map(|(_, z)| *z), Some(1000.0));
        assert!(l.walk.iter().all(|(p, _)| (p.dist(c) - 1000.0).abs() < 1e-6), "{:?}", l.walk);
        assert!(l.walk.last().is_some_and(|(p, _)| near(*p, c + turned(P2(0.0, 0.0) - c, 4.0 * step))), "{:?}", l.walk.last());
        // Turning the other way puts the centre as far the other side of its foot, and carries it round that way.
        let right = helix(P2(0.0, 0.0), 90.0, 1000.0, 300.0, 5, 200.0, 0.0, Spiral { to: Side::Right, ..sp });
        assert!(right.walk.iter().all(|(p, _)| (p.dist(P2(1000.0, 0.0)) - 1000.0).abs() < 1e-6), "{:?}", right.walk);
        assert!(right.walk.last().is_some_and(|(p, _)| p.0 > 0.0 && p.1 > 0.0), "{:?}", right.walk.last());
    }

    #[test]
    fn a_helical_ramp_is_one_curved_run_that_slopes_the_whole_way_round() {
        // Three metres up at 10%, from the middle of its foot at the origin heading east, three metres wide, the
        // inside of it drawn four metres from the centre it coils about: the centre stands 5.5 m north of its
        // foot, that and half its width, which is how far out its walk line runs.
        let sp = Spiral { radius: 4000.0, to: Side::Left };
        let l = coil(P2(0.0, 0.0), 0.0, 3000.0, (0.0, 3000.0), 10.0, sp);
        let c = P2(0.0, 5500.0);
        assert!(l.pads.is_empty(), "a helical ramp has no landing");
        // One run, as long as its slope makes it, bent round that centre as far as that length reaches there.
        let sweep = (30000.0 / 5500.0f64).to_degrees();
        assert_eq!(l.runs.len(), 1);
        let run = l.runs[0];
        assert_eq!((run.length, run.z, run.risers), (30000.0, (0.0, 3000.0), 0));
        assert_eq!(run.bend, Some(Bend { c, sweep }));
        assert_eq!(run.round(), Some((c, 5500.0)));
        // Its outline is the one ring between the two arcs, and covers the sector they cut and no more, give or
        // take what a curve drawn as the chords of it falls short by.
        let sector = sweep.to_radians() / 2.0 * (7000.0 * 7000.0 - 4000.0 * 4000.0);
        let laid = geom::area(&run.ring(3000.0)).abs();
        assert!(sector - laid > 0.0 && sector - laid < sector / 500.0, "{laid} of {sector}");
        // Its walk line runs from the middle of its foot to the middle of its top, as far up as it is round at
        // every point of it, every one of them 5.5 m out from the centre.
        assert_eq!(l.walk.first().copied(), Some((P2(0.0, 0.0), 0.0)));
        assert_eq!(l.walk.last().map(|(_, z)| *z), Some(3000.0));
        assert!(l.walk.iter().all(|(p, _)| (p.dist(c) - 5500.0).abs() < 1e-6), "{:?}", l.walk);
        let half = l.walk.iter().min_by(|a, b| (a.1 - 1500.0).abs().total_cmp(&(b.1 - 1500.0).abs()));
        assert!(half.is_some_and(|(p, _)| p.dist(c + turned(P2(0.0, 0.0) - c, sweep / 2.0)) < 1500.0), "{half:?}");
        // Its deck in pieces, each a plane level across the run: at the foot of the first it stands at the foot of
        // the ramp, at the head of the last at its top, and each piece climbs its share of the rise.
        let slabs = run.slabs(3000.0);
        assert_eq!(slabs.len(), (sweep / 5.0).ceil() as usize);
        let at = |deg: f64| c + turned(P2(0.0, 0.0) - c, deg);
        for (i, (_, top)) in slabs.iter().enumerate() {
            let (from, to) = (sweep * i as f64 / slabs.len() as f64, sweep * (i + 1) as f64 / slabs.len() as f64);
            assert!((top.at(at(from)) - 3000.0 * from / sweep).abs() < 1e-6, "{i}: {}", top.at(at(from)));
            assert!((top.at(at(to)) - 3000.0 * to / sweep).abs() < 1e-6, "{i}: {}", top.at(at(to)));
            // Level square to the way it runs: a piece of it falls along the chord it covers and not across it.
            let along = (at(to) - at(from)).unit();
            assert!(top.g.cross(along).abs() < 1e-9, "{i}: {:?} across {along:?}", top.g);
        }
        // No flat piece of a helicoid is level along both the radial lines it meets its neighbours on, so where two
        // of them meet they stand at the same height on the walk line and a hair apart at the edges: here, a
        // centimetre for every metre out to the side at a tenth of a slope, which is the slope across the turn
        // of one piece.
        let shared = at(sweep / slabs.len() as f64);
        let out = (shared - c).unit() * 1500.0;
        if let [(_, first), (_, second), ..] = slabs.as_slice() {
            assert!((first.at(shared) - second.at(shared)).abs() < 1e-9, "level where they meet");
            let step = (first.at(shared + out) - second.at(shared + out)).abs();
            assert!(step > 0.0 && step < 1.5 * 10.0, "{step} apart at the edge");
        }
        // What a plan draws of it: its two arcs stretch by stretch, each with the height of the deck along it, so
        // the plan dashes them from where they climb over its cut plane, and its two ends, one at each height.
        let drawn = run.edges(3000.0);
        assert_eq!(drawn.len(), 2 * (slabs.len() + 1), "both arcs, and the two ends");
        assert!(drawn.iter().rev().take(2).map(|(_, _, z)| *z).eq([3000.0, 0.0]), "its ends stand at its ends' heights");
        let climbing: Vec<f64> = drawn.iter().take(drawn.len() - 2).map(|(_, _, z)| *z).collect();
        assert!(climbing.iter().all(|z| (0.0..=3000.0).contains(z)) && climbing.windows(2).any(|w| w[1] > w[0]), "{climbing:?}");
        // Coiling the other way puts the centre as far the other side of its foot, and carries it round that way.
        let right = coil(P2(0.0, 0.0), 0.0, 3000.0, (0.0, 3000.0), 10.0, Spiral { to: Side::Right, ..sp });
        assert_eq!(right.runs.first().and_then(|r| r.bend), Some(Bend { c: P2(0.0, -5500.0), sweep: -sweep }));
        assert!(right.walk.iter().all(|(p, _)| (p.dist(P2(0.0, -5500.0)) - 5500.0).abs() < 1e-6), "{:?}", right.walk);
    }
}

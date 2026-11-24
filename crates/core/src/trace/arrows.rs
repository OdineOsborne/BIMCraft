//! The line a plan draws up a stair to say which way it climbs: no face of a wall, though it pairs as one.
//!
//! A survey draws a stair's walking line down the middle of the flight and ends it in an arrowhead where the stair
//! arrives. It runs as long as the flight, alongside the stair's walls, half the stair's width from the wall beside
//! it, so the nearest-face rule takes it and that wall's face for a wall as thick as half the stair: on the permesso
//! di costruire this was written against, the walking line of the stair to the upper floor paired with the edge of
//! its landing, and the hall got a wall 500 thick across the landing with a door in it. What tells it from a face is
//! the arrowhead, which nothing else in a building's plan has: two short lines drawn back from one end, alike in
//! length and splayed at the same angle either side of it. That line and the lines it is drawn on from, end to end,
//! are the walking line ([`walking`]), and none of them is a face.

use super::How;
use crate::geom::P2;
use std::collections::HashMap;

/// The longest barb of an arrowhead, in millimetres: an arrowhead is drawn a hand's width long, and two lines longer
/// than this meeting at a point are a corner of something.
const BARB: f64 = 500.0;

/// The narrowest and widest a barb is splayed from its arrow, in degrees.
const SPLAY: (f64, f64) = (10.0, 60.0);

/// Which of `lines` are a walking line: a line ending in an arrowhead, and every line it is drawn on from where only
/// the two of them meet, together with the barbs of the arrowhead.
pub fn walking(lines: &[(P2, P2)], how: &How) -> Vec<bool> {
    let near = how.thin / 10.0;
    // Where each line's ends are, laid out in squares as wide as two ends may be apart and still meet.
    let cell = |p: P2| ((p.0 / near).floor() as i64, (p.1 / near).floor() as i64);
    let mut at: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
    for (i, (p, q)) in lines.iter().enumerate() {
        if p.dist(*q) > 0.0 {
            at.entry(cell(*p)).or_default().push((i, false));
            at.entry(cell(*q)).or_default().push((i, true));
        }
    }
    let end = |i: usize, far: bool| if far { lines[i].1 } else { lines[i].0 };
    // The lines with an end at `p`, and which of their ends it is.
    let meeting = |p: P2| -> Vec<(usize, bool)> {
        let (cx, cy) = cell(p);
        let mut found = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for (i, far) in at.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if end(*i, *far).dist(p) <= near && !found.iter().any(|(j, _)| j == i) {
                        found.push((*i, *far));
                    }
                }
            }
        }
        found
    };
    let mut walk = vec![false; lines.len()];
    let (narrow, wide) = (SPLAY.0.to_radians().cos(), SPLAY.1.to_radians().cos());
    for (i, (p, q)) in lines.iter().enumerate() {
        for (tip, other) in [(*p, *q), (*q, *p)] {
            let shaft = (other - tip).unit();
            let len = tip.dist(other);
            let meet = meeting(tip);
            if meet.len() != 3 {
                continue;
            }
            // The two barbs: alike, short, splayed from the shaft the same way to either side of it.
            let barbs: Vec<(usize, P2, f64)> = meet
                .iter()
                .filter(|(j, _)| *j != i)
                .map(|(j, far)| {
                    let back = end(*j, !*far);
                    (*j, (back - tip).unit(), tip.dist(back))
                })
                .collect();
            let [(b0, d0, l0), (b1, d1, l1)] = barbs[..] else { continue };
            let alike = (l0 - l1).abs() <= l0.max(l1) / 10.0 && l0.max(l1) <= BARB && l0.max(l1) < len;
            let splayed = |d: P2| (wide..=narrow).contains(&d.dot(shaft));
            let either_side = shaft.cross(d0) * shaft.cross(d1) < 0.0;
            let same_angle = (d0.dot(shaft) - d1.dot(shaft)).abs() <= 0.1;
            if !(alike && splayed(d0) && splayed(d1) && either_side && same_angle) {
                continue;
            }
            walk[b0] = true;
            walk[b1] = true;
            // The arrow, and the line it is drawn on from, for as long as nothing else meets it where they join.
            let (mut line, mut from) = (i, other);
            for _ in 0..lines.len() {
                walk[line] = true;
                let meet = meeting(from);
                let [(a, fa), (b, fb)] = meet[..] else { break };
                let (next, far) = if a == line { (b, fb) } else { (a, fa) };
                if walk[next] {
                    break;
                }
                line = next;
                from = end(next, !far);
            }
        }
    }
    walk
}

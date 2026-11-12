//! The ground a project stands on: the points a survey measured, the surface of triangles between them, and the
//! contour lines a plan draws of it.
//!
//! A survey measures points, not surfaces. What the points make is the triangulation that joins each of them to its
//! nearest neighbours and to no one across them — the Delaunay one, where no point of the survey lies inside the
//! circle through any triangle's three corners. That is what a surveyor's own drawing shows and the one
//! triangulation of a set of points that does not depend on the order they were measured in.

use crate::geom::P2;
use crate::view3d::P3;

/// How far apart in height the contour lines of a plan stand where the ground does not say, mm: half a metre, as a
/// surveyor's plan draws them.
pub const CONTOUR: f64 = 500.0;

/// The most points one ground holds. A survey of a site measures a few hundred; past this it is a point cloud, which
/// a project reads as one (`cloud.rs`) and does not triangulate.
pub const MOST_SPOTS: usize = 2000;

/// The most contour lines one ground is drawn with, however small its interval: a plan drawn with more is black.
const MOST_CONTOURS: usize = 400;

/// How close in plan two points are to be the same one, mm. Two measurements of one point are one point, and a
/// triangulation cannot hold both.
const SAME: f64 = 1.0;

/// The points of `spots` a surface is made of: the finite ones, no two in the same place in plan, in the order they
/// were given and at most [`MOST_SPOTS`] of them.
pub fn usable(spots: &[P3]) -> Vec<P3> {
    let mut out: Vec<P3> = Vec::new();
    for p in spots.iter().filter(|p| p.iter().all(|x| x.is_finite())) {
        if out.len() >= MOST_SPOTS {
            break;
        }
        if !out.iter().any(|q| (q[0] - p[0]).abs() <= SAME && (q[1] - p[1]).abs() <= SAME) {
            out.push(*p);
        }
    }
    out
}

/// Whether the points make a surface at all: three of them that are not in one line. Measured along the longest line
/// the points stretch out on, so it costs one pass over them and not a triangulation.
pub fn stands(spots: &[P3]) -> bool {
    let pts: Vec<P2> = usable(spots).iter().map(|p| P2(p[0], p[1])).collect();
    let Some(a) = pts.first().copied() else { return false };
    let Some(b) = pts.iter().copied().max_by(|p, q| p.dist(a).total_cmp(&q.dist(a))) else { return false };
    let far = b.dist(a);
    far > SAME && pts.iter().any(|p| ((*p - a).cross(b - a) / far).abs() > SAME)
}

/// The lowest and the highest point measured.
pub fn span(spots: &[P3]) -> Option<(f64, f64)> {
    let zs = usable(spots);
    let lo = zs.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
    let hi = zs.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
    (!zs.is_empty()).then_some((lo, hi))
}

/// The triangles of the surface, each counter-clockwise seen from above.
pub fn mesh(spots: &[P3]) -> Vec<[P3; 3]> {
    let pts = usable(spots);
    let plan: Vec<P2> = pts.iter().map(|p| P2(p[0], p[1])).collect();
    delaunay(&plan)
        .into_iter()
        .filter_map(|[a, b, c]| {
            let (a, b, c) = (*pts.get(a)?, *pts.get(b)?, *pts.get(c)?);
            let ccw = (P2(b[0], b[1]) - P2(a[0], a[1])).cross(P2(c[0], c[1]) - P2(a[0], a[1])) > 0.0;
            Some(if ccw { [a, b, c] } else { [a, c, b] })
        })
        .collect()
}

/// How big the surface is: the ground it covers seen from above, and the land itself, which is larger wherever it
/// slopes — square millimetres, both.
pub fn areas(spots: &[P3]) -> (f64, f64) {
    mesh(spots).iter().fold((0.0, 0.0), |(flat, real), t| {
        let [a, b, c] = *t;
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        (flat + n[2].abs() / 2.0, real + (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0)
    })
}

/// The height of the surface over a plan point, where the surface stands over it.
pub fn height(spots: &[P3], q: P2) -> Option<f64> {
    mesh(spots).into_iter().find_map(|t| over(&t, q))
}

/// The height of one triangle over a plan point, where it stands over it.
fn over(t: &[P3; 3], q: P2) -> Option<f64> {
    let [a, b, c] = t.map(|p| P2(p[0], p[1]));
    let total = (b - a).cross(c - a);
    if total.abs() < 1e-9 {
        return None;
    }
    let (wa, wb, wc) = ((b - q).cross(c - q) / total, (c - q).cross(a - q) / total, (a - q).cross(b - q) / total);
    let out = -1e-9;
    (wa >= out && wb >= out && wc >= out).then_some(wa * t[0][2] + wb * t[1][2] + wc * t[2][2])
}

/// The lines where the surface crosses each height that is a whole number of `interval` above nothing: each height
/// and the segments drawn at it, lowest first. `interval` of nothing is [`CONTOUR`].
pub fn contours(spots: &[P3], interval: f64) -> Vec<(f64, Vec<[P2; 2]>)> {
    let step = if interval.is_finite() && interval > 0.0 { interval } else { CONTOUR };
    let tris = mesh(spots);
    let Some((lo, hi)) = span(spots) else { return Vec::new() };
    // Strictly between the lowest point and the highest: a line drawn at the very height of either of them is the
    // edge of what was surveyed and not a line of the land, and a flat ground has no contour at all.
    let first = (lo / step).floor() + 1.0;
    let last = (hi / step).ceil() - 1.0;
    if !first.is_finite() || !last.is_finite() || last < first || last - first + 1.0 > MOST_CONTOURS as f64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for k in first as i64..=last as i64 {
        let z = k as f64 * step;
        let lines: Vec<[P2; 2]> = tris.iter().filter_map(|t| crosses(t, z)).collect();
        if !lines.is_empty() {
            out.push((z, lines));
        }
    }
    out
}

/// Where a height cuts one triangle: the segment across it, if it crosses two of its sides.
fn crosses(t: &[P3; 3], z: f64) -> Option<[P2; 2]> {
    let mut ends: Vec<P2> = Vec::new();
    for (p, q) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        let (zp, zq) = (p[2], q[2]);
        // The side is crossed where the height is between its ends: at its lower end and not its upper, so a corner
        // the height passes exactly through is counted by one of the two sides that meet there and not by both.
        if (zp <= z && z < zq) || (zq <= z && z < zp) {
            let k = (z - zp) / (zq - zp);
            ends.push(P2(p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k));
        }
    }
    match ends[..] {
        [a, b] if a.dist(b) > 1e-9 => Some([a, b]),
        _ => None,
    }
}

/// The Delaunay triangulation of plan points: triangles as the places of the points given, none of them holding any
/// other point of the set inside the circle through its three corners (Bowyer–Watson, one point at a time into a
/// triangle big enough to hold them all, which goes at the end).
fn delaunay(pts: &[P2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 || pts.iter().any(|p| !p.is_finite()) {
        return Vec::new();
    }
    let (lo, hi) = pts.iter().fold((P2(f64::MAX, f64::MAX), P2(f64::MIN, f64::MIN)), |(lo, hi), p| {
        (P2(lo.0.min(p.0), lo.1.min(p.1)), P2(hi.0.max(p.0), hi.1.max(p.1)))
    });
    let mid = (lo + hi) * 0.5;
    let wide = (hi.0 - lo.0).max(hi.1 - lo.1).max(1.0) * 20.0;
    // The three corners of that first triangle stand after the points given, so dropping the triangles that still
    // hold one of them is dropping every index past the points.
    let all: Vec<P2> =
        pts.iter().copied().chain([P2(mid.0 - wide, mid.1 - wide), P2(mid.0 + wide, mid.1 - wide), P2(mid.0, mid.1 + wide)]).collect();
    let at = |i: usize| all.get(i).copied().unwrap_or_default();
    let mut tris: Vec<[usize; 3]> = vec![[n, n + 1, n + 2]];
    let mut edges: Vec<([usize; 2], bool)> = Vec::new();
    for (i, p) in pts.iter().enumerate() {
        edges.clear();
        tris.retain(|t| {
            if !holds(at(t[0]), at(t[1]), at(t[2]), *p) {
                return true;
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let side = if a < b { [a, b] } else { [b, a] };
                match edges.iter_mut().find(|(e, _)| *e == side) {
                    // A side two of the triangles going share is inside the hole they leave and no edge of it.
                    Some((_, alone)) => *alone = false,
                    None => edges.push((side, true)),
                }
            }
            false
        });
        tris.extend(edges.iter().filter(|(_, alone)| *alone).map(|([a, b], _)| [*a, *b, i]));
    }
    tris.retain(|t| t.iter().all(|k| *k < n));
    tris
}

/// Whether `p` is inside the circle through `a`, `b` and `c`. A triangle with no area holds everything, so that a
/// sliver three points in a line make is taken out and the hole it leaves triangulated again.
fn holds(a: P2, b: P2, c: P2, p: P2) -> bool {
    let (u, v, w) = (a - p, b - p, c - p);
    let det = u.dot(u) * v.cross(w) - v.dot(v) * u.cross(w) + w.dot(w) * u.cross(v);
    match (b - a).cross(c - a) {
        turn if turn > 0.0 => det > 0.0,
        turn if turn < 0.0 => det < 0.0,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four corners of a square 10 m across, falling a metre from west to east.
    fn slope() -> Vec<P3> {
        vec![[0.0, 0.0, 1000.0], [10_000.0, 0.0, 0.0], [10_000.0, 10_000.0, 0.0], [0.0, 10_000.0, 1000.0]]
    }

    #[test]
    fn the_surface_of_four_points_is_two_triangles_that_cover_the_square_between_them() {
        let tris = mesh(&slope());
        assert_eq!(tris.len(), 2, "a square is two triangles");
        let area: f64 = tris
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|p| P2(p[0], p[1]));
                (b - a).cross(c - a) / 2.0
            })
            .sum();
        assert!((area - 100e6).abs() < 1.0, "the two cover the square and no more: {area}");
        assert!(area > 0.0, "counter-clockwise seen from above");
        // The surface stands where its points say, and between them it slopes evenly.
        assert_eq!(height(&slope(), P2(0.0, 5000.0)), Some(1000.0));
        assert_eq!(height(&slope(), P2(5000.0, 5000.0)), Some(500.0));
        assert_eq!(height(&slope(), P2(2500.0, 2500.0)), Some(750.0));
        assert_eq!(height(&slope(), P2(-1.0, 5000.0)), None, "off it there is no ground");
        assert_eq!(span(&slope()), Some((0.0, 1000.0)));
    }

    #[test]
    fn a_point_inside_the_others_is_joined_to_each_of_them_and_the_surface_bends_at_it() {
        let mut spots = slope();
        spots.push([5000.0, 5000.0, 2000.0]);
        let tris = mesh(&spots);
        assert_eq!(tris.len(), 4, "four triangles round the one in the middle");
        assert_eq!(height(&spots, P2(5000.0, 5000.0)), Some(2000.0), "the surface goes through the point measured");
        // Halfway from the middle to the corner it is halfway between the two heights, which the flat square was not.
        assert_eq!(height(&spots, P2(2500.0, 2500.0)), Some(1500.0));
    }

    #[test]
    fn points_all_in_one_line_are_no_surface_and_two_measurements_of_one_point_are_one() {
        let line = vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 100.0], [2000.0, 0.0, 200.0], [3000.0, 0.0, 300.0]];
        assert!(mesh(&line).is_empty(), "no three of them are a triangle");
        assert!(!stands(&line));
        assert!(!stands(&line[..2]));
        // The same point twice is one point: the second is left out, so three of them are a triangle and not a sliver.
        let twice = vec![[0.0, 0.0, 0.0], [0.0, 0.5, 500.0], [5000.0, 0.0, 0.0], [0.0, 5000.0, 0.0]];
        assert_eq!(usable(&twice).len(), 3);
        assert_eq!(mesh(&twice).len(), 1);
        // Nothing of a hostile list reaches the surface: what is not a number is left out.
        assert!(mesh(&[[f64::NAN, 0.0, 0.0], [0.0, f64::INFINITY, 0.0], [0.0, 0.0, 0.0]]).is_empty());
        assert_eq!(span(&[]), None);
    }

    #[test]
    fn the_contours_stand_at_the_heights_they_are_apart_and_cross_the_fall_of_the_land() {
        // The square falls a metre from west to east: contours every 250 mm stand at 250, 500 and 750, each of them
        // a line the full 10 m across the square — the ends, 0 and 1000, are the corners themselves and no line.
        let lines = contours(&slope(), 250.0);
        assert_eq!(lines.iter().map(|(z, _)| *z).collect::<Vec<_>>(), vec![250.0, 500.0, 750.0]);
        for (z, segs) in &lines {
            let total: f64 = segs.iter().map(|[a, b]| a.dist(*b)).sum();
            assert!((total - 10_000.0).abs() < 1.0, "the contour at {z} runs the width of the square: {total}");
            // It runs north–south, across the fall, at the x the height puts it: a metre of fall over ten.
            let x = 10_000.0 - z * 10.0;
            assert!(segs.iter().flatten().all(|p| (p.0 - x).abs() < 1.0), "the contour at {z} stands at {x}");
        }
        // Half a metre apart where nothing says, and nothing at all where the interval would draw a black plan.
        assert_eq!(contours(&slope(), 0.0).iter().map(|(z, _)| *z).collect::<Vec<_>>(), vec![500.0]);
        assert!(contours(&slope(), 1e-6).is_empty());
        assert!(contours(&slope(), f64::NAN).len() == 1, "no number is no interval, so the usual one is used");
    }
}

//! CAD drawings under a plan, as Revit's Link CAD: a survey or a consultant's drawing shown under the model, to be
//! traced over and snapped to, never edited. The file is read by [`crate::dxf`] and held in `Model::drawings`, keyed by
//! its path; the project's file holds the path alone, as a linked model's does, so a drawing of any size costs a
//! project nothing.
//!
//! What a view draws, and what a point snaps to, is [`lines`]: the drawing's shapes where the underlay stands, turned
//! by its angle, in this project's millimetres. An arc becomes the line through the points along it, as a plan draws
//! every curve.

use crate::geom::P2;
use crate::model::{Model, Underlay};

/// How far apart, in degrees, the points along an arc are: a circle becomes 72 lines, which is round at any scale a
/// plan is read at.
const ARC_STEP: f64 = 5.0;
/// The most points one arc becomes, whatever its angle says.
const ARC_POINTS: usize = 721;

/// The lines the underlay draws, in this project's millimetres: each a run of points and whether it closes. An underlay
/// whose drawing is not loaded draws nothing, as a link that is not loaded shows nothing.
pub fn lines(m: &Model, u: &Underlay) -> Vec<(Vec<P2>, bool)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    // The drawing was read in its own units; `scale` is what it was read with, so its points are millimetres already.
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            match shape {
                crate::dxf::Shape::Line(line) => out.push((line.iter().map(|p| place(*p)).collect(), false)),
                crate::dxf::Shape::Path { points, closed } => out.push((points.iter().map(|p| place(*p)).collect(), *closed)),
                crate::dxf::Shape::Arc { at, radius, from, to } => {
                    out.push((along(*at, *radius, *from, *to).into_iter().map(place).collect(), false));
                }
            }
        }
    }
    out
}

/// The points along an arc of `radius` about `at`, from `from` to `to`, degrees counter-clockwise from east. An arc
/// whose end is before its start runs the long way round, as a drawing reads one.
fn along(at: P2, radius: f64, from: f64, to: f64) -> Vec<P2> {
    let sweep = {
        let turn = to - from;
        if turn <= 0.0 { turn + 360.0 } else { turn }
    };
    let steps = ((sweep / ARC_STEP).ceil() as usize).clamp(1, ARC_POINTS - 1);
    (0..=steps)
        .map(|i| {
            let a = (from + sweep * i as f64 / steps as f64).to_radians();
            P2(at.0 + radius * a.cos(), at.1 + radius * a.sin())
        })
        .collect()
}

/// The box an underlay's lines lie in, mm: what a view needs to fit one. None where it draws nothing.
pub fn extent(m: &Model, u: &Underlay) -> Option<[P2; 2]> {
    let mut found: Option<[P2; 2]> = None;
    for p in lines(m, u).iter().flat_map(|(points, _)| points.clone()) {
        found = Some(match found {
            None => [p, p],
            Some([lo, hi]) => [P2(lo.0.min(p.0), lo.1.min(p.1)), P2(hi.0.max(p.0), hi.1.max(p.1))],
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::{Drawing, Layer, Shape};
    use std::sync::Arc;

    /// A model with one underlay, of one shape, where `at` and turned `angle`.
    fn with(shape: Shape, at: P2, angle: f64) -> (Model, Underlay) {
        let mut m = Model::new("Test");
        let drawing = Drawing { layers: vec![Layer { name: "MURI".into(), shapes: vec![shape] }], scale: 1.0, ..Drawing::default() };
        m.drawings.insert("flat.dxf".into(), Arc::new(drawing));
        let u = Underlay { name: "Survey".into(), path: "flat.dxf".into(), at, angle, scale: 1.0, ..Underlay::default() };
        (m, u)
    }

    #[test]
    fn a_drawing_stands_where_the_underlay_does_and_turns_with_it() {
        let line = Shape::Line([P2(0.0, 0.0), P2(1000.0, 0.0)]);
        let (m, u) = with(line.clone(), P2(500.0, 200.0), 0.0);
        assert_eq!(lines(&m, &u), vec![(vec![P2(500.0, 200.0), P2(1500.0, 200.0)], false)]);
        // A quarter turn takes the line north; the underlay's own point stays where it is.
        let (m, u) = with(line, P2(0.0, 0.0), 90.0);
        let [(points, _)] = &lines(&m, &u)[..] else { panic!() };
        assert_eq!(points[0], P2(0.0, 0.0));
        assert!((points[1].0).abs() < 1e-9 && (points[1].1 - 1000.0).abs() < 1e-9, "{:?}", points[1]);
    }

    #[test]
    fn an_arc_becomes_the_line_through_the_points_along_it() {
        let (m, u) = with(Shape::Arc { at: P2(0.0, 0.0), radius: 1000.0, from: 0.0, to: 90.0 }, P2(0.0, 0.0), 0.0);
        let [(points, closed)] = &lines(&m, &u)[..] else { panic!() };
        assert!(!closed && points.len() == 19, "{} points for a quarter at 5°", points.len());
        assert!((points[0].0 - 1000.0).abs() < 1e-9 && (points[18].1 - 1000.0).abs() < 1e-9, "{:?}", points);
        // Every point is on the circle.
        assert!(points.iter().all(|p| (p.0.hypot(p.1) - 1000.0).abs() < 1e-9));
    }

    #[test]
    fn a_hidden_layer_draws_nothing_and_a_drawing_that_is_not_loaded_draws_nothing_either() {
        let (m, mut u) = with(Shape::Line([P2(0.0, 0.0), P2(1.0, 0.0)]), P2(0.0, 0.0), 0.0);
        u.hidden = vec!["MURI".into()];
        assert!(lines(&m, &u).is_empty());
        u.hidden.clear();
        u.path = "elsewhere.dxf".into();
        assert!(lines(&m, &u).is_empty(), "a file that was never read draws nothing");
    }

    #[test]
    fn a_circle_closes_on_itself() {
        let (m, u) = with(Shape::Arc { at: P2(0.0, 0.0), radius: 500.0, from: 0.0, to: 360.0 }, P2(0.0, 0.0), 0.0);
        let [(points, _)] = &lines(&m, &u)[..] else { panic!() };
        assert_eq!(points.len(), 73, "a circle at 5°");
        assert!((points[0].0 - points[72].0).abs() < 1e-9 && (points[0].1 - points[72].1).abs() < 1e-9);
        assert_eq!(extent(&m, &u).map(|[lo, hi]| (lo.0.round(), hi.0.round())), Some((-500.0, 500.0)));
    }
}

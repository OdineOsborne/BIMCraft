// Ported in part from rvt-rs (https://github.com/DrunkOnJava/rvt-rs), Apache-2.0,
// Copyright 2026 Griffin Long, at commit b6086d41b9758fd9b92f42f0a47b8345b9c58056: src/partition_beam_axes.rs
// (`beam_body`: the section a record box leaves once its location line's own extent is taken out of it, and its
// tolerances). Changed: rewritten for BIMCraft, returning the reason a solve fails so the import can say it, and
// giving the section in millimetres for the model.
//! Beams (structural framing): a section along the location line the element's data carries.
//!
//! A framing element's data holds its **location line** as a bounded line ([`crate::curves`]), and its record holds its
//! **box**. The line gives the beam's plan line and the height of each of its two ends; the section is what the box leaves
//! once the line's own extent is taken out of it: the depth from the vertical extent, the width from one plan extent, and
//! the other plan extent is a check the solve must pass.
//!
//! Its **level** is read as a wall's and a column's are, the higher level its list names at or below the box, and its
//! `offset` and `end_offset` are each end of the line over that level. The line is the top of the section, which is what
//! Revit's framing places and what a BIMCraft beam means by [`Justify::Top`].
//!
//! The section's *shape* is not read: the data read here does not say whether a beam is rectangular, an I, a channel or
//! round, so every beam comes in as the rectangle of its solve and the notes say so. What is left out, with its reason:
//! an element whose data holds no bounded line, one whose line leaves its box, one too steep to have a horizontal width
//! axis (a brace), and one whose third box extent disagrees with the section the other two give (an oblique beam whose
//! end cuts are not square to its line).

use bimcraft_core::model::{Element, Justify, Run, RunType};

use crate::curves;
use crate::data::newest;
use crate::import::{Facts, Import, kind, micron, mm, point};
use crate::inventory::{Gathered, Wants};
use crate::openings::symbol_name;
use crate::walls::constraints;

/// `OST_StructuralFraming`.
pub(crate) const OST_STRUCTURAL_FRAMING: i64 = -2_001_320;

/// How far an end of the line may lie outside the record box, feet.
const END_TOLERANCE_FEET: f64 = 1e-6;
/// How far the third extent of the box may disagree with the section the other two give, feet.
const CLOSURE_TOLERANCE_FEET: f64 = 0.01;
/// The least share of the line that must lie in plan: a steeper one has no horizontal width axis.
const MIN_PLAN_SHARE: f64 = 0.1;
/// Two sections within this many millimetres of each other are the same type.
const SAME_SECTION_MM: f64 = 0.05;

pub(crate) fn wanted(f: &Facts, out: &mut Wants) {
    for r in f.of_category(OST_STRUCTURAL_FRAMING) {
        out.data.insert(r.id);
    }
}

/// The width and depth, in millimetres, that the record box `bbox` leaves once the extent of the line from `start` to
/// `end` is taken out of it, or why it leaves none.
fn section(bbox: [f64; 6], start: [f64; 3], end: [f64; 3]) -> Result<(f64, f64), String> {
    for axis in 0..3 {
        let (low, high) = (bbox[axis], bbox[axis + 3]);
        if low > high {
            return Err("its record box is empty".into());
        }
        for point in [start, end] {
            if point[axis] < low - END_TOLERANCE_FEET || point[axis] > high + END_TOLERANCE_FEET {
                return Err("its location line leaves its record box".into());
            }
        }
    }
    let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if length <= 0.0 {
        return Err("its location line has no length".into());
    }
    let d = [delta[0] / length, delta[1] / length, delta[2] / length];
    let plan = d[0].hypot(d[1]);
    if plan < MIN_PLAN_SHARE {
        return Err("it is too steep for a horizontal width axis".into());
    }
    // Width axis: horizontal, across the line. Depth axis: in the line's vertical plane, across the line.
    let u = [-d[1] / plan, d[0] / plan, 0.0];
    let w = [-d[2] * d[0] / plan, -d[2] * d[1] / plan, plan];
    let extent = [bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]];
    // Each extent is length·|d| + width·|u| + depth·|w| on its axis: the depth from the vertical one, the width from the
    // plan axis the width axis leans on most, and the remaining plan axis is the check.
    let depth = (extent[2] - length * d[2].abs()) / w[2];
    let (solved, check) = if u[0].abs() >= u[1].abs() { (0, 1) } else { (1, 0) };
    let width = (extent[solved] - length * d[solved].abs() - depth * w[solved].abs()) / u[solved].abs();
    if width <= 0.0 || depth <= 0.0 {
        return Err("its record box leaves it no section".into());
    }
    let closure = length * d[check].abs() + width * u[check].abs() + depth * w[check].abs();
    if (closure - extent[check]).abs() > CLOSURE_TOLERANCE_FEET {
        return Err("its record box does not fit a straight section along its line: its ends are cut off square".into());
    }
    Ok((mm(width), mm(depth)))
}

pub(crate) fn import(cx: &mut Import, f: &Facts, g: &Gathered) {
    let symbols = f.symbols.get(&OST_STRUCTURAL_FRAMING).cloned().unwrap_or_default();
    for r in f.of_category(OST_STRUCTURAL_FRAMING) {
        let made = (|| -> Result<Run, String> {
            let line = g
                .data
                .get(&r.id)
                .and_then(|copies| newest(copies, curves::first_line))
                .map(|(line, _)| line)
                .ok_or("its data holds no bounded line, so its location line is not read")?;
            let (start, end) = (line.start(), line.end());
            let (width, depth) = section(r.bbox, start, end)?;
            let (base, _) = constraints(r, &f.elevations);
            let level = base.and_then(|l| cx.levels.get(&l).copied()).ok_or("it belongs to no level")?;
            let elevation = base.and_then(|l| f.elevations.get(&l).copied()).unwrap_or(0.0);
            let over = |z: f64| micron(mm(z) - mm(elevation));
            let (offset, at_end) = (over(start[2]), over(end[2]));
            let name = symbol_name(r, &symbols, g, OST_STRUCTURAL_FRAMING).unwrap_or_else(|| "Imported beam".to_string());
            let type_name = kind(
                cx.model.types.runs.entry("beam".to_string()).or_default(),
                &name,
                |t: &RunType| (t.width - width).abs() < SAME_SECTION_MM && (t.depth - depth).abs() < SAME_SECTION_MM && !t.round,
                RunType { width, depth, round: false, material: String::new() },
            );
            Ok(Run {
                type_name,
                level,
                start: point(start[0], start[1]),
                end: point(end[0], end[1]),
                offset,
                end_offset: (at_end != offset).then_some(at_end),
                justify: Justify::Top,
                system: String::new(),
                params: Default::default(),
            })
        })();
        match made {
            Ok(run) => {
                let id = cx.model.insert(Element::Beam(run));
                cx.from.insert(id, format!("Beam {}", r.id));
                cx.note("beam", r.id, "comes in as the rectangle of its solve: the shape of its section is not read");
            }
            Err(why) => cx.left_out("beam", r.id, &why),
        }
    }
}

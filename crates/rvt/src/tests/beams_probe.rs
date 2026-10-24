//! A MEASUREMENT, not a port: does the bounded-line record exist in the data of the
//! structural-framing elements of a real file, and does the reconciliation with the
//! record box close on it?
//!
//! No format finding of its own: it reads records this crate already reads (`curves`,
//! `records`) and re-derives rvt-rs's reconciliation of the line with the box only to
//! count how often it closes. A port of the solve would need its row in
//! `docs/rvt-provenance.md` first.

use super::all_samples;
use crate::curves;
use crate::data::newest;
use crate::inventory::Wants;
use crate::{Limits, Revit};
use std::collections::BTreeMap;

const OST_STRUCTURAL_FRAMING: i64 = -2_001_320;
/// What this measures: the three structural categories the reader counts but has had no module for.
const MEASURED: [(i64, &str); 3] =
    [(OST_STRUCTURAL_FRAMING, "structural framing"), (-2_001_300, "structural foundations"), (-2_009_000, "structural rebar")];
const END_TOLERANCE_FEET: f64 = 1e-6;
const CLOSURE_TOLERANCE_FEET: f64 = 0.01;
const MIN_PLAN_SHARE: f64 = 0.1;

/// Why a line and a box do not give a beam.
#[derive(Debug, PartialEq)]
enum Fail {
    Outside,
    Steep,
    Closure,
}

/// The length, width and depth the box leaves once the line's own extent is taken out,
/// or why it does not close.
fn solve(bbox: [f64; 6], start: [f64; 3], end: [f64; 3]) -> Result<(f64, f64, f64), Fail> {
    for axis in 0..3 {
        let (low, high) = (bbox[axis], bbox[axis + 3]);
        if low > high {
            return Err(Fail::Outside);
        }
        for point in [start, end] {
            if point[axis] < low - END_TOLERANCE_FEET || point[axis] > high + END_TOLERANCE_FEET {
                return Err(Fail::Outside);
            }
        }
    }
    let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if length <= 0.0 {
        return Err(Fail::Outside);
    }
    let d = [delta[0] / length, delta[1] / length, delta[2] / length];
    let plan = d[0].hypot(d[1]);
    if plan < MIN_PLAN_SHARE {
        return Err(Fail::Steep);
    }
    let u = [-d[1] / plan, d[0] / plan, 0.0];
    let w = [-d[2] * d[0] / plan, -d[2] * d[1] / plan, plan];
    let extent = [bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]];
    let depth = (extent[2] - length * d[2].abs()) / w[2];
    let (solved_axis, check_axis) = if u[0].abs() >= u[1].abs() { (0, 1) } else { (1, 0) };
    let width = (extent[solved_axis] - length * d[solved_axis].abs() - depth * w[solved_axis].abs()) / u[solved_axis].abs();
    let closure = length * d[check_axis].abs() + width * u[check_axis].abs() + depth * w[check_axis].abs();
    if !(width > 0.0 && depth > 0.0) || (closure - extent[check_axis]).abs() > CLOSURE_TOLERANCE_FEET {
        return Err(Fail::Closure);
    }
    Ok((length, width, depth))
}

#[test]
fn how_many_structural_framing_elements_carry_a_bounded_line() {
    let samples = all_samples();
    if samples.is_empty() {
        eprintln!("no samples: set BIMCRAFT_RVT_SAMPLES");
        return;
    }
    for path in samples {
        for (category, label) in MEASURED {
            let mut revit = match Revit::open(&path, Limits::default()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{}: cannot open: {e}", path.display());
                    continue;
                }
            };
            let release = match revit.basic_file_info() {
                Ok(i) => i.release,
                Err(e) => {
                    eprintln!("{}: no release: {e}", path.display());
                    continue;
                }
            };
            let survey = match revit.survey(&|c| c == category) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{} (Revit {release}): survey failed: {e}", path.display());
                    continue;
                }
            };
            // The newest copy of each placed framing record, with its box.
            let mut current: BTreeMap<u32, (u32, [f64; 6])> = BTreeMap::new();
            for r in survey.records.iter().filter(|r| r.placed) {
                match current.get(&r.id) {
                    Some((held, _)) if *held >= r.partition => {}
                    _ => {
                        current.insert(r.id, (r.partition, r.bbox));
                    }
                }
            }
            if current.is_empty() {
                eprintln!("{} (Revit {release}): no placed {label}", path.display());
                continue;
            }
            let mut wants = Wants::default();
            wants.data.extend(current.keys().copied());
            let gathered = match revit.gather(survey.release, &survey.tags, &survey.ids, &wants) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("{} (Revit {release}): gather failed: {e}", path.display());
                    continue;
                }
            };
            let (mut no_data, mut no_line, mut lines) = (0usize, 0usize, 0usize);
            let (mut ok, mut outside, mut steep, mut no_closure) = (0usize, 0usize, 0usize, 0usize);
            let mut sections: BTreeMap<(i64, i64), usize> = BTreeMap::new();
            for (id, (_, bbox)) in &current {
                let Some(copies) = gathered.data.get(id) else {
                    no_data += 1;
                    continue;
                };
                let Some((line, _)) = newest(copies, curves::first_line) else {
                    no_line += 1;
                    continue;
                };
                lines += 1;
                match solve(*bbox, line.start(), line.end()) {
                    Ok((_, width, depth)) => {
                        ok += 1;
                        // millimetres, rounded, so equal sections group
                        let mm = |feet: f64| (feet * 304.8).round() as i64;
                        *sections.entry((mm(width), mm(depth))).or_default() += 1;
                    }
                    Err(Fail::Outside) => outside += 1,
                    Err(Fail::Steep) => steep += 1,
                    Err(Fail::Closure) => no_closure += 1,
                }
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            eprintln!("\n=== {name} (Revit {release}) : {label} ===");
            eprintln!("  placed:              {}", current.len());
            eprintln!("  with a bounded line:       {lines}   (no data {no_data}, no line {no_line})");
            eprintln!("  the solve closes:          {ok}");
            eprintln!("  it does not:               outside {outside}, too steep {steep}, no closure {no_closure}");
            let mut by_count: Vec<_> = sections.iter().collect();
            by_count.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            for ((w, d), n) in by_count.iter().take(8) {
                eprintln!("     {n:>4} × {w} mm wide, {d} mm deep");
            }
        }
    }
}

//! Isolated footings: a pad under a column, read as a column laid flat.
//!
//! A footing's **place** and **size** in plan are its record box's, as a column's are, and its **thickness** is how deep
//! that box is; its `base_offset` is the bottom of the box over its level. A footing turned off the axes by some other
//! angle than a right angle has a box bigger than its pad, so its pad is its family symbol's envelope turned by the angle
//! of its placement, and where that envelope is not known it is left out rather than come in too big. Its **level** is the
//! one its reference list names, as a wall's and a column's are, and it has no top constraint: a footing hangs from its
//! level, it does not reach another.
//!
//! The pad's *shape* is not read, as a column's is not: the data read here does not say whether a footing is rectangular,
//! round or stepped, so every footing comes in as the rectangle of its box and the notes say so. Continuous (strip)
//! footings are a run along a line, not a pad, and are not read here.

use bimcraft_core::model::{Column, ColumnType, Element};

use crate::data::newest;
use crate::import::{Facts, Import, kind, micron, mm, point};
use crate::inventory::{Gathered, Wants};
use crate::openings::{symbol_name, symbol_of};
use crate::placement::transform;
use crate::walls::constraints;

/// `OST_StructuralFoundation`.
pub(crate) const OST_STRUCTURAL_FOUNDATION: i64 = -2_001_300;

pub(crate) fn wanted(f: &Facts, out: &mut Wants) {
    for r in f.of_category(OST_STRUCTURAL_FOUNDATION) {
        out.data.insert(r.id);
    }
}

pub(crate) fn import(cx: &mut Import, f: &Facts, g: &Gathered) {
    let symbols = f.symbols.get(&OST_STRUCTURAL_FOUNDATION).cloned().unwrap_or_default();
    for r in f.of_category(OST_STRUCTURAL_FOUNDATION) {
        let made = (|| -> Result<Column, String> {
            let (base, _) = constraints(r, &f.elevations);
            let level = base.and_then(|l| cx.levels.get(&l).copied()).ok_or("it hangs from no level")?;
            let elevation = base.and_then(|l| f.elevations.get(&l).copied()).unwrap_or(0.0);
            // Turned by an angle that is no right angle: the symbol's envelope, which is the pad before it is turned.
            let placed = g.data.get(&r.id).and_then(|c| newest(c, transform)).map(|(t, _)| t);
            let angle = placed.and_then(|t| t.plan_angle()).map(f64::to_degrees).unwrap_or(0.0);
            let quarter = (angle / 90.0 - (angle / 90.0).round()).abs() < 1e-6;
            let (width, depth, at, angle) = match placed.filter(|_| !quarter) {
                Some(t) => {
                    let sym = symbol_of(r, &symbols, g, OST_STRUCTURAL_FOUNDATION)
                        .and_then(|(id, _)| f.symbol_boxes.get(&id))
                        .ok_or("it is turned and its family symbol's size is not known")?;
                    (mm(sym[3] - sym[0]), mm(sym[4] - sym[1]), point(t.origin[0], t.origin[1]), micron(angle))
                }
                None => (
                    mm(r.bbox[3] - r.bbox[0]),
                    mm(r.bbox[4] - r.bbox[1]),
                    point((r.bbox[0] + r.bbox[3]) / 2.0, (r.bbox[1] + r.bbox[4]) / 2.0),
                    0.0,
                ),
            };
            let name = symbol_name(r, &symbols, g, OST_STRUCTURAL_FOUNDATION).unwrap_or_else(|| "Imported footing".to_string());
            let type_name = kind(
                &mut cx.model.types.footing,
                &name,
                |t| (t.width - width).abs() < 0.05 && (t.depth - depth).abs() < 0.05 && !t.round,
                ColumnType { width, depth, round: false },
            );
            Ok(Column {
                type_name,
                level,
                at,
                angle,
                top_level: None,
                height: mm(r.bbox[5] - r.bbox[2]),
                base_offset: micron(mm(r.bbox[2]) - mm(elevation)),
                params: Default::default(),
            })
        })();
        match made {
            Ok(footing) => {
                let id = cx.model.insert(Element::Footing(footing));
                cx.from.insert(id, format!("Footing {}", r.id));
                cx.note("footing", r.id, "comes in as the rectangle of its box: the shape of its pad is not read");
            }
            Err(why) => cx.left_out("footing", r.id, &why),
        }
    }
}

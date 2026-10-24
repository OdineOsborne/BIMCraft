//! Beams: their line, level, offsets, section, and what is not read.

use super::walls::{base, import, level_named};
use crate::project_fixture::{Block, NameEntry, Project, Rec, line_bytes};
use bimcraft_core::import::Imported;
use bimcraft_core::model::{Element, Justify, Run};

const FRAMING: i64 = -2_001_320;

fn beams(i: &Imported) -> Vec<&Run> {
    i.model.elements.values().filter_map(|e| if let Element::Beam(r) = e { Some(r) } else { None }).collect()
}

/// Levels 0 and 10 ft, and a framing symbol 660 named "W-Beam".
fn project(release: u32) -> Project {
    let mut p = base(release);
    p.recs.push(Rec { placed: false, ..Rec::new(660, FRAMING) });
    p.names.push(NameEntry { id: 660, name: "W-Beam".into(), category: FRAMING });
    p
}

/// A framing element whose record box is `bbox` and whose data holds the bounded line.
fn beam(p: &mut Project, id: u32, bbox: [f64; 6], line: (f64, f64, [f64; 3], [f64; 3])) {
    p.recs.push(Rec::new(id, FRAMING).at(bbox, &[10, 11, 660, u64::from(id)]));
    let (start, end, origin, direction) = line;
    p.data.push(Block { id, payload: line_bytes(start, end, origin, direction) });
}

#[test]
fn a_level_beam_comes_in_on_its_line_with_its_section_hanging_from_it() {
    for release in [2019, 2024] {
        let mut p = project(release);
        // 10 ft east from the origin, 1 ft wide and 2 ft deep, its line on top of the section.
        beam(&mut p, 100, [0.0, -0.5, -2.0, 10.0, 0.5, 0.0], (0.0, 10.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        let i = import(&p);
        let [b] = beams(&i)[..] else { panic!("{:?}", i.skipped) };
        assert_eq!((b.start.0, b.start.1, b.end.0, b.end.1), (0.0, 0.0, 3048.0, 0.0));
        assert_eq!((b.level, b.offset, b.end_offset), (level_named(&i, "Level 1"), 0.0, None));
        assert_eq!(b.justify, Justify::Top);
        let t = &i.model.types.runs["beam"][&b.type_name];
        assert_eq!((t.width, t.depth, t.round), (304.8, 609.6, false));
        assert!(b.type_name.contains("W-Beam"), "{}", b.type_name);
    }
}

#[test]
fn a_sloped_beam_keeps_the_height_of_each_end() {
    let mut p = project(2024);
    // 8 ft east and 6 ft up: the 3-4-5 slope, 1 ft wide and 2 ft deep.
    beam(&mut p, 100, [-0.6, -0.5, -1.6, 8.6, 0.5, 6.0], (0.0, 10.0, [0.0, 0.0, 0.0], [0.8, 0.0, 0.6]));
    let i = import(&p);
    let [b] = beams(&i)[..] else { panic!("{:?}", i.skipped) };
    assert_eq!((b.start.0, b.end.0), (0.0, 2438.4));
    assert_eq!((b.offset, b.end_offset), (0.0, Some(1828.8)));
}

#[test]
fn a_beam_above_its_level_keeps_its_offset() {
    let mut p = project(2024);
    // The same beam, its line 3 ft over level 1.
    beam(&mut p, 100, [0.0, -0.5, 1.0, 10.0, 0.5, 3.0], (0.0, 10.0, [0.0, 0.0, 3.0], [1.0, 0.0, 0.0]));
    let i = import(&p);
    let [b] = beams(&i)[..] else { panic!("{:?}", i.skipped) };
    assert_eq!((b.offset, b.end_offset), (914.4, None));
}

#[test]
fn a_framing_element_too_steep_for_a_section_is_left_out_with_the_reason() {
    let mut p = project(2024);
    beam(&mut p, 100, [-0.5, -0.5, 0.0, 0.5, 0.5, 10.0], (0.0, 10.0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    let i = import(&p);
    assert!(beams(&i).is_empty(), "{:?}", beams(&i));
    assert!(i.skipped.iter().any(|s| s.contains("1 beam") && s.contains("steep") && s.contains("100")), "{:?}", i.skipped);
}

#[test]
fn a_framing_element_whose_box_does_not_fit_its_line_is_left_out() {
    let mut p = project(2024);
    // A line that leaves its box: the box stops at 5 ft, the line runs to 10.
    beam(&mut p, 100, [0.0, -0.5, -2.0, 5.0, 0.5, 0.0], (0.0, 10.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    let i = import(&p);
    assert!(beams(&i).is_empty());
    assert!(i.skipped.iter().any(|s| s.contains("1 beam") && s.contains("100")), "{:?}", i.skipped);
}

#[test]
fn a_framing_element_with_no_bounded_line_is_left_out() {
    let mut p = project(2024);
    p.recs.push(Rec::new(100, FRAMING).at([0.0, -0.5, -2.0, 10.0, 0.5, 0.0], &[10, 11, 660, 100]));
    let i = import(&p);
    assert!(beams(&i).is_empty());
    assert!(i.skipped.iter().any(|s| s.contains("1 beam") && s.contains("line") && s.contains("100")), "{:?}", i.skipped);
}

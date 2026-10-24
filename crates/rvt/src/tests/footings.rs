//! Isolated footings: their place, size, thickness, level, type, and what is not read.

use super::walls::{base, import, level_named};
use crate::project_fixture::{NameEntry, Project, Rec};
use bimcraft_core::import::Imported;
use bimcraft_core::model::{Column, Element};

const FOUNDATIONS: i64 = -2_001_300;

fn footings(i: &Imported) -> Vec<&Column> {
    i.model.elements.values().filter_map(|e| if let Element::Footing(c) = e { Some(c) } else { None }).collect()
}

/// Levels 0 and 10 ft, and a foundation symbol 670 named "Pad".
fn project(release: u32) -> Project {
    let mut p = base(release);
    p.recs.push(Rec { placed: false, ..Rec::new(670, FOUNDATIONS) });
    p.names.push(NameEntry { id: 670, name: "Pad".into(), category: FOUNDATIONS });
    p
}

#[test]
fn a_footing_comes_in_at_the_middle_of_its_box_as_thick_as_the_box_is_deep() {
    for release in [2019, 2024] {
        let mut p = project(release);
        // 4 ft by 4 ft in plan at (10, 5), 1.5 ft thick, its top at level 1.
        p.recs.push(Rec::new(100, FOUNDATIONS).at([8.0, 3.0, -1.5, 12.0, 7.0, 0.0], &[10, 670, 100]));
        let i = import(&p);
        let [f] = footings(&i)[..] else { panic!("{:?}", i.skipped) };
        assert_eq!((f.at.0, f.at.1, f.angle), (3048.0, 1524.0, 0.0));
        assert_eq!((f.level, f.top_level), (level_named(&i, "Level 1"), None));
        assert_eq!((f.height, f.base_offset), (457.2, -457.2));
        let t = &i.model.types.footing[&f.type_name];
        assert_eq!((t.width, t.depth, t.round), (1219.2, 1219.2, false));
        assert!(f.type_name.contains("Pad"), "{}", f.type_name);
    }
}

#[test]
fn a_footing_under_a_level_keeps_its_offset() {
    let mut p = project(2024);
    // Its top 2 ft below level 1, 1 ft thick.
    p.recs.push(Rec::new(100, FOUNDATIONS).at([8.0, 3.0, -3.0, 12.0, 7.0, -2.0], &[10, 670, 100]));
    let i = import(&p);
    let [f] = footings(&i)[..] else { panic!("{:?}", i.skipped) };
    assert_eq!((f.height, f.base_offset), (304.8, -914.4));
}

#[test]
fn a_footing_whose_list_names_no_level_takes_the_lowest_one() {
    // `level_at_or_below` gives the lowest level when none is at or below the box, which a footing under the
    // ground always is: it hangs from level 1, not from nothing.
    let mut p = project(2024);
    p.recs.push(Rec::new(100, FOUNDATIONS).at([8.0, 3.0, -1.5, 12.0, 7.0, 0.0], &[670, 100]));
    let i = import(&p);
    let [f] = footings(&i)[..] else { panic!("{:?}", i.skipped) };
    assert_eq!((f.level, f.base_offset), (level_named(&i, "Level 1"), -457.2));
}

#[test]
fn every_footing_says_its_shape_is_not_read() {
    let mut p = project(2024);
    p.recs.push(Rec::new(100, FOUNDATIONS).at([8.0, 3.0, -1.5, 12.0, 7.0, 0.0], &[10, 670, 100]));
    let i = import(&p);
    assert_eq!(footings(&i).len(), 1);
    assert!(
        i.skipped.iter().any(|s| s.contains("1 footing") && s.contains("rectangle of its box") && s.contains("100")),
        "{:?}",
        i.skipped
    );
}

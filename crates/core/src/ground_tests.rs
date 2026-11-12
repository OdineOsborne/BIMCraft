//! The ground a project stands on, driven through its command: the surface the points of a survey make, what a plan
//! and the 3D view draw of it, what IFC writes, and what it refuses.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::geom::P2;
use crate::model::Element;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A path of this test run's own in the temporary folder.
fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bimcraft-ground-{}-{name}", std::process::id()))
}

/// A square of land 10 m across, falling a metre from west to east, as four measured points.
fn slope() -> Value {
    json!([[0, 0, 1000], [10_000, 0, 0], [10_000, 10_000, 0], [0, 10_000, 1000]])
}

/// How many lines a plan of the lowest level draws.
fn drawn(s: &Session) -> usize {
    let level = s.model.levels().first().map(|(id, _)| *id).expect("a level");
    crate::render::plan(&s.model, level, 800, 600).prims.len()
}

#[test]
fn the_ground_is_the_surface_between_the_points_a_survey_measured() {
    let mut s = Session::new();
    let bare = drawn(&s);
    let out = run(&mut s, "ground", json!({"spots": slope(), "interval": 250, "name": "Rilievo"}));
    let id = out["id"].as_u64().expect("an id");
    // What it is made of, and how big it is: a hundred square metres seen from above, and a little more than that
    // of land, because it slopes — a metre of fall over ten is 0.5% more surface.
    assert_eq!(
        (out["points"].as_u64(), out["triangles"].as_u64(), out["lowest"].as_f64(), out["highest"].as_f64()),
        (Some(4), Some(2), Some(0.0), Some(1000.0)),
        "{out}"
    );
    assert_eq!((out["area_m2"].as_f64(), out["surface_m2"].as_f64()), (Some(100.0), Some(100.5)), "{out}");
    assert_eq!(out["name"], json!("Rilievo"));
    // And its own points, which are what it is: the reply holds them as it holds them.
    assert_eq!(
        out["spots"],
        json!([[0.0, 0.0, 1000.0], [10_000.0, 0.0, 0.0], [10_000.0, 10_000.0, 0.0], [0.0, 10_000.0, 1000.0]]),
        "{out}"
    );
    // It stands on no level, because the land is under all of them.
    let e = s.model.elements.get(id).expect("the ground");
    assert_eq!(s.model.level_of(e), None);
    assert_eq!(e.category(), "ground");

    // In 3D it is the surface itself: at the middle of the square it stands halfway down the fall, and off the
    // square there is no ground at all.
    let top = |s: &Session, x: f64, y: f64| crate::view3d::tops_at(&s.model, P2(x, y)).get(&id).copied();
    assert!(top(&s, 5000.0, 5000.0).is_some_and(|z| (z - 500.0).abs() < 0.1), "{:?}", top(&s, 5000.0, 5000.0));
    assert!(top(&s, 2500.0, 2500.0).is_some_and(|z| (z - 750.0).abs() < 0.1));
    assert_eq!(top(&s, -1000.0, 5000.0), None);

    // A plan draws its contours, three of them a quarter of a metre apart: more lines than the empty plan had.
    assert!(drawn(&s) > bare, "the plan draws the ground");
    let lines = crate::ground::contours(&[[0.0, 0.0, 1000.0], [10_000.0, 0.0, 0.0], [10_000.0, 10_000.0, 0.0]], 250.0);
    assert_eq!(lines.len(), 3);

    // A schedule of the ground lists it, with what it covers and what it is.
    let table = run(&mut s, "schedule", json!({"category": "ground"}));
    assert_eq!(table["rows"].as_array().map(Vec::len), Some(1), "{table}");
    assert!(table["columns"].to_string().contains("Surface Area"), "{table}");
}

#[test]
fn a_ground_moves_in_plan_and_keeps_the_heights_its_points_were_measured_at() {
    let mut s = Session::new();
    let id = run(&mut s, "ground", json!({"spots": slope()}))["id"].as_u64().expect("an id");
    run(&mut s, "move", json!({"ids": [id], "by": [5000, 0]}));
    let Some(Element::Ground(g)) = s.model.elements.get(id) else { panic!("the ground") };
    assert_eq!(g.spots[0], [5000.0, 0.0, 1000.0], "moved east, standing as high as it did");
    assert_eq!(g.spots[1], [15_000.0, 0.0, 0.0]);
    // And it survives the project's own file, which says it holds a ground by its format.
    let path = temp("terreno.bimcraft");
    run(&mut s, "save", json!({"path": path.to_string_lossy()}));
    let mut back = Session::new();
    run(&mut back, "open", json!({"path": path.to_string_lossy()}));
    let grounds: Vec<&Element> = back.model.elements.values().filter(|e| matches!(e, Element::Ground(_))).collect();
    assert_eq!(grounds.len(), 1);
    assert_eq!(grounds.first().copied(), s.model.elements.get(id));
    let _ = std::fs::remove_file(path);
}

#[test]
fn the_ground_is_written_to_ifc_as_the_terrain_of_the_site() {
    let mut s = Session::new();
    run(&mut s, "ground", json!({"spots": slope()}));
    let path = temp("terreno.ifc");
    run(&mut s, "save", json!({"path": path.to_string_lossy()}));
    let ifc = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(ifc.matches("IFCGEOGRAPHICELEMENT(").count(), 1, "{ifc}");
    assert!(ifc.contains(",.TERRAIN.)"), "it is terrain and not a building element");
    // Its body is the triangles themselves, and it stands in the site, which holds what is under every storey.
    assert!(ifc.contains("IFCTRIANGULATEDFACESET("), "{ifc}");
    let site = ifc.lines().find(|l| l.contains("IFCSITE(")).and_then(|l| l.split('=').next()).unwrap_or_default().to_string();
    assert!(
        ifc.lines().any(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE(") && l.contains(&format!(",{site})"))),
        "the site holds it, not a storey: {site}"
    );
    // And it comes back: a ground holds nothing but its points, so the points are what the file has to carry.
    let mut back = Session::new();
    let opened = run(&mut back, "open", json!({"path": path.to_string_lossy()}));
    assert_eq!(opened["skipped"], json!([]), "nothing of it was left out: {opened}");
    let Some(Element::Ground(g)) = back.model.elements.values().find(|e| matches!(e, Element::Ground(_))) else {
        panic!("the ground came back")
    };
    let mut spots = g.spots.clone();
    spots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(spots, vec![[0.0, 0.0, 1000.0], [0.0, 10_000.0, 1000.0], [10_000.0, 0.0, 0.0], [10_000.0, 10_000.0, 0.0]]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn what_is_no_ground_is_refused_and_said_why() {
    let mut s = Session::new();
    for (told, why) in [
        (json!({"spots": [[0, 0, 0], [1000, 0, 0]]}), "no surface"),
        (json!({"spots": [[0, 0, 0], [1000, 0, 100], [2000, 0, 200]]}), "one line"),
        (json!({"spots": []}), "no surface"),
        (json!({"spots": [[0, 0, 0], [1000, 0], [0, 1000, 0]]}), "no point"),
        (json!({"spots": [[0, 0, 0], [1000, "x", 0], [0, 1000, 0]]}), "no number"),
        (json!({"spots": slope(), "interval": -250}), "no height"),
    ] {
        let e = s.execute("ground", &told).unwrap_err();
        assert!(e.contains(why), "{told}: {e}");
    }
    // More points than a ground is made of: that is a point cloud, which a project reads as one.
    let many: Vec<[f64; 3]> = (0..crate::ground::MOST_SPOTS + 1).map(|i| [i as f64 * 10.0, (i % 7) as f64 * 10.0, 0.0]).collect();
    let e = s.execute("ground", &json!({"spots": many})).unwrap_err();
    assert!(e.contains(&format!("more than {}", crate::ground::MOST_SPOTS)), "{e}");
    assert!(s.model.elements.values().all(|e| !matches!(e, Element::Ground(_))), "nothing was built of any of them");
}

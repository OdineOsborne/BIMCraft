//! Reading a DXF: what comes in by layer, the unit it is read in, what is left out, and what is refused.

use crate::dxf::{Options, Shape, read};
use crate::geom::P2;

/// A DXF of `entities`, with `header` between its `$`-variables: the pairs of a file, a group code to a line.
fn dxf(header: &str, entities: &str) -> Vec<u8> {
    format!("0\nSECTION\n2\nHEADER\n{header}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n").into_bytes()
}

/// A line from (x1, y1) to (x2, y2) on `layer`.
fn line(layer: &str, x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!("0\nLINE\n8\n{layer}\n10\n{x1}\n20\n{y1}\n11\n{x2}\n21\n{y2}\n")
}

fn read_ok(bytes: &[u8]) -> crate::dxf::Drawing {
    read(bytes, &Options::default()).expect("it reads")
}

#[test]
fn a_drawing_comes_in_layer_by_layer_with_what_is_drawn_on_each() {
    let entities = format!(
        "{}{}{}{}",
        line("MURI", 0.0, 0.0, 5000.0, 0.0),
        line("MURI", 5000.0, 0.0, 5000.0, 4000.0),
        // A closed polyline of four points: a room's outline.
        "0\nLWPOLYLINE\n8\nLOCALI\n90\n4\n70\n1\n10\n0\n20\n0\n10\n5000\n20\n0\n10\n5000\n20\n4000\n10\n0\n20\n4000\n",
        "0\nCIRCLE\n8\nARREDI\n10\n1000\n20\n1000\n40\n250\n",
    );
    let d = read_ok(&dxf("", &entities));
    let names: Vec<&str> = d.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MURI", "LOCALI", "ARREDI"], "the layers, as they were first drawn on");
    assert_eq!(d.shapes(), 4);
    assert_eq!(d.layers[0].shapes[0], Shape::Line([P2(0.0, 0.0), P2(5000.0, 0.0)]));
    match &d.layers[1].shapes[0] {
        Shape::Path { points, closed } => {
            assert_eq!((points.len(), *closed), (4, true));
            assert_eq!(points[2], P2(5000.0, 4000.0));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(d.layers[2].shapes[0], Shape::Arc { at: P2(1000.0, 1000.0), radius: 250.0, from: 0.0, to: 360.0 });
    // The box it all lies in: the circle reaches past the walls at the bottom left.
    assert_eq!(d.extent, Some([P2(0.0, 0.0), P2(5000.0, 4000.0)]));
    assert!(d.left_out.is_empty(), "{:?}", d.left_out);
}

#[test]
fn a_polyline_keeps_its_points_which_are_the_vertices_after_it() {
    let entities = "0\nPOLYLINE\n8\nMURI\n70\n1\n\
                    0\nVERTEX\n8\nMURI\n10\n0\n20\n0\n\
                    0\nVERTEX\n8\nMURI\n10\n3000\n20\n0\n\
                    0\nVERTEX\n8\nMURI\n10\n3000\n20\n2000\n\
                    0\nSEQEND\n8\nMURI\n";
    let d = read_ok(&dxf("", entities));
    assert_eq!(d.layers.len(), 1);
    match &d.layers[0].shapes[0] {
        Shape::Path { points, closed } => {
            assert_eq!((points.as_slice(), *closed), ([P2(0.0, 0.0), P2(3000.0, 0.0), P2(3000.0, 2000.0)].as_slice(), true))
        }
        other => panic!("{other:?}"),
    }
    assert!(d.left_out.is_empty(), "a polyline's vertices are its own, not things left out: {:?}", d.left_out);
}

#[test]
fn the_file_says_its_own_unit_and_the_caller_may_say_another() {
    // A drawing in metres: 5 becomes 5000 mm.
    let metres = dxf("9\n$INSUNITS\n70\n6\n", &line("MURI", 0.0, 0.0, 5.0, 0.0));
    let d = read_ok(&metres);
    assert_eq!(d.scale, 1000.0);
    assert_eq!(d.layers[0].shapes[0], Shape::Line([P2(0.0, 0.0), P2(5000.0, 0.0)]));
    // The caller knows better: centimetres.
    let d = read(&metres, &Options { scale: Some(10.0) }).expect("it reads");
    assert_eq!((d.scale, d.layers[0].shapes[0].clone()), (10.0, Shape::Line([P2(0.0, 0.0), P2(50.0, 0.0)])));
    // A file that says nothing is read as millimetres, which a drawing of a building usually is.
    assert_eq!(read_ok(&dxf("", &line("M", 0.0, 0.0, 5000.0, 0.0))).scale, 1.0);
    // A unit of no length is refused.
    assert!(read(&metres, &Options { scale: Some(0.0) }).is_err());
}

#[test]
fn what_it_does_not_read_is_counted_by_its_own_name_the_most_first() {
    let entities = format!(
        "{}0\nTEXT\n8\nTESTI\n1\nSoggiorno\n0\nTEXT\n8\nTESTI\n1\nCucina\n0\nTEXT\n8\nTESTI\n1\nBagno\n0\nINSERT\n8\nBLOCCHI\n0\nSPLINE\n8\nMURI\n",
        line("MURI", 0.0, 0.0, 1000.0, 0.0)
    );
    let d = read_ok(&dxf("", &entities));
    assert_eq!(d.shapes(), 1, "only the line is geometry this reads");
    assert_eq!(d.left_out, ["3 TEXT", "1 INSERT", "1 SPLINE"], "the most of a kind first, then by name");
}

#[test]
fn a_polylines_bulge_is_read_as_the_line_between_its_points_and_said() {
    let entities = "0\nLWPOLYLINE\n8\nMURI\n90\n3\n10\n0\n20\n0\n42\n0.5\n10\n1000\n20\n0\n10\n1000\n20\n1000\n";
    let d = read_ok(&dxf("", entities));
    assert_eq!(d.shapes(), 1);
    assert_eq!(d.left_out, ["1 polyline arcs, read as the straight line between their points"]);
}

#[test]
fn a_dwg_and_a_binary_dxf_say_what_they_are_and_what_to_do() {
    let dwg = read(b"AC1032\0\0\0\0\0\0", &Options::default()).expect_err("refused");
    assert!(dwg.contains("DWG") && dwg.contains("convert it to DXF"), "{dwg}");
    let binary = read(b"AutoCAD Binary DXF\r\n\x1a\0", &Options::default()).expect_err("refused");
    assert!(binary.contains("binary") && binary.contains("ASCII"), "{binary}");
}

#[test]
fn a_file_that_is_no_drawing_comes_back_empty_rather_than_breaking() {
    for bytes in [
        &b""[..],
        b"hello",
        // Group codes with nothing under them, and a value with no code.
        b"0\n",
        b"10\nnot a number\n20\n\n0\nLINE\n",
        // An entity whose points are nonsense, and one too far from the origin to be a drawing.
        b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nM\n10\nNaN\n20\n0\n11\n1\n21\n0\n0\nENDSEC\n",
        b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nM\n10\n1e300\n20\n0\n11\n1\n21\n0\n0\nENDSEC\n",
        b"0\nSECTION\n2\nENTITIES\n0\nARC\n8\nM\n10\n0\n20\n0\n40\n-5\n0\nENDSEC\n",
        // A polyline of one point is no path, and a vertex with no polyline open belongs to nothing.
        b"0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\nM\n10\n0\n20\n0\n0\nVERTEX\n10\n5\n20\n5\n0\nENDSEC\n",
    ] {
        let d = read(bytes, &Options::default()).expect("it reads, and says what it found");
        assert_eq!(d.shapes(), 0, "{:?} from {:?}", d.layers, String::from_utf8_lossy(bytes));
    }
}

#[test]
fn an_entity_with_no_layer_is_on_layer_zero_as_a_drawing_has_it() {
    let d = read_ok(&dxf("", "0\nLINE\n10\n0\n20\n0\n11\n100\n21\n0\n"));
    assert_eq!(d.layers[0].name, "0");
}

#[test]
fn a_polyline_of_more_points_than_one_holds_is_left_out_whole() {
    let mut entities = String::from("0\nLWPOLYLINE\n8\nMURI\n");
    for i in 0..=crate::dxf::MAX_VERTICES {
        entities.push_str(&format!("10\n{i}\n20\n0\n"));
    }
    let d = read_ok(&dxf("", &entities));
    assert_eq!(d.shapes(), 0, "half a drawing is worse than none of it");
    assert_eq!(d.left_out, ["1 LWPOLYLINE"]);
}

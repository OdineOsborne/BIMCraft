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
    assert_eq!(d.left_out, ["3 TEXT", "1 INSERT of a block the file doesn't hold", "1 SPLINE"], "the most of a kind first, then by name");
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

/// A block named `name`, holding `entities`, with its base point at `at`.
fn block(name: &str, at: (f64, f64), entities: &str) -> String {
    format!("0\nBLOCK\n2\n{name}\n10\n{}\n20\n{}\n{entities}0\nENDBLK\n", at.0, at.1)
}

/// An INSERT of the block `name` on `layer`, put at `at`, turned `angle` degrees and scaled by `scale`.
fn insert(layer: &str, name: &str, at: (f64, f64), angle: f64, scale: (f64, f64)) -> String {
    format!("0\nINSERT\n8\n{layer}\n2\n{name}\n10\n{}\n20\n{}\n41\n{}\n42\n{}\n50\n{angle}\n", at.0, at.1, scale.0, scale.1)
}

/// A DXF whose BLOCKS section holds `blocks` and whose ENTITIES hold `entities`.
fn with_blocks(blocks: &str, entities: &str) -> Vec<u8> {
    format!("0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n").into_bytes()
}

/// Whether a shape is the line between these two points, to the millionth of a millimetre: a block turned a right
/// angle lands on its point through a sine, not on the number a draughtsman typed.
fn is_line(shape: &Shape, [a, b]: [P2; 2]) -> bool {
    matches!(shape, Shape::Line([p, q]) if p.dist(a) < 1e-6 && q.dist(b) < 1e-6)
}

#[test]
fn a_block_is_drawn_wherever_it_is_put_turned_and_scaled_as_the_insert_says() {
    let blocks = block("PORTA", (0.0, 0.0), &line("0", 0.0, 0.0, 800.0, 0.0));
    let entities = format!(
        "{}{}{}",
        insert("MURI", "PORTA", (1000.0, 2000.0), 0.0, (1.0, 1.0)),
        insert("MURI", "PORTA", (4000.0, 2000.0), 90.0, (1.0, 1.0)),
        insert("MURI", "PORTA", (0.0, 0.0), 0.0, (-1.0, 2.0)),
    );
    let d = read_ok(&with_blocks(&blocks, &entities));
    assert_eq!(d.shapes(), 3, "{:?}", d.layers);
    assert_eq!(d.layers[0].name, "MURI", "what is drawn on layer 0 in a block takes the layer the block is put on");
    let drawn = &d.layers[0].shapes;
    assert!(is_line(&drawn[0], [P2(1000.0, 2000.0), P2(1800.0, 2000.0)]), "{:?}", drawn[0]);
    assert!(is_line(&drawn[1], [P2(4000.0, 2000.0), P2(4000.0, 2800.0)]), "turned a right angle: {:?}", drawn[1]);
    assert!(is_line(&drawn[2], [P2(0.0, 0.0), P2(-800.0, 0.0)]), "mirrored: {:?}", drawn[2]);
    assert!(d.left_out.is_empty(), "{:?}", d.left_out);
}

#[test]
fn a_block_is_drawn_from_its_base_point_and_keeps_the_layers_its_own_lines_name() {
    // The base point is where the insertion point lands, so a block drawn away from the origin is not put twice.
    let blocks = block(
        "TAVOLO",
        (1000.0, 1000.0),
        &format!("{}{}", line("0", 1000.0, 1000.0, 1800.0, 1000.0), line("ARREDI", 1000.0, 1000.0, 1000.0, 1600.0)),
    );
    let d = read_ok(&with_blocks(&blocks, &insert("MURI", "TAVOLO", (5000.0, 0.0), 0.0, (1.0, 1.0))));
    assert_eq!(d.shapes(), 2);
    let names: Vec<&str> = d.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MURI", "ARREDI"], "a line on a layer of its own keeps it; one on layer 0 takes the insert's");
    assert!(is_line(&d.layers[0].shapes[0], [P2(5000.0, 0.0), P2(5800.0, 0.0)]), "{:?}", d.layers[0].shapes[0]);
    assert!(is_line(&d.layers[1].shapes[0], [P2(5000.0, 0.0), P2(5000.0, 600.0)]), "{:?}", d.layers[1].shapes[0]);
}

#[test]
fn a_round_thing_in_a_block_is_drawn_where_the_scale_keeps_it_round() {
    let round = "0\nCIRCLE\n8\n0\n10\n0\n20\n0\n40\n250\n0\nARC\n8\n0\n10\n0\n20\n0\n40\n500\n50\n0\n51\n90\n";
    let blocks = block("TONDO", (0.0, 0.0), round);
    // Scaled evenly, a circle is a circle: the radius goes with it and an arc turns with the block.
    let d = read_ok(&with_blocks(&blocks, &insert("M", "TONDO", (100.0, 100.0), 90.0, (2.0, 2.0))));
    assert_eq!(d.shapes(), 2, "{:?}", d.layers);
    assert_eq!(d.layers[0].shapes[0], Shape::Arc { at: P2(100.0, 100.0), radius: 500.0, from: 0.0, to: 360.0 });
    match &d.layers[0].shapes[1] {
        Shape::Arc { radius, from, to, .. } => {
            assert!((*radius - 1000.0).abs() < 1e-6 && (*from - 90.0).abs() < 1e-6 && (*to - 180.0).abs() < 1e-6, "{radius} {from} {to}")
        }
        other => panic!("{other:?}"),
    }
    // Scaled unevenly it is an ellipse, which this does not draw, so it is left out and said.
    let flat = read_ok(&with_blocks(&blocks, &insert("M", "TONDO", (0.0, 0.0), 0.0, (1.0, 2.0))));
    assert_eq!(flat.shapes(), 0);
    assert_eq!(flat.left_out, ["2 round things in a block scaled into ellipses"]);
}

#[test]
fn a_block_that_holds_another_is_opened_too_and_one_that_holds_itself_does_not_run_away() {
    let blocks = format!(
        "{}{}",
        block("ANTA", (0.0, 0.0), &line("0", 0.0, 0.0, 800.0, 0.0)),
        block("PORTA", (0.0, 0.0), &format!("{}{}", line("0", 0.0, 0.0, 0.0, 100.0), insert("0", "ANTA", (0.0, 100.0), 0.0, (1.0, 1.0)))),
    );
    let d = read_ok(&with_blocks(&blocks, &insert("MURI", "PORTA", (2000.0, 0.0), 0.0, (1.0, 1.0))));
    assert_eq!(d.shapes(), 2, "{:?}", d.layers);
    assert!(is_line(&d.layers[0].shapes[1], [P2(2000.0, 100.0), P2(2800.0, 100.0)]), "{:?}", d.layers[0].shapes[1]);
    // A block that puts itself inside itself is a drawing with no end to it, and a drawing has an end.
    let itself =
        block("GIRO", (0.0, 0.0), &format!("{}{}", line("0", 0.0, 0.0, 100.0, 0.0), insert("0", "GIRO", (100.0, 0.0), 0.0, (1.0, 1.0))));
    let d = read_ok(&with_blocks(&itself, &insert("MURI", "GIRO", (0.0, 0.0), 0.0, (1.0, 1.0))));
    assert!(d.shapes() <= crate::dxf::MAX_NESTING, "{} shapes from a block inside itself", d.shapes());
    assert!(d.left_out.iter().any(|w| w.contains("deep")), "{:?}", d.left_out);
}

#[test]
fn the_words_a_survey_writes_come_in_where_it_writes_them() {
    // A `TEXT`: what it says under code 1, where it stands under 10 and 20, how tall under 40, turned by 50.
    let words = "0\nTEXT\n8\nTESTI\n1\nSoggiorno\n10\n1000\n20\n2000\n40\n250\n50\n90\n";
    let d = read_ok(&dxf("", words));
    assert_eq!(d.layers[0].name, "TESTI");
    assert_eq!(d.layers[0].shapes[0], Shape::Words { at: P2(1000.0, 2000.0), text: "Soggiorno".into(), height: 250.0, angle: 90.0 });
    assert!(d.left_out.is_empty(), "{:?}", d.left_out);
    // One that says nothing, or stands nowhere, is no word of the drawing and is counted like anything else.
    let bare = read_ok(&dxf("", "0\nTEXT\n8\nTESTI\n1\nCucina\n0\nTEXT\n8\nTESTI\n10\n0\n20\n0\n40\n250\n"));
    assert_eq!(bare.shapes(), 0);
    assert_eq!(bare.left_out, ["2 TEXT"]);
    // An `MTEXT` says the same thing with the paragraph it is written in: the formatting is not read, and what it
    // says is.
    // Its point is where it hangs from, the top left of it unless it says otherwise, so the line it stands on is a
    // word's height under that.
    let para = "0\nMTEXT\n8\nTESTI\n10\n0\n20\n0\n40\n200\n1\n{\\fArial|b1;Camera}\\Pmatrimoniale\n";
    let d = read_ok(&dxf("", para));
    assert_eq!(d.layers[0].shapes[0], Shape::Words { at: P2(0.0, -200.0), text: "Camera matrimoniale".into(), height: 200.0, angle: 0.0 });
}

#[test]
fn a_word_inside_a_block_is_written_where_the_block_puts_it() {
    // A survey draws a window as a block and writes its size in it, so a block's words come in with its lines: as
    // tall as the block makes them and turned with it.
    let blocks = block("FINESTRA", (0.0, 0.0), "0\nTEXT\n8\n0\n1\nF1\n10\n0\n20\n0\n40\n100\n");
    let d = read_ok(&with_blocks(&blocks, &insert("TESTI", "FINESTRA", (3000.0, 1000.0), 90.0, (2.0, 2.0))));
    assert_eq!(
        d.layers[0].shapes[0],
        Shape::Words { at: P2(3000.0, 1000.0), text: "F1".into(), height: 200.0, angle: 90.0 },
        "{:?}",
        d.layers[0].shapes[0]
    );
}

#[test]
fn the_words_of_a_drawing_are_drawn_under_the_plan_and_are_no_lines_of_the_building() {
    // A word is drawn with the strokes its letters are drawn with, so it is seen under a plan like everything else.
    let m = crate::model::Model::new("Test");
    let mut m = m;
    let layers = vec![crate::dxf::Layer {
        name: "TESTI".into(),
        shapes: vec![Shape::Words { at: P2(0.0, 0.0), text: "AB".into(), height: 1000.0, angle: 0.0 }],
    }];
    m.drawings.insert("rilievo.dxf".into(), std::sync::Arc::new(crate::dxf::Drawing { layers, scale: 1.0, ..Default::default() }));
    let u = crate::model::Underlay { name: "Rilievo".into(), path: "rilievo.dxf".into(), scale: 1.0, ..Default::default() };
    assert!(crate::underlay::lines(&m, &u).is_empty(), "a word is no line to snap to or to trace");
    let drawn = crate::underlay::words(&m, &u);
    assert!(drawn.len() >= 4, "A and B are drawn with four strokes between them: {}", drawn.len());
    assert!(drawn.iter().flat_map(|(_, points)| points).all(|p| p.1 >= -1.0), "a word stands on its line, y up: {drawn:?}");
}

#[test]
fn a_block_put_a_million_times_over_a_million_stops_at_what_a_drawing_holds() {
    // A grid of copies of a block that holds nothing draws nothing, so nothing in the drawing would ever stop it:
    // how many times a block is opened at all is what is bounded.
    let empty = block("NIENTE", (0.0, 0.0), "");
    let grid = "0\nINSERT\n8\nM\n2\nNIENTE\n10\n0\n20\n0\n70\n2000000\n71\n2000000\n44\n10\n45\n10\n";
    let d = read_ok(&with_blocks(&empty, grid));
    assert_eq!(d.shapes(), 0);
    assert!(d.left_out.iter().any(|w| w.contains("a drawing holds")), "{:?}", d.left_out);
}

#[test]
fn an_insert_of_a_block_the_file_does_not_hold_is_counted_and_nothing_else() {
    let d =
        read_ok(&with_blocks("", &format!("{}{}", insert("B", "MANCA", (0.0, 0.0), 0.0, (1.0, 1.0)), line("MURI", 0.0, 0.0, 1000.0, 0.0))));
    assert_eq!(d.shapes(), 1);
    assert_eq!(d.left_out, ["1 INSERT of a block the file doesn't hold"]);
}

#[test]
fn a_session_without_a_converter_says_what_a_dwg_is_and_stops() {
    let dir = std::env::temp_dir().join(format!("bimcraft-dwg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("piano.dwg");
    std::fs::write(&path, b"AC1032\0\0\0\0\0\0").unwrap();
    let mut s = crate::engine::Session::new();
    let err = s.execute("dxf", &serde_json::json!({"path": path.to_string_lossy()})).expect_err("refused");
    assert!(err.contains("is a DWG") && err.contains("convert"), "{err}");
}

#[test]
fn a_session_with_one_reads_a_dwg_wherever_it_reads_a_dxf() {
    let dir = std::env::temp_dir().join(format!("bimcraft-dwg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("convertito.dwg");
    std::fs::write(&path, b"AC1032\0\0\0\0\0\0").unwrap();
    let mut s = crate::engine::Session::new();
    // What the applications hand over, here a stand-in: the core knows only bytes in, bytes out.
    s.dwg = Some(std::sync::Arc::new(|bytes: &[u8]| {
        assert!(crate::dxf::is_dwg(bytes), "it is given the DWG's own bytes");
        Ok((
            b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMURI\n10\n0\n20\n0\n11\n1000\n21\n0\n0\nENDSEC\n0\nEOF\n".to_vec(),
            vec!["3 PROXY left out".into()],
        ))
    }));
    let out = s.execute("dxf", &serde_json::json!({"path": path.to_string_lossy()})).expect("it reads");
    assert_eq!(out["shapes"], 1, "the DXF it was turned into");
    assert_eq!(out["layers"][0]["name"], "MURI");
    assert_eq!(out["converted"], serde_json::json!(["3 PROXY left out"]), "what the DWG reader said it could not read");
}

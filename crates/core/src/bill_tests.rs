//! The bill of quantities (`bill.rs`): several codes on one element, the quantity each unit is for, what is not
//! measured, the spreadsheet it leaves as, and the price list in use.

use serde_json::json;

use crate::engine::Session;

fn file(name: &str, text: &str) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-bill-{}-{n}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

const LIST: &str = "Codice;Descrizione;UM;Prezzo\n\
    M.01;Muratura in blocchi di laterizio;m3;250,00\n\
    I.01;Intonaco civile per interni;m2;22,50\n\
    F.01;Ferro per cemento armato;kg;1,80\n\
    S.01;Soglia in pietra;cad;80,00\n";

/// Two walls 5 m and 4 m long, 3 m high and 250 mm thick, and a door in the first; a price list.
fn site() -> Session {
    let mut s = Session::new();
    s.script(
        "type {\"category\": \"wall\", \"name\": \"Muratura 250\", \"thickness\": 250}\n\
         wall {\"points\": [[0,0],[5000,0]], \"type\": \"Muratura 250\", \"height\": 3000}\n\
         wall {\"points\": [[0,4000],[4000,4000]], \"type\": \"Muratura 250\", \"height\": 3000}\n\
         type {\"category\": \"door\", \"name\": \"P 900\", \"width\": 900, \"height\": 2100}",
    )
    .expect("the walls");
    let first = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Wall(_))).map(|(id, _)| id).expect("a wall");
    s.execute("door", &json!({"wall": first, "offset": 1000, "type": "P 900"})).expect("a door");
    s.execute("price_list", &json!({"name": "Liguria", "path": file("liguria.csv", LIST), "code": "Codice", "description": "Descrizione", "unit": "UM", "price": "Prezzo"}))
        .expect("the price list");
    s
}

#[test]
fn one_element_carries_several_codes_each_measured_by_its_own_unit() {
    let mut s = site();
    s.execute("price_code", &json!({"code": "M.01", "categories": ["walls"]})).expect("the masonry");
    // Plaster on both faces: the same code, measured twice.
    s.execute("price_code", &json!({"code": "I.01", "categories": ["walls"], "add": true, "times": 2})).expect("the plaster");
    let out = s.execute("bill_of_quantities", &json!({})).expect("the bill");
    let lines = out["lines"].as_array().expect("lines");
    assert_eq!(lines.len(), 2, "{out}");
    // 5 × 3 × 0,25 less the door (0,9 × 2,1 × 0,25) plus 4 × 3 × 0,25: 3,75 − 0,4725 + 3 = 6,2775 → 6,28 m³.
    let masonry = lines.iter().find(|l| l["code"] == "M.01").expect("M.01");
    assert_eq!(masonry["unit"], "m3");
    let q = masonry["quantity"].as_f64().expect("a quantity");
    assert!((q - 6.2775).abs() < 0.006, "{q}");
    // Net area 15 − 1,89 + 12 = 25,11 m², twice: 50,22 m² at 22,50.
    let plaster = lines.iter().find(|l| l["code"] == "I.01").expect("I.01");
    assert_eq!(plaster["quantity"].as_f64(), Some(50.22));
    assert_eq!(plaster["amount"].as_f64(), Some(1129.95));
    assert_eq!(plaster["elements"].as_array().map(Vec::len), Some(2), "the two walls are listed under it");
    assert_eq!(plaster["elements"][0]["times"], 2.0);
    let total = out["total"].as_f64().expect("a total");
    let sum: f64 = lines.iter().filter_map(|l| l["amount"].as_f64()).sum();
    assert!((total - sum).abs() < 0.005, "{total} {sum}");
}

#[test]
fn a_unit_nothing_of_an_element_measures_is_said_and_not_guessed() {
    let mut s = site();
    s.execute("price_code", &json!({"code": "F.01", "categories": ["walls"]})).expect("the steel");
    let out = s.execute("bill_of_quantities", &json!({})).expect("the bill");
    let line = &out["lines"][0];
    assert!(line["quantity"].is_null() && line["amount"].is_null(), "kilograms are not a wall's: {line}");
    assert_eq!(out["unmeasured"], json!(["F.01"]));
    assert_eq!(out["total"], 0.0);
}

#[test]
fn a_unit_of_pieces_counts_the_elements() {
    let mut s = site();
    s.execute("price_code", &json!({"code": "S.01", "categories": ["doors"]})).expect("the sill");
    let out = s.execute("bill_of_quantities", &json!({})).expect("the bill");
    assert_eq!(out["lines"][0]["quantity"].as_f64(), Some(1.0));
    assert_eq!(out["lines"][0]["amount"].as_f64(), Some(80.0));
}

#[test]
fn the_bill_and_its_specification_leave_as_spreadsheets_in_the_projects_language() {
    let mut s = site();
    s.execute("price_code", &json!({"code": "I.01", "categories": ["walls"], "times": 2})).expect("the plaster");
    s.model_mut().language = crate::language::Language::Italian;
    let dir = std::env::temp_dir().join(format!("bimcraft-bill-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (bill, spec) = (dir.join("computo.csv"), dir.join("capitolato.csv"));
    s.execute("bill_of_quantities", &json!({"path": bill.to_string_lossy(), "spec_path": spec.to_string_lossy()})).expect("written");
    let b = std::fs::read_to_string(&bill).expect("the bill");
    let first = b.lines().next().unwrap_or_default();
    assert_eq!(first, "N.;Codice;Descrizione;U.M.;Quantità;Prezzo unitario (EUR);Importo (EUR)", "{b}");
    // A decimal comma, as an Italian spreadsheet reads it.
    assert!(b.contains("I.01;Intonaco civile per interni;m2;50,22;22,50;1129,95"), "{b}");
    assert!(b.lines().last().unwrap_or_default().contains("Totale") && b.contains("1129,95"), "{b}");
    let sp = std::fs::read_to_string(&spec).expect("the specification");
    assert!(sp.contains("1;I.01;Intonaco civile per interni;m2;22,50"), "{sp}");
}

#[test]
fn a_code_can_be_added_beside_another_and_taken_off_alone() {
    let mut s = site();
    s.execute("price_code", &json!({"code": "M.01", "categories": ["walls"]})).expect("the masonry");
    s.execute("price_code", &json!({"code": "I.01", "categories": ["walls"], "add": true, "times": 2})).expect("the plaster");
    let wall = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Wall(_))).map(|(_, e)| e.clone()).expect("a wall");
    assert_eq!(s.model.article_code(&wall), Some("M.01; I.01 x2"));
    // Given again, a code already there is not doubled.
    s.execute("price_code", &json!({"code": "I.01", "categories": ["walls"], "add": true, "times": 2})).expect("again");
    let wall = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Wall(_))).map(|(_, e)| e.clone()).expect("a wall");
    assert_eq!(s.model.article_code(&wall), Some("M.01; I.01 x2"));
    s.execute("price_code", &json!({"code": "M.01", "categories": ["walls"], "remove": true})).expect("off");
    let wall = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Wall(_))).map(|(_, e)| e.clone()).expect("a wall");
    assert_eq!(s.model.article_code(&wall), Some("I.01 x2"));
}

#[test]
fn the_price_list_in_use_is_the_one_prices_come_from() {
    let mut s = site();
    // A second list with the same code at another price, as a province's and a region's often are.
    let other = "Codice;Descrizione;UM;Prezzo\nI.01;Intonaco civile;m2;19,00\n";
    s.execute("price_list", &json!({"name": "Savona", "path": file("savona.csv", other), "code": "Codice", "description": "Descrizione", "unit": "UM", "price": "Prezzo"}))
        .expect("the second list");
    s.execute("price_code", &json!({"code": "I.01", "categories": ["walls"]})).expect("the plaster");
    // Without a choice, the first list by name that holds it: Liguria.
    assert_eq!(s.execute("bill_of_quantities", &json!({})).expect("bill")["lines"][0]["price"], 22.5);
    let out = s.execute("price_list_use", &json!({"name": "Savona"})).expect("chosen");
    assert_eq!(out["in_use"], "Savona");
    assert_eq!(s.execute("bill_of_quantities", &json!({})).expect("bill")["lines"][0]["price"], 19.0);
    // It is kept in the project's file.
    let (back, _) = crate::model::Model::read(&s.model.to_json().expect("json")).expect("read");
    assert_eq!(back.price_list_in_use.as_deref(), Some("Savona"));
    // A list the project hasn't is refused, and no name goes back to every list.
    assert!(s.execute("price_list_use", &json!({"name": "Genova"})).is_err());
    s.execute("price_list_use", &json!({})).expect("back to all");
    assert_eq!(s.execute("bill_of_quantities", &json!({})).expect("bill")["lines"][0]["price"], 22.5);
}

#[test]
fn codes_read_their_times_and_a_whole_code_is_never_cut() {
    let s = site();
    let m = &s.model;
    assert_eq!(crate::bill::codes(m, "A.1; B.2 x2"), vec![("A.1".to_string(), 1.0), ("B.2".to_string(), 2.0)]);
    assert_eq!(crate::bill::codes(m, "B.2 ×3"), vec![("B.2".to_string(), 3.0)]);
    assert_eq!(crate::bill::codes(m, "B.2*1,5"), vec![("B.2".to_string(), 1.5)]);
    // An x inside a code is part of it.
    assert_eq!(crate::bill::codes(m, "AX.10"), vec![("AX.10".to_string(), 1.0)]);
    assert_eq!(crate::bill::codes(m, "  "), Vec::<(String, f64)>::new());
}

#[test]
fn a_unit_is_read_as_a_published_list_writes_it() {
    use crate::bill::{Measure, measure_of};
    assert_eq!(measure_of("metro lineare (m)"), Measure::Length);
    assert_eq!(measure_of("metro quadrato (m²)"), Measure::Area);
    assert_eq!(measure_of("m³"), Measure::Volume);
    assert_eq!(measure_of("cadauno (cad)"), Measure::Count);
    // A screed priced by the centimetre of thickness is an area, given as many times as it is thick.
    assert_eq!(measure_of("m²/cm"), Measure::Area);
    assert_eq!(measure_of("Kg"), Measure::None);
    assert_eq!(measure_of("h"), Measure::None);
}

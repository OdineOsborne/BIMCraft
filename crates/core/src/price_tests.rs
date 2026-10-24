//! Price lists: importing one, the mapping being the caller's, what a file keeps, and what is refused.

use serde_json::json;

use crate::engine::Session;
use crate::library::Kind;
use crate::model::{MAX_PRICE_ITEMS, Model};

/// A file of `text` in a folder of this test's own, and its path.
fn file(name: &str, text: &str) -> String {
    let dir = std::env::temp_dir().join(format!("bimcraft-price-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

fn run(s: &mut Session, id: &str, p: serde_json::Value) -> Result<serde_json::Value, String> {
    s.execute(id, &p)
}

#[test]
fn a_price_list_comes_in_with_its_items_and_the_library_gives_them_ids_and_guids() {
    let path = file("prezzi.csv", "Codice,Descrizione,UM,Prezzo\nA.01.01,Calcestruzzo C25/30,m3,142.50\nA.01.02,Acciaio B450C,kg,1.85\n");
    let mut s = Session::new();
    let out = run(
        &mut s,
        "price_list",
        json!({"name": "Prezzario", "path": path, "code": "Codice", "description": "Descrizione", "unit": "UM", "price": "Prezzo"}),
    )
    .expect("imports");
    assert_eq!(out["name"], "Prezzario");
    assert_eq!(out["items"], 2);

    let list = &s.model.price_lists["Prezzario"];
    assert_eq!(list.items.len(), 2);
    let item = &list.items["A.01.01"];
    assert_eq!((item.description.as_str(), item.unit.as_str(), item.price), ("Calcestruzzo C25/30", "m3", 142.5));
    // The mapping it was read with is kept with the list, so the same file can be read again the same way.
    assert_eq!(list.mapping.code, "Codice");
    assert_eq!(list.mapping.price, "Prezzo");

    // The library gives the list and every item an id and a GUID, as it does every other entry.
    let names = s.model.library_names();
    assert!(names.contains(&(Kind::PriceList, "Prezzario".to_string())), "{names:?}");
    assert!(names.contains(&(Kind::PriceItem, "Prezzario: A.01.01".to_string())), "{names:?}");
    assert!(s.model.library_in_step(), "every entry has its id and GUID");
}

#[test]
fn the_columns_are_the_callers_so_another_files_headings_need_no_code() {
    // Another region, other headings, a semicolon, and the price before the unit.
    let path = file("altro.csv", "ART;VOCE;IMPORTO;MISURA\n1.2.3;Muratura in laterizio;89,40;m2\n");
    let mut s = Session::new();
    run(
        &mut s,
        "price_list",
        json!({"name": "Altra regione", "path": path, "code": "ART", "description": "VOCE", "unit": "MISURA", "price": "IMPORTO"}),
    )
    .expect("imports");
    let item = &s.model.price_lists["Altra regione"].items["1.2.3"];
    // A comma is a decimal point here, as an Italian price list writes it.
    assert_eq!((item.description.as_str(), item.unit.as_str(), item.price), ("Muratura in laterizio", "m2", 89.4));
}

#[test]
fn a_price_list_survives_the_file_it_is_saved_in() {
    let path = file("round.csv", "Codice,Prezzo\nX.1,10.5\n");
    let mut s = Session::new();
    run(&mut s, "price_list", json!({"name": "L", "path": path, "code": "Codice", "price": "Prezzo"})).expect("imports");
    let json = s.model.to_json().expect("writes the project");
    let (back, _left_out) = Model::read(&json).expect("reads back");
    assert_eq!(back.price_lists["L"].items["X.1"].price, 10.5);
    assert!(back.library_in_step(), "the entries keep their ids and GUIDs");
}

#[test]
fn a_row_whose_price_is_no_number_is_left_out_and_said() {
    let path = file("sporco.csv", "Codice,Prezzo\nA,12.0\nB,da definire\nC,\n");
    let mut s = Session::new();
    let out = run(&mut s, "price_list", json!({"name": "L", "path": path, "code": "Codice", "price": "Prezzo"})).expect("imports");
    assert_eq!(out["items"], 1, "only the row that has a price");
    let left = out["left_out"].as_array().expect("it says what it left out");
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.iter().any(|s| s.as_str().is_some_and(|s| s.contains('B'))), "{left:?}");
}

#[test]
fn a_file_with_more_rows_than_a_list_holds_is_refused() {
    let mut text = String::from("Codice,Prezzo\n");
    for i in 0..=MAX_PRICE_ITEMS {
        text.push_str(&format!("C{i},1.0\n"));
    }
    let path = file("troppi.csv", &text);
    let mut s = Session::new();
    let err = run(&mut s, "price_list", json!({"name": "L", "path": path, "code": "Codice", "price": "Prezzo"})).expect_err("refused");
    assert!(err.contains(&MAX_PRICE_ITEMS.to_string()), "{err}");
    assert!(s.model.price_lists.is_empty(), "nothing was left behind");
}

#[test]
fn a_column_the_file_has_not_is_refused_naming_it() {
    let path = file("manca.csv", "Codice,Prezzo\nA,1.0\n");
    let mut s = Session::new();
    let err = run(&mut s, "price_list", json!({"name": "L", "path": path, "code": "Articolo", "price": "Prezzo"})).expect_err("refused");
    assert!(err.contains("Articolo"), "{err}");
}

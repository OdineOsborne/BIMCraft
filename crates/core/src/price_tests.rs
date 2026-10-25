//! Price lists: importing one, the mapping being the caller's, what a file keeps, and what is refused.

use serde_json::json;

use crate::engine::Session;
use crate::library::Kind;
use crate::model::{MAX_PRICE_ITEMS, Model};

/// A file of `text` in a folder of this call's own, and its path. The folder is counted as well as named, because the
/// tests run side by side in one process and two of them write a file of the same name: one would read what the other
/// was still writing.
fn file(name: &str, text: &str) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bimcraft-price-{}-{n}-{name}", std::process::id()));
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

#[test]
fn the_columns_of_a_file_come_back_with_the_role_each_heading_gives_away() {
    let path = file(
        "regionale.csv",
        "ARTICOLO;DESCRIZIONE DEI LAVORI;U.M.;PREZZO UNITARIO\nA.01.010;Calcestruzzo C25/30;m3;142,50\nA.01.020;Acciaio B450C;kg;1,85\n",
    );
    let mut s = Session::new();
    let out = run(&mut s, "price_columns", json!({"path": path})).expect("reads the headings");
    assert_eq!(out["separator"], ";");
    assert_eq!(out["columns"], json!(["ARTICOLO", "DESCRIZIONE DEI LAVORI", "U.M.", "PREZZO UNITARIO"]));
    assert_eq!(out["row_count"], 2);
    // The rows come back as the file spells them, for looking at before the list is read.
    assert_eq!(out["rows"][0], json!(["A.01.010", "Calcestruzzo C25/30", "m3", "142,50"]));
    // Every role is given away by its heading's own words, so the window proposes the whole mapping.
    assert_eq!(out["guess"]["code"], "ARTICOLO");
    assert_eq!(out["guess"]["description"], "DESCRIZIONE DEI LAVORI");
    assert_eq!(out["guess"]["unit"], "U.M.");
    assert_eq!(out["guess"]["price"], "PREZZO UNITARIO");
    // It reads: the project is untouched and it is no undo step.
    assert!(s.model.price_lists.is_empty());
    assert!(s.history().is_empty(), "{:?}", s.history());
}

#[test]
fn a_heading_that_gives_nothing_away_is_guessed_at_nothing_and_the_columns_still_come_back() {
    let path = file("mute.csv", "A,B,C\n1,2,3\n");
    let mut s = Session::new();
    let out = run(&mut s, "price_columns", json!({"path": path})).expect("reads the headings");
    assert_eq!(out["columns"], json!(["A", "B", "C"]));
    assert_eq!(out["guess"], json!({}), "nothing is guessed at: the user picks");
}

#[test]
fn short_words_are_not_looked_for_inside_a_heading() {
    // `um` is a unit and `art` a code, but `consumo` and `partita` are neither: a word under four letters has to be the
    // whole heading.
    let path = file("parole.csv", "PARTITA;CONSUMO;PREZZO\nA,1,2\n");
    let mut s = Session::new();
    let out = run(&mut s, "price_columns", json!({"path": path})).expect("reads the headings");
    assert_eq!(out["guess"], json!({"price": "PREZZO"}));
}

#[test]
fn only_the_first_rows_come_back_however_long_the_file_is() {
    let mut text = String::from("Codice,Prezzo\n");
    for i in 0..50 {
        text.push_str(&format!("C{i},1.0\n"));
    }
    let path = file("lungo.csv", &text);
    let mut s = Session::new();
    let out = run(&mut s, "price_columns", json!({"path": path})).expect("reads the headings");
    assert_eq!(out["row_count"], 50);
    assert_eq!(out["rows"].as_array().map(Vec::len), Some(6), "enough to look at, no more");
}

#[test]
fn the_separator_may_be_the_callers_and_the_columns_follow_it() {
    let path = file("punti.csv", "Codice,Descrizione;Prezzo\nA.1,Scavo,10.50\n");
    let mut s = Session::new();
    // A comma splits this heading into the most columns, so that is what it is read with; a caller who knows better says so.
    assert_eq!(run(&mut s, "price_columns", json!({"path": path.clone()})).expect("read")["columns"].as_array().map(Vec::len), Some(2));
    let out = run(&mut s, "price_columns", json!({"path": path, "separator": ";"})).expect("read");
    assert_eq!(out["columns"], json!(["Codice,Descrizione", "Prezzo"]));
}

/// A project with two wall types, three walls, and a price list of two articles.
fn site() -> (Session, String) {
    let path = file(
        "muri.csv",
        "Codice;Descrizione;UM;Prezzo\n         B.02.100;Muratura in blocchi di laterizio;m2;89,40\n         B.02.200;Parete in cartongesso;m2;34,10\n",
    );
    let mut s = Session::new();
    s.script(
        "type {\"category\": \"wall\", \"name\": \"Muratura 250\", \"thickness\": 250}\n         type {\"category\": \"wall\", \"name\": \"Cartongesso 100\", \"thickness\": 100}\n         wall {\"points\": [[0,0],[5000,0]], \"type\": \"Muratura 250\", \"height\": 3000}\n         wall {\"points\": [[0,0],[4000,0]], \"type\": \"Muratura 250\", \"height\": 3000}\n         wall {\"points\": [[0,2000],[6000,2000]], \"type\": \"Cartongesso 100\", \"height\": 3000}",
    )
    .expect("the site is built");
    run(
        &mut s,
        "price_list",
        json!({"name": "Prezzario", "path": path, "code": "Codice", "description": "Descrizione", "unit": "UM", "price": "Prezzo"}),
    )
    .expect("the price list comes in");
    (s, "Prezzario".to_string())
}

/// The walls of type `name`.
fn walls_of(s: &Session, name: &str) -> Vec<crate::model::Id> {
    s.model.elements.iter().filter(|(_, e)| matches!(e, crate::model::Element::Wall(w) if w.type_name == name)).map(|(id, _)| *id).collect()
}

#[test]
fn a_code_given_to_a_filter_reaches_the_walls_it_picks_and_no_others() {
    let (mut s, _) = site();
    let out = run(
        &mut s,
        "price_code",
        json!({"code": "B.02.100", "categories": ["walls"], "rules": [{"field": "Type", "op": "contains", "value": "Muratura"}]}),
    )
    .expect("the code is given");
    assert_eq!(out["elements"], 2, "the two walls of that type, and not the third");
    assert_eq!(out["parameter"], "Article Code");

    // The parameter was made for the category it reached, and the project knows it is the one that carries the code.
    let param = &s.model.parameters["Article Code"];
    assert_eq!(param.categories, vec!["wall".to_string()]);
    assert_eq!(s.model.code_parameter.as_deref(), Some("Article Code"));
    assert!(s.model.library_in_step(), "the parameter is a library entry like any other");

    for id in walls_of(&s, "Muratura 250") {
        let e = s.model.elements.get(id).expect("the wall");
        assert_eq!(s.model.article_code(e), Some("B.02.100"));
    }
    for id in walls_of(&s, "Cartongesso 100") {
        let e = s.model.elements.get(id).expect("the wall");
        assert_eq!(s.model.article_code(e), None, "nothing the filter passed over was touched");
    }
}

#[test]
fn a_schedule_prices_what_carries_a_code_and_leaves_the_rest_empty() {
    let (mut s, _) = site();
    run(
        &mut s,
        "price_code",
        json!({"code": "B.02.100", "categories": ["walls"], "rules": [{"field": "Type", "op": "contains", "value": "Muratura"}]}),
    )
    .expect("the code is given");
    // A bill of quantities, as a user writes one: the quantity the price is for, the unit price, and the amount.
    let out = run(
        &mut s,
        "schedule",
        json!({"name": "Computo", "category": "walls", "fields": [
            "Article Code",
            "Price Description",
            "Price Unit",
            {"name": "Quantita", "formula": "Length * Height / 1000000", "kind": "number", "total": true},
            "Unit Price",
            {"name": "Importo", "formula": "Length * Height / 1000000 * Unit Price", "kind": "number", "total": true},
        ]}),
    )
    .expect("the schedule is made");

    let rows = out["rows"].as_array().expect("rows").clone();
    assert_eq!(rows.len(), 3);
    // 5 m by 3 m at 89,40 €/m² is 1341,00; 4 by 3 is 1072,80; the cartongesso wall carries no code, so no price.
    let priced: Vec<f64> = rows.iter().filter_map(|r| r[5].as_f64()).collect();
    assert_eq!(priced, vec![1341.0, 1072.8]);
    let empty = rows.iter().find(|r| r[0].is_null()).expect("the wall with no code");
    assert!(empty[4].is_null() && empty[5].is_null(), "no code, no price and no amount: a hole in the bill is visible");
    // The total is of what is priced, not of what is drawn.
    assert_eq!(out["totals"][5], 2413.8);
    let priced_row = rows.iter().find(|r| r[0] == "B.02.100").expect("a row with the code");
    assert_eq!(priced_row[1], "Muratura in blocchi di laterizio", "the wording comes from the price list");
    assert_eq!(priced_row[2], "m2");
}

#[test]
fn a_code_no_price_list_holds_is_refused_so_a_typo_is_no_silent_hole() {
    let (mut s, _) = site();
    let err = run(&mut s, "price_code", json!({"code": "B.02.1OO", "categories": ["walls"]})).expect_err("refused");
    assert!(err.contains("B.02.1OO") && err.contains("Prezzario"), "{err}");
    assert!(s.model.code_parameter.is_none(), "nothing was left behind");
    assert!(s.model.parameters.is_empty());
}

#[test]
fn the_price_fields_are_offered_only_where_a_price_reaches() {
    let (mut s, _) = site();
    let offered = |s: &Session| crate::schedule::fields(&s.model, "wall").into_iter().any(|(n, _)| n == "Unit Price");
    assert!(!offered(&s), "a price list alone prices nothing: nothing carries a code yet");
    run(&mut s, "price_code", json!({"code": "B.02.100", "categories": ["walls"]})).expect("the code is given");
    assert!(offered(&s));
    // A category the code's parameter is not for has no price fields, so no column that is always empty.
    assert!(!crate::schedule::fields(&s.model, "floor").into_iter().any(|(n, _)| n == "Unit Price"));
}

#[test]
fn clearing_a_code_takes_the_price_off_again() {
    let (mut s, _) = site();
    run(&mut s, "price_code", json!({"code": "B.02.100", "categories": ["walls"]})).expect("given");
    let out = run(&mut s, "price_code", json!({"clear": true, "categories": ["walls"]})).expect("cleared");
    assert_eq!(out["elements"], 3);
    let id = walls_of(&s, "Muratura 250")[0];
    assert_eq!(s.model.article_code(s.model.elements.get(id).expect("the wall")), None);
    assert_eq!(crate::schedule::field_text(&s.model, id, "Unit Price"), None);
}

#[test]
fn a_code_follows_a_view_filter_so_the_rules_are_made_where_they_already_are() {
    let (mut s, _) = site();
    run(
        &mut s,
        "filter",
        json!({"name": "Muri portanti", "categories": ["walls"], "rules": [{"field": "Thickness", "op": "greater-or-equal", "value": 200}]}),
    )
    .expect("the filter is made");
    let out = run(&mut s, "price_code", json!({"code": "B.02.100", "filter": "Muri portanti"})).expect("the code is given");
    assert_eq!(out["elements"], 2);
    let err = run(&mut s, "price_code", json!({"code": "B.02.100", "filter": "Muri esterni"})).expect_err("refused");
    assert!(err.contains("Muri portanti"), "it names the filters there are: {err}");
}

#[test]
fn the_code_parameter_is_the_users_to_name_and_a_parameter_that_holds_numbers_is_refused() {
    let (mut s, _) = site();
    run(&mut s, "price_code", json!({"code": "B.02.100", "categories": ["walls"], "parameter": "Codice articolo"})).expect("given");
    assert_eq!(s.model.code_parameter.as_deref(), Some("Codice articolo"));
    run(&mut s, "parameter", json!({"name": "Peso", "kind": "number", "categories": ["walls"]})).expect("a number parameter");
    let err = run(&mut s, "price_code", json!({"code": "B.02.100", "categories": ["walls"], "parameter": "Peso"})).expect_err("refused");
    assert!(err.contains("text"), "{err}");
}

/// The project of `site`, with the code `B.02.100` on its two `Muratura 250` walls, exported as IFC.
fn priced_ifc(unit: &str, currency: &str) -> String {
    let path = file("ifc.csv", &format!("Codice;Descrizione;UM;Prezzo\nB.02.100;Muratura in blocchi di laterizio;{unit};89,40\n"));
    let mut s = Session::new();
    s.script(
        "type {\"category\": \"wall\", \"name\": \"Muratura 250\", \"thickness\": 250}\n         wall {\"points\": [[0,0],[5000,0]], \"type\": \"Muratura 250\", \"height\": 3000}\n         wall {\"points\": [[0,4000],[4000,4000]], \"type\": \"Muratura 250\", \"height\": 3000}",
    )
    .expect("the walls are built");
    let mut p = json!({"name": "Prezzario", "path": path, "code": "Codice", "description": "Descrizione", "unit": "UM", "price": "Prezzo"});
    if !currency.is_empty() {
        p["currency"] = json!(currency);
    }
    run(&mut s, "price_list", p).expect("the price list comes in");
    run(&mut s, "price_code", json!({"code": "B.02.100", "categories": ["walls"]})).expect("the code is given");
    crate::ifc::export(&s.model)
}

#[test]
fn a_priced_project_leaves_as_a_bill_of_quantities() {
    let f = priced_ifc("m2", "");
    assert!(f.contains("IFCCOSTSCHEDULE(") && f.contains(".PRICEDBILLOFQUANTITIES."), "a schedule of costs");
    // The item is named by the price list's wording and identified by the article code.
    assert!(f.contains("IFCCOSTITEM(") && f.contains("'Muratura in blocchi di laterizio'") && f.contains("'B.02.100'"), "{f}");
    assert!(f.contains("IFCCOSTVALUE($,$,IFCMONETARYMEASURE(89.4),$,$,$,'Unit cost',$,$,$)"), "the unit price");
    // 5 m and 4 m of wall, 3 m high: 27 m² in all, and the money the file is written in.
    assert!(f.contains("IFCQUANTITYAREA('Area',$,$,27.,$)"), "{}", lines(&f, "IFCQUANTITY"));
    assert!(f.contains("IFCMONETARYUNIT('EUR')"), "a bill says what its money is");
    // The elements it prices are assigned to it, and it to the schedule.
    let assigns: Vec<&str> = f.lines().filter(|l| l.contains("IFCRELASSIGNSTOCONTROL(")).collect();
    assert_eq!(assigns.len(), 2, "{assigns:?}");
}

#[test]
fn the_quantity_an_item_carries_follows_the_price_lists_own_unit() {
    // 5 × 3 × 0,25 and 4 × 3 × 0,25 is 6,75 m³.
    let f = priced_ifc("m3", "");
    assert!(f.contains("IFCQUANTITYVOLUME('Volume',$,$,6.75,$)"), "{}", lines(&f, "IFCQUANTITY"));
    let f = priced_ifc("m", "");
    assert!(f.contains("IFCQUANTITYLENGTH('Length',$,$,9000.,$)"), "{}", lines(&f, "IFCQUANTITY"));
    // A unit no element can answer is a count, which is always true.
    let f = priced_ifc("cad", "");
    assert!(f.contains("IFCQUANTITYCOUNT('Count',$,$,2.,$)"), "{}", lines(&f, "IFCQUANTITY"));
}

#[test]
fn the_money_is_the_price_lists_and_a_currency_that_is_no_currency_is_refused() {
    let f = priced_ifc("m2", "chf");
    assert!(f.contains("IFCMONETARYUNIT('CHF')"), "upper case, as ISO 4217 writes it");
    let path = file("valuta.csv", "Codice;Prezzo\nA.1;10\n");
    let mut s = Session::new();
    let err = run(&mut s, "price_list", json!({"name": "L", "path": path, "code": "Codice", "price": "Prezzo", "currency": "euro"}))
        .expect_err("refused");
    assert!(err.contains("no currency"), "{err}");
    assert!(s.model.price_lists.is_empty(), "nothing was left behind");
}

#[test]
fn a_project_whose_elements_carry_no_code_writes_no_bill() {
    let (s, _) = site();
    let f = crate::ifc::export(&s.model);
    assert!(!f.contains("IFCCOSTSCHEDULE("), "a price list alone is not a bill: nothing carries a code");
    assert!(f.contains("IFCMONETARYUNIT("), "the money is still said, since the project has a price list");
}

#[test]
fn a_wall_says_its_volume_as_well_as_its_area_both_net_of_its_openings() {
    let mut s = Session::new();
    let made = run(&mut s, "wall", json!({"points": [[0, 0], [5000, 0]], "height": 3000})).expect("a wall");
    let wall = made["ids"][0].as_u64().expect("its id");
    run(&mut s, "type", json!({"category": "door", "name": "D 900", "width": 900, "height": 2100})).expect("a door type");
    run(&mut s, "door", json!({"wall": wall, "offset": 1000, "type": "D 900"})).expect("a door");
    let e = crate::commands::element_json(&s.model, wall);
    // A wall 200 mm thick: 15 m² gross is 3 m³, and 13,11 m² net is 2,62 m³.
    assert_eq!((e["volume_m3"].as_f64(), e["net_volume_m3"].as_f64()), (Some(3.0), Some(2.62)));
}

/// The lines of `f` that hold `what`, for an assertion to say what it found instead.
fn lines(f: &str, what: &str) -> String {
    f.lines().filter(|l| l.contains(what)).collect::<Vec<_>>().join("\n")
}

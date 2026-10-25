//! Manage > Price List: the file, the mapping picked from its own headings, and the list it reads, driven as a user drives it.

use super::*;
use egui_kittest::kittest::NodeT;

/// A price list on disk, written as an Italian published one is: a semicolon between the columns and a comma for the
/// decimal point. Its path.
fn price_file(name: &str, text: &str) -> String {
    let dir = std::env::temp_dir().join(format!("bimcraft-ui-price-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

const REGIONAL: &str = "ARTICOLO;DESCRIZIONE DEI LAVORI;U.M.;PREZZO UNITARIO\n\
                        A.01.010;Calcestruzzo C25/30 per fondazioni;m3;142,50\n\
                        A.01.020;Acciaio B450C in barre;kg;1,85\n\
                        B.02.100;Muratura in blocchi di laterizio;m2;89,40\n";

/// Types `text` into the field beside the label named `label`, which carries the same name as the field.
fn fill(h: &mut Harness<BimApp>, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get_by_role_and_label(Role::TextInput, label).type_text(text);
    h.run();
}

/// Picks `column` for the role named `label`. The file's headings are also shown in the rows being looked at, so the one
/// in the open list is the one that answers a click.
fn choose_column(h: &mut Harness<BimApp>, label: &str, column: &str) {
    h.get_by_role_and_label(Role::ComboBox, label).click_accesskit();
    h.run();
    let in_list = h.get_all_by_label(column).filter(|n| n.accesskit_node().role() != Role::Label);
    let node = in_list.min_by(|a, b| a.rect().top().total_cmp(&b.rect().top())).expect("the column in the open list");
    node.click_accesskit();
    h.run();
    if egui::Popup::is_any_open(&h.ctx) {
        h.get_by_role_and_label(Role::ComboBox, label).click_accesskit();
        h.run();
    }
}

#[test]
fn the_price_list_window_reads_a_file_with_the_columns_it_picks_from_its_own_headings() {
    let path = price_file("prezzario.csv", REGIONAL);
    let mut h = app(BOX);
    press(&mut h, "Manage");
    press(&mut h, "Price List");
    assert!(h.state().price_dialog.is_some());
    fill(&mut h, "File:", &path);
    press(&mut h, "Read Columns");
    // The separator was found, and every role is proposed on the column whose heading gives it away.
    let d = h.state().price_dialog.as_ref().expect("open").separator.clone();
    assert_eq!(d, ";");
    assert_eq!(combo_value(&h, "Code:"), "ARTICOLO");
    assert_eq!(combo_value(&h, "Description:"), "DESCRIZIONE DEI LAVORI");
    assert_eq!(combo_value(&h, "Unit:"), "U.M.");
    assert_eq!(combo_value(&h, "Price:"), "PREZZO UNITARIO");
    assert!(h.query_by_label("3 rows in the file").is_some());
    // Nothing is read without a name for the list.
    assert!(h.get_by_label("Import").accesskit_node().is_disabled(), "no name yet");
    fill(&mut h, "Name:", "Prezzario Regionale");
    press(&mut h, "Import");
    assert!(h.state().price_dialog.is_none(), "it closes");
    let list = &h.state().session.model.price_lists["Prezzario Regionale"];
    assert_eq!(list.items.len(), 3);
    let item = &list.items["A.01.010"];
    assert_eq!((item.unit.as_str(), item.price), ("m3", 142.5));
    assert!(item.description.starts_with("Calcestruzzo"), "{}", item.description);
    assert!(h.state().status.text.contains("3 items"), "{}", h.state().status.text);
}

#[test]
fn a_role_is_the_users_to_change_and_the_wording_may_be_left_at_none() {
    let path = price_file("altro.csv", "CODICE,VOCE,PREZZO\nX.1,Scavo a sezione obbligata,10.50\n");
    let mut h = app(BOX);
    shortcut(&mut h, "PL");
    fill(&mut h, "File:", &path);
    press(&mut h, "Read Columns");
    // A comma splits this one, and `VOCE` is no word this knows, so the wording is proposed at none.
    assert_eq!(h.state().price_dialog.as_ref().expect("open").separator, ",");
    assert_eq!(combo_value(&h, "Description:"), "(none)");
    choose_column(&mut h, "Description:", "VOCE");
    assert_eq!(combo_value(&h, "Description:"), "VOCE");
    fill(&mut h, "Name:", "Altra regione");
    press(&mut h, "Import");
    let list = &h.state().session.model.price_lists["Altra regione"];
    assert_eq!(list.items["X.1"].description, "Scavo a sezione obbligata");
    assert_eq!(list.mapping.description, "VOCE", "the mapping it was read with is kept with the list");
}

#[test]
fn a_file_that_cannot_be_read_is_said_inside_the_window_which_stays_open() {
    let mut h = app(BOX);
    shortcut(&mut h, "PL");
    fill(&mut h, "File:", "C:/nothing/here/prezzi.csv");
    press(&mut h, "Read Columns");
    let d = h.state().price_dialog.as_ref().expect("it stays open");
    assert!(d.error.contains("cannot read"), "{}", d.error);
    assert!(d.columns.is_empty());
    assert!(h.state().status.text.is_empty(), "the window says it, not the status bar");
    press(&mut h, "Close");
    assert!(h.state().price_dialog.is_none());
}

/// The window with `REGIONAL` already read into it, under the name `Prezzario`.
fn with_a_price_list() -> Harness<'static, BimApp> {
    let path = price_file("assegna.csv", "ARTICOLO;DESCRIZIONE;U.M.;PREZZO\nB.02.100;Muratura;m2;89,40\nB.02.200;Cartongesso;m2;34,10\n");
    let mut h = app(BOX);
    shortcut(&mut h, "PL");
    fill(&mut h, "File:", &path);
    press(&mut h, "Read Columns");
    fill(&mut h, "Name:", "Prezzario");
    press(&mut h, "Import");
    h
}

#[test]
fn the_window_writes_the_picked_items_code_on_the_selection() {
    let mut h = with_a_price_list();
    // A wall of the box, picked on the plan as a user picks one.
    click(&mut h, P2(4000.0, 0.0));
    let wall = *h.state().selection.first().expect("a wall is selected");
    shortcut(&mut h, "PL");
    assert!(h.query_by_label("1 selected").is_some());
    // Nothing is assigned until an item is picked.
    assert!(h.get_by_label("Assign to Selection").accesskit_node().is_disabled());
    press(&mut h, "B.02.100   Muratura   m2   89.40");
    press(&mut h, "Assign to Selection");
    let app = h.state();
    let e = app.session.model.elements.get(wall).expect("the wall");
    assert_eq!(app.session.model.article_code(e), Some("B.02.100"));
    assert_eq!(app.session.model.code_parameter.as_deref(), Some("Article Code"));
    assert!(app.status.text.contains("B.02.100"), "{}", app.status.text);
    assert!(app.price_dialog.is_some(), "it stays open: a project is priced item by item");

    // The schedule of its category now has the price fields, and the window says what the selection carries.
    let fields = bimcraft_core::schedule::fields(&h.state().session.model, "wall");
    assert!(fields.iter().any(|(n, _)| n == "Unit Price"), "{fields:?}");
    assert!(h.query_by_label("carrying B.02.100").is_some());

    // And takes it off again.
    press(&mut h, "Clear from Selection");
    let app = h.state();
    assert_eq!(app.session.model.article_code(app.session.model.elements.get(wall).expect("the wall")), None);
    assert!(app.status.text.contains("came off"), "{}", app.status.text);
}

#[test]
fn the_search_narrows_the_items_and_with_nothing_selected_nothing_can_be_assigned() {
    let mut h = with_a_price_list();
    shortcut(&mut h, "PL");
    assert!(h.query_by_label("0 selected").is_some());
    assert!(h.get_by_label("Assign to Selection").accesskit_node().is_disabled(), "nothing is selected");
    assert!(h.query_by_label("B.02.200   Cartongesso   m2   34.10").is_some());
    fill(&mut h, "Search:", "muratura");
    assert!(h.query_by_label("B.02.200   Cartongesso   m2   34.10").is_none(), "the search narrows the list");
    assert!(h.query_by_label("B.02.100   Muratura   m2   89.40").is_some());
    fill(&mut h, "Search:", "zzz");
    assert!(h.query_by_label("No item of that name.").is_some());
}

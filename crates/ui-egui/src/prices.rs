//! Price List: a published price list as the project's own data, so a bill of quantities has prices to work with. The
//! window picks a file, says which column holds the article code, the wording, the unit and the unit price, and reads the
//! rows under that mapping.
//!
//! The window is also where an element is given the article code that joins it to an item: pick the item, select the
//! elements, assign. The rules that pick out many elements at once live where they already are — a view filter, or
//! `price_code`'s own `categories` and `rules` — and the code is written on the elements either way, so a schedule
//! shows where each price came from.
//!
//! Nothing here reads a file: three commands do, and this module only shows what they say. `price_columns` gives back a
//! file's headings, its first rows and which column each role looks like, so the mapping is **picked from the file**
//! instead of typed; `price_list` then reads the rows under the mapping the user settled on. So a price list of any form
//! — another region's, another language's, a semicolon or a tab between its columns — needs no code written for it.

use egui::{Button, ComboBox, Context, Grid, Id as UiId, Modal, RichText, ScrollArea, TextEdit};
use serde_json::{Value, json};

use crate::BimApp;
use crate::lang::{t, tf};
use crate::style::pal;

/// The roles a price list's columns fill, as the window asks for them: the label, the parameter of `price_list` that
/// names the column, and whether the list cannot be read without it. A file that heads no column with the wording or the
/// unit of measure is read without them, so those two may be left at none.
const ROLES: [(&str, &str, bool); 4] =
    [("Code:", "code", true), ("Description:", "description", false), ("Unit:", "unit", false), ("Price:", "price", true)];

/// What the window offers as the character between columns, as the core takes it.
const SEPARATORS: [(&str, &str); 3] = [("Semicolon", ";"), ("Comma", ","), ("Tab", "\t")];

/// How many items the list under the search shows: enough to pick from, never a whole regional price list.
const SHOWN: usize = 200;

/// How much of a cell the rows being looked at show: enough to tell a column by, not enough to push the window wide.
const CELL: usize = 44;

/// The Price List window: the file, the mapping being settled on it, and what the core read of it.
pub struct PriceDialog {
    pub(crate) name: String,
    pub(crate) path: String,
    /// The one character between the columns, as the core found it or as the user chose it.
    pub(crate) separator: String,
    /// The file's headings, as it spells them: what each role is picked from.
    pub(crate) columns: Vec<String>,
    /// Its first rows, for looking at before the list is read.
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) row_count: usize,
    /// The column each role is on, in `ROLES`' order; empty where none.
    pub(crate) roles: [String; 4],
    /// What its prices are in, as ISO 4217 writes it: three letters, EUR unless the user says otherwise.
    pub(crate) currency: String,
    /// What is typed in the search over the project's items, and the item picked out of it.
    pub(crate) search: String,
    pub(crate) picked: String,
    /// Why the file could not be read, or the list not made: said inside the window, because a status box in the view's
    /// corner can lie over the buttons of a modal.
    pub(crate) error: String,
    /// Whether Assign to Selection gives the code beside the ones the elements carry, and how many times it is measured.
    pub(crate) add: bool,
    pub(crate) times: f64,
    /// The bill of quantities, as `bill_of_quantities` last gave it, while the window shows it instead of the list.
    pub(crate) bill: Option<Value>,
}

impl PriceDialog {
    fn new() -> Self {
        PriceDialog {
            name: String::new(),
            path: String::new(),
            separator: String::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            roles: Default::default(),
            currency: "EUR".to_string(),
            search: String::new(),
            picked: String::new(),
            error: String::new(),
            add: false,
            times: 1.0,
            bill: None,
        }
    }

    /// Whether there is enough to read the list: a name, and every role it cannot do without on a column of the file.
    fn ready(&self) -> bool {
        !self.name.trim().is_empty() && ROLES.iter().zip(&self.roles).all(|((_, _, needed), col)| !needed || self.columns.contains(col))
    }
}

/// A JSON array of text as the strings it holds.
fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().map(|c| c.as_str().unwrap_or_default().to_string()).collect()
}

/// A cell as the rows being looked at show it: cut to `CELL` characters, with an ellipsis where it was cut.
fn short(cell: &str) -> String {
    if cell.chars().count() <= CELL {
        return cell.to_string();
    }
    cell.chars().take(CELL - 1).chain(['…']).collect()
}

impl BimApp {
    /// Price List (PL): the window, with nothing read yet.
    pub(crate) fn open_prices(&mut self) {
        self.price_dialog = Some(PriceDialog::new());
    }

    /// Browse…: the file the user picks, read as soon as it comes back.
    fn price_browse(&mut self, ctx: &Context) {
        let dialog = rfd::AsyncFileDialog::new().add_filter(t("Price list"), &["csv", "txt", "tsv"]);
        self.ask_file(ctx, dialog, false, |app, path| {
            if let Some(d) = &mut app.price_dialog {
                d.path = path.to_string_lossy().into_owned();
                if d.name.trim().is_empty() {
                    d.name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                }
            }
            app.price_read();
        });
    }

    /// What the file holds, from `price_columns`: the columns each role is picked from, the rows to look at, and the
    /// mapping the core proposes, which the user may change. A separator already chosen is kept, since it is sent along.
    pub(crate) fn price_read(&mut self) {
        let Some(mut d) = self.price_dialog.take() else { return };
        d.error.clear();
        match self.session.execute("price_columns", &json!({"path": d.path.trim(), "separator": d.separator})) {
            Ok(r) => {
                d.separator = r["separator"].as_str().unwrap_or(";").to_string();
                d.columns = strings(&r["columns"]);
                d.rows = r["rows"].as_array().into_iter().flatten().map(strings).collect();
                d.row_count = r["row_count"].as_u64().unwrap_or(0) as usize;
                for ((_, role, _), col) in ROLES.iter().zip(&mut d.roles) {
                    *col = r["guess"][role].as_str().unwrap_or_default().to_string();
                }
            }
            Err(why) => {
                d.error = why;
                d.columns.clear();
                d.rows.clear();
                d.row_count = 0;
            }
        }
        self.price_dialog = Some(d);
    }

    pub(crate) fn price_window(&mut self, ctx: &Context) {
        if self.price_dialog.as_ref().is_some_and(|d| d.bill.is_some()) {
            self.bill_window(ctx);
            return;
        }
        let Some(mut d) = self.price_dialog.take() else { return };
        let (mut close, mut browse, mut reread, mut import) = (false, false, false, false);
        let (mut assign, mut take_off, mut show_bill) = (false, false, false);
        let mut use_list: Option<Option<String>> = None;
        let in_use = self.session.model.price_list_in_use.clone();
        let columns = d.columns.clone();
        let lists: Vec<(String, usize)> = self.session.model.price_lists.iter().map(|(name, l)| (name.clone(), l.items.len())).collect();
        let needle = d.search.trim().to_lowercase();
        let items: Vec<(String, String)> = self
            .session
            .model
            .price_lists
            .values()
            .flat_map(|l| l.items.iter())
            .filter(|(code, i)| {
                needle.is_empty() || code.to_lowercase().contains(&needle) || i.description.to_lowercase().contains(&needle)
            })
            .take(SHOWN)
            .map(|(code, i)| (code.clone(), format!("{code}   {}   {}   {:.2}", short(&i.description), i.unit, i.price)))
            .collect();
        let model = &self.session.model;
        let mut carried: Vec<&str> =
            self.selection.iter().filter_map(|id| model.elements.get(*id)).filter_map(|e| model.article_code(e)).collect();
        carried.sort_unstable();
        carried.dedup();
        let carried = carried.join(", ");
        let selected = self.selection.len();
        let r = Modal::new(UiId::new("price-list")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.label(RichText::new(t("Price List")).heading());
            ui.add_space(4.0);
            ui.label(
                RichText::new(t(
                    "A price list of this project's own: every row of the file becomes an item named by its article code, with its \
                     wording, its unit and its unit price.",
                ))
                .color(pal().muted),
            );
            ui.add_space(8.0);
            Grid::new("price-file").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
                let label = ui.label(t("File:"));
                ui.add_sized([380.0, 22.0], TextEdit::singleline(&mut d.path)).labelled_by(label.id);
                browse = ui.button(t("Browse…")).clicked();
                ui.end_row();
                let label = ui.label(t("Name:"));
                ui.add_sized([380.0, 22.0], TextEdit::singleline(&mut d.name)).labelled_by(label.id);
                ui.end_row();
                let label = ui.label(t("Currency:"));
                ui.add_sized([80.0, 22.0], TextEdit::singleline(&mut d.currency).char_limit(3)).labelled_by(label.id);
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                reread |= ui.add_enabled(!d.path.trim().is_empty(), Button::new(t("Read Columns"))).clicked();
                ui.add_space(8.0);
                ui.label(t("Separator:"));
                for (label, ch) in SEPARATORS {
                    if ui.selectable_label(d.separator == ch, t(label)).clicked() {
                        d.separator = ch.to_string();
                        reread = true;
                    }
                }
            });
            if !columns.is_empty() {
                ui.add_space(8.0);
                Grid::new("price-mapping").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                    for (i, (label, _, needed)) in ROLES.iter().enumerate() {
                        let name = ui.label(t(label));
                        let shown = if d.roles[i].is_empty() { t("(none)").to_string() } else { d.roles[i].clone() };
                        let c = ComboBox::from_id_salt(("price-role", i)).selected_text(shown).width(170.0).show_ui(ui, |ui| {
                            if !needed && ui.selectable_label(d.roles[i].is_empty(), t("(none)")).clicked() {
                                d.roles[i].clear();
                            }
                            for col in &columns {
                                if ui.selectable_label(&d.roles[i] == col, col).clicked() {
                                    d.roles[i] = col.clone();
                                }
                            }
                        });
                        c.response.labelled_by(name.id);
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new(tf("{} rows in the file", &[&d.row_count.to_string()])).color(pal().muted));
                ScrollArea::horizontal().max_height(150.0).auto_shrink([false, true]).show(ui, |ui| {
                    Grid::new("price-rows").striped(true).spacing([10.0, 2.0]).show(ui, |ui| {
                        for col in &columns {
                            ui.label(RichText::new(short(col)).strong());
                        }
                        ui.end_row();
                        for row in &d.rows {
                            for cell in row {
                                ui.label(short(cell));
                            }
                            ui.end_row();
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new(t("In this project")).strong());
            if lists.is_empty() {
                ui.label(RichText::new(t("No price list yet.")).color(pal().muted));
            }
            for (name, items) in &lists {
                ui.label(tf("{}: {} items", &[name, &items.to_string()]));
            }
            if !lists.is_empty() {
                ui.horizontal(|ui| {
                    let label = ui.label(t("Price list in use:"));
                    let shown = in_use.clone().unwrap_or_else(|| t("(every list)").to_string());
                    let c = ComboBox::from_id_salt("price-in-use").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                        if ui.selectable_label(in_use.is_none(), t("(every list)")).clicked() && in_use.is_some() {
                            use_list = Some(None);
                        }
                        for (name, _) in &lists {
                            if ui.selectable_label(in_use.as_deref() == Some(name.as_str()), name).clicked()
                                && in_use.as_deref() != Some(name.as_str())
                            {
                                use_list = Some(Some(name.clone()));
                            }
                        }
                    });
                    c.response.labelled_by(label.id);
                });
            }
            ui.add_space(10.0);
            ui.label(RichText::new(t("Article Code")).strong());
            ui.label(
                RichText::new(t("The code of the item that prices an element, written on the elements you have selected. A schedule of \
                     their category then has the wording, the unit and the unit price among its fields."))
                .color(pal().muted),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let label = ui.label(t("Search:"));
                ui.add_sized([220.0, 22.0], TextEdit::singleline(&mut d.search)).labelled_by(label.id);
                ui.label(RichText::new(tf("{} selected", &[&selected.to_string()])).color(pal().muted));
                if !carried.is_empty() {
                    ui.label(RichText::new(tf("carrying {}", &[&carried])).color(pal().muted));
                }
            });
            ui.add_space(4.0);
            ScrollArea::vertical().id_salt("price-items").max_height(130.0).auto_shrink([false, true]).show(ui, |ui| {
                if items.is_empty() {
                    // Nothing to pick from at all is not the same as nothing by that name, and the way out of the two is not
                    // the same either.
                    let why = if lists.is_empty() { "Read a price list first." } else { "No item of that name." };
                    ui.label(RichText::new(t(why)).color(pal().muted));
                }
                for (code, line) in &items {
                    if ui.selectable_label(&d.picked == code, line).clicked() {
                        d.picked = code.clone();
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.add, t("Beside the codes they carry"));
                ui.add_space(8.0);
                let label = ui.label(t("Times:"));
                ui.add(egui::DragValue::new(&mut d.times).range(0.5..=100.0).speed(0.5)).labelled_by(label.id);
            });
            ui.horizontal(|ui| {
                assign = ui.add_enabled(selected > 0 && !d.picked.is_empty(), Button::new(t("Assign to Selection"))).clicked();
                take_off = ui.add_enabled(selected > 0 && !carried.is_empty(), Button::new(t("Clear from Selection"))).clicked();
            });
            if !d.error.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&d.error).color(pal().error));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                import = ui.add_enabled(d.ready(), Button::new(t("Import"))).clicked();
                show_bill = ui.add_enabled(!lists.is_empty(), Button::new(t("Bill of Quantities…"))).clicked();
                close = ui.button(t("Close")).clicked();
            });
        });
        let closing = close || r.should_close();
        self.price_dialog = Some(d);
        if browse {
            self.price_browse(ctx);
            return;
        }
        if reread {
            self.price_read();
            return;
        }
        if import {
            self.price_import();
            return;
        }
        if assign || take_off {
            self.price_assign(take_off);
            return;
        }
        if let Some(name) = use_list {
            if self.run("price_list_use", json!({"name": name})).is_none() {
                let why = std::mem::take(&mut self.status.text);
                self.status = crate::Status::default();
                if let Some(d) = &mut self.price_dialog {
                    d.error = why;
                }
            }
            return;
        }
        if show_bill {
            self.bill_read();
            return;
        }
        if closing {
            self.price_dialog = None;
        }
    }

    /// Assign to Selection, and Clear from Selection: the picked item's code written on the elements selected, or taken
    /// off them. The window stays open, since a project is priced item by item.
    fn price_assign(&mut self, clear: bool) {
        let Some(d) = &self.price_dialog else { return };
        let code = d.picked.clone();
        if self.selection.is_empty() || (code.is_empty() && !clear) {
            return;
        }
        let ids: Vec<Value> = self.selection.iter().map(|id| json!(id)).collect();
        let params = match clear {
            true => json!({"clear": true, "ids": ids}),
            false => json!({"code": code, "ids": ids, "add": d.add, "times": d.times}),
        };
        match self.run("price_code", params) {
            Some(reply) => {
                let n = reply["elements"].as_u64().unwrap_or(0).to_string();
                let text = if clear { tf("The code came off {} elements.", &[&n]) } else { tf("{} elements carry {}.", &[&n, &code]) };
                self.status = crate::Status { text, error: false };
            }
            None => {
                let why = std::mem::take(&mut self.status.text);
                self.status = crate::Status::default();
                if let Some(d) = &mut self.price_dialog {
                    d.error = why;
                }
            }
        }
    }

    /// Import: the list read under the mapping on screen. What came in is said in the status bar and the window closes;
    /// what the core refused is said inside the window, which stays open on the mapping that was refused.
    fn price_import(&mut self) {
        let Some(d) = &self.price_dialog else { return };
        let mut params = json!({"name": d.name.trim(), "path": d.path.trim(), "separator": d.separator, "currency": d.currency.trim()});
        for ((_, role, _), col) in ROLES.iter().zip(&d.roles) {
            if !col.is_empty() {
                params[*role] = json!(col);
            }
        }
        match self.run("price_list", params) {
            Some(reply) => {
                let items = reply["items"].as_u64().unwrap_or(0);
                let left_out = reply["left_out"].as_array().map_or(0, Vec::len);
                let name = reply["name"].as_str().unwrap_or_default().to_string();
                let mut text = tf("{} items came in from {}.", &[&items.to_string(), &name]);
                if left_out > 0 {
                    text.push(' ');
                    text.push_str(&tf("{} rows were left out.", &[&left_out.to_string()]));
                }
                self.status = crate::Status { text, error: false };
                self.price_dialog = None;
            }
            None => {
                // `run` put the refusal in the status bar, where a modal's own box covers it: it is shown in the window.
                let why = std::mem::take(&mut self.status.text);
                self.status = crate::Status::default();
                if let Some(d) = &mut self.price_dialog {
                    d.error = why;
                }
            }
        }
    }
    /// The bill of quantities, read again from the project: on opening it and after anything that changes it.
    pub(crate) fn bill_read(&mut self) {
        let reply = self.session.execute("bill_of_quantities", &json!({}));
        if let Some(d) = &mut self.price_dialog {
            match reply {
                Ok(b) => d.bill = Some(b),
                Err(why) => d.error = why,
            }
        }
    }

    /// Bill of Quantities: every article code the project's elements carry, its quantity, price and amount, the total,
    /// and what could not be measured or priced; the bill and its specification saved as spreadsheets.
    fn bill_window(&mut self, ctx: &Context) {
        let Some(mut d) = self.price_dialog.take() else { return };
        let b = d.bill.clone().unwrap_or(Value::Null);
        let (mut back, mut export_bill, mut export_spec) = (false, false, false);
        let money = |v: &Value| v.as_f64().map_or_else(String::new, |x| format!("{x:.2}"));
        let currency = b["currency"].as_str().unwrap_or("EUR").to_string();
        let lines = b["lines"].as_array().cloned().unwrap_or_default();
        let r = Modal::new(UiId::new("bill-of-quantities")).show(ctx, |ui| {
            ui.set_width(760.0);
            ui.label(RichText::new(t("Bill of Quantities")).heading());
            ui.add_space(4.0);
            ui.label(
                RichText::new(t(
                    "Every article code the elements carry, measured by the unit of its price list: the area of a wall net of \
                     its openings, a volume, a length, or a count.",
                ))
                .color(pal().muted),
            );
            ui.add_space(8.0);
            if lines.is_empty() {
                ui.label(RichText::new(t("Nothing carries an article code yet.")).color(pal().muted));
            }
            ScrollArea::vertical().id_salt("bill-lines").max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                Grid::new("bill-grid").striped(true).num_columns(7).spacing([10.0, 3.0]).show(ui, |ui| {
                    for h in ["No.", "Code", "Description", "Unit", "Quantity", "Unit Price", "Amount"] {
                        ui.label(RichText::new(t(h)).strong());
                    }
                    ui.end_row();
                    for l in &lines {
                        ui.label(l["number"].to_string());
                        ui.label(l["code"].as_str().unwrap_or_default());
                        ui.label(short(l["description"].as_str().unwrap_or_default()));
                        ui.label(l["unit"].as_str().unwrap_or_default());
                        ui.label(money(&l["quantity"]));
                        ui.label(money(&l["price"]));
                        ui.label(money(&l["amount"]));
                        ui.end_row();
                    }
                });
            });
            ui.add_space(6.0);
            ui.label(RichText::new(tf("Total: {} {}", &[&money(&b["total"]), &currency])).strong());
            let named = |v: &Value| strings(v).join(", ");
            if !named(&b["unmeasured"]).is_empty() {
                ui.label(RichText::new(tf("Not measured, by their unit: {}", &[&named(&b["unmeasured"])])).color(pal().error));
            }
            if !named(&b["unpriced"]).is_empty() {
                ui.label(RichText::new(tf("In no price list in use: {}", &[&named(&b["unpriced"])])).color(pal().error));
            }
            if !d.error.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&d.error).color(pal().error));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                export_bill = ui.add_enabled(!lines.is_empty(), Button::new(t("Export Bill…"))).clicked();
                export_spec = ui.add_enabled(!lines.is_empty(), Button::new(t("Export Specification…"))).clicked();
                back = ui.button(t("Back")).clicked();
            });
        });
        if back || r.should_close() {
            d.bill = None;
        }
        self.price_dialog = Some(d);
        for (go, key, name) in [(export_bill, "path", "computo.csv"), (export_spec, "spec_path", "capitolato.csv")] {
            if !go {
                continue;
            }
            let dialog = rfd::AsyncFileDialog::new().add_filter("CSV", &["csv"]).set_file_name(name);
            self.ask_file(ctx, dialog, true, move |app, path| {
                let shown = path.to_string_lossy().into_owned();
                match app.session.execute("bill_of_quantities", &json!({key: shown})) {
                    Ok(_) => app.status = crate::Status { text: tf("Saved {}.", &[&shown]), error: false },
                    Err(why) => {
                        if let Some(d) = &mut app.price_dialog {
                            d.error = why;
                        }
                    }
                }
            });
        }
    }
}

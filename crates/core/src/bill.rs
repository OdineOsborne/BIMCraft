//! The bill of quantities: every article code the project's elements carry, the quantity each is for, its price and
//! its amount, and under each the elements it measures, as an estimate (Italian *computo metrico estimativo*) lists
//! them; and the specification, the wording of every item the bill uses (the descriptive part of a *capitolato*).
//!
//! An element carries its codes as the value of the project's article-code parameter ([`Model::article_code`]): one
//! code, or several separated by `;`, each of which may say how many times it is measured (`B.03.010 x2`, the
//! plaster on both faces of a wall). **The quantity follows the price list's own unit**, because only the list knows
//! what its price is for: square metres are the element's area (a wall's net of its openings), cubic metres its
//! volume, metres its length, a unit of pieces its count. A unit nothing of an element measures (`kg`, `h`) is not
//! guessed at: the line keeps no quantity and says so, and the same is true of a code no price list holds, which
//! keeps its quantity and no price.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::model::{Id, Model};

/// How many codes one element carries at most: more is a typing accident, not an estimate.
pub const MAX_CODES: usize = 16;
/// How many times one code may be measured on one element.
pub const MAX_TIMES: f64 = 1000.0;

/// What a unit of a price list measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Area,
    Volume,
    Length,
    Count,
    /// Nothing an element says: weight, time, a lump sum by hand.
    None,
}

/// What unit `unit`, as a price list writes it, is a measure of. A unit written out with its symbol after it in
/// brackets (`metro lineare (m)`, as the Ligurian list writes them) is its symbol. An area per centimetre (`m²/cm`, a
/// screed priced by its thickness) is an area, measured as many times as it is centimetres thick.
pub fn measure_of(unit: &str) -> Measure {
    let unit = match (unit.rfind('('), unit.trim_end().strip_suffix(')')) {
        (Some(open), Some(head)) if open < head.len() => &head[open + 1..],
        _ => unit,
    };
    let u: String = unit.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    let u = u.trim_end_matches('.');
    match u {
        "m2" | "mq" | "m\u{b2}" | "sqm" | "m^2" | "m2/cm" | "m\u{b2}/cm" | "mq/cm" => Measure::Area,
        "m3" | "mc" | "m\u{b3}" | "m^3" => Measure::Volume,
        "m" | "ml" | "lm" => Measure::Length,
        "cad" | "n" | "nr" | "no" | "num" | "numero" | "pz" | "pezzi" | "each" | "ea" | "pcs" | "un" | "und" | "unit" | "unita"
        | "unit\u{e0}" | "acorpo" | "corpo" | "ls" => Measure::Count,
        _ => Measure::None,
    }
}

/// The codes `value` holds and how many times each is measured: `A.1`, `A.1; B.2 x2`. A whole value a price list
/// holds as one code is that code, so a code that happens to hold a `;` is not cut in two.
pub fn codes(m: &Model, value: &str) -> Vec<(String, f64)> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if m.price_item(value).is_some() {
        return vec![(value.to_string(), 1.0)];
    }
    value
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .take(MAX_CODES)
        // A part a price list holds whole is that code, whatever letters it ends in.
        .map(|part| match m.price_item(part) {
            Some(_) => (part.to_string(), 1.0),
            None => times(part).unwrap_or_else(|| (part.to_string(), 1.0)),
        })
        .collect()
}

/// `CODE x2`, `CODE ×2`, `CODE * 1,5`: the code and the number after the last times sign, where what follows the sign
/// is a number and nothing else, and something comes before it.
pub(crate) fn times(part: &str) -> Option<(String, f64)> {
    let (at, sign) = part.char_indices().rev().find(|(_, c)| matches!(c, 'x' | 'X' | '\u{d7}' | '*'))?;
    let (head, tail) = (part[..at].trim(), part[at + sign.len_utf8()..].trim());
    let number = !tail.is_empty()
        && tail.starts_with(|c: char| c.is_ascii_digit())
        && tail.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',');
    let n = tail.replace(',', ".").parse::<f64>().ok().filter(|n| number && n.is_finite() && *n > 0.0 && *n <= MAX_TIMES)?;
    (!head.is_empty()).then(|| (head.to_string(), n))
}

/// One element measured under a line of the bill.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: Id,
    pub category: String,
    /// Its type, or a room's name: what tells it apart in a list.
    pub label: String,
    pub level: String,
    pub times: f64,
    /// What it measures in the line's unit, `times` included; `None` where the unit is no measure of it.
    pub quantity: Option<f64>,
}

/// A line of the bill: one article code.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub code: String,
    pub description: String,
    pub unit: String,
    /// `None` where no price list holds the code.
    pub price: Option<f64>,
    pub quantity: Option<f64>,
    pub amount: Option<f64>,
    pub items: Vec<Item>,
}

/// The whole bill.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bill {
    pub currency: String,
    pub lines: Vec<Line>,
    pub total: f64,
    /// Codes elements carry that no price list holds.
    pub unpriced: Vec<String>,
    /// Codes whose unit nothing of an element measures.
    pub unmeasured: Vec<String>,
}

fn round(x: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (x * f).round() / f
}

/// What element `j` (its [`crate::commands::element_json`]) measures in `measure`.
fn quantity(j: &Value, measure: Measure) -> Option<f64> {
    let num = |k: &str| j.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    match measure {
        Measure::Area => num("net_area_m2").or_else(|| num("area_m2")),
        Measure::Volume => num("net_volume_m3").or_else(|| num("volume_m3")),
        Measure::Length => num("length").map(|mm| mm / 1000.0),
        Measure::Count => Some(1.0),
        Measure::None => None,
    }
}

/// The bill of quantities of the project: a line for every code its elements carry, in the order of the codes.
pub fn bill(m: &Model) -> Bill {
    let index = crate::walls::Index::new(m);
    let mut by_code: BTreeMap<String, Vec<(Id, f64)>> = BTreeMap::new();
    for (id, e) in m.elements.iter() {
        let Some(value) = m.article_code(e) else { continue };
        for (code, times) in codes(m, value) {
            by_code.entry(code).or_default().push((*id, times));
        }
    }
    let mut out = Bill { currency: m.currency().to_string(), ..Bill::default() };
    for (code, elements) in by_code {
        let item = m.price_item(&code);
        let unit = item.map(|i| i.unit.trim().to_string()).unwrap_or_default();
        // A code no list holds is still counted, so an element priced by a code that has gone is not dropped.
        let measure = if item.is_some() { measure_of(&unit) } else { Measure::Count };
        let mut items = Vec::new();
        for (id, times) in elements {
            let j = crate::commands::element_json_indexed(m, id, Some(&index));
            let label = j.get("type").or_else(|| j.get("name")).and_then(Value::as_str).unwrap_or_default().to_string();
            let level = m.elements.get(id).and_then(|e| m.level_of(e)).and_then(|g| m.level(g)).map(|l| l.name.clone()).unwrap_or_default();
            let category = m.elements.get(id).map(|e| e.category().to_string()).unwrap_or_default();
            let quantity = quantity(&j, measure).map(|q| round(q * times, 4));
            items.push(Item { id, category, label, level, times, quantity });
        }
        let quantity = match measure {
            Measure::None => None,
            _ => Some(round(items.iter().filter_map(|i| i.quantity).sum::<f64>(), 2)),
        };
        let price = item.map(|i| i.price);
        let amount = match (quantity, price) {
            (Some(q), Some(p)) => Some(round(q * p, 2)),
            _ => None,
        };
        if item.is_none() {
            out.unpriced.push(code.clone());
        } else if measure == Measure::None {
            out.unmeasured.push(code.clone());
        }
        out.total += amount.unwrap_or(0.0);
        out.lines.push(Line {
            description: item.map(|i| i.description.trim().to_string()).unwrap_or_default(),
            code,
            unit,
            price,
            quantity,
            amount,
            items,
        });
    }
    out.total = round(out.total, 2);
    out
}

/// The bill as the command returns it.
pub fn to_json(b: &Bill) -> Value {
    json!({
        "currency": b.currency,
        "total": b.total,
        "unpriced": b.unpriced,
        "unmeasured": b.unmeasured,
        "lines": b.lines.iter().enumerate().map(|(n, l)| json!({
            "number": n + 1,
            "code": l.code,
            "description": l.description,
            "unit": l.unit,
            "quantity": l.quantity,
            "price": l.price,
            "amount": l.amount,
            "elements": l.items.iter().map(|i| json!({
                "id": i.id, "category": i.category, "label": i.label, "level": i.level, "times": i.times, "quantity": i.quantity,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// How a CSV of the bill is written for the project's language: a language that writes a decimal comma separates
/// columns with `;`, as its spreadsheets read them.
struct Csv {
    sep: char,
    comma: bool,
}

impl Csv {
    fn of(m: &Model) -> Csv {
        match m.language {
            crate::language::Language::English => Csv { sep: ',', comma: false },
            _ => Csv { sep: ';', comma: true },
        }
    }

    fn text(&self, s: &str) -> String {
        let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
        if s.contains([self.sep, '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
    }

    fn num(&self, x: Option<f64>, places: usize) -> String {
        match x {
            None => String::new(),
            Some(x) => {
                let s = format!("{x:.places$}");
                if self.comma { s.replace('.', ",") } else { s }
            }
        }
    }

    fn row(&self, cells: &[String]) -> String {
        let mut r = cells.join(&self.sep.to_string());
        r.push('\n');
        r
    }
}

/// The bill as a spreadsheet reads it: a heading, then each line with, under it, the elements it measures, and the
/// total at the end.
pub fn bill_csv(m: &Model, b: &Bill) -> String {
    let w = |e: &str| m.language.word(e).to_string();
    let c = Csv::of(m);
    let mut out = c.row(&[
        w("No."),
        w("Code"),
        w("Description"),
        w("Unit"),
        w("Quantity"),
        format!("{} ({})", w("Unit Price"), b.currency),
        format!("{} ({})", w("Amount"), b.currency),
    ]);
    for (n, l) in b.lines.iter().enumerate() {
        out.push_str(&c.row(&[
            (n + 1).to_string(),
            c.text(&l.code),
            c.text(&l.description),
            c.text(&l.unit),
            c.num(l.quantity, 2),
            c.num(l.price, 2),
            c.num(l.amount, 2),
        ]));
        for i in &l.items {
            let times = if i.times == 1.0 { String::new() } else { format!(" x{}", c.num(Some(i.times), 0)) };
            let what = format!(
                "  {} {} {}{}{}",
                w(&i.category),
                i.id,
                i.label,
                if i.level.is_empty() { String::new() } else { format!(", {}", i.level) },
                times
            );
            out.push_str(&c.row(&[
                String::new(),
                String::new(),
                c.text(&what),
                String::new(),
                c.num(i.quantity, 2),
                String::new(),
                String::new(),
            ]));
        }
    }
    out.push_str(&c.row(&[
        String::new(),
        String::new(),
        c.text(&w("Total")),
        String::new(),
        String::new(),
        String::new(),
        c.num(Some(b.total), 2),
    ]));
    out
}

/// The specification: every item the bill uses, in its order, with the price list's whole wording.
pub fn spec_csv(m: &Model, b: &Bill) -> String {
    let w = |e: &str| m.language.word(e).to_string();
    let c = Csv::of(m);
    let mut out = c.row(&[w("No."), w("Code"), w("Description"), w("Unit"), format!("{} ({})", w("Unit Price"), b.currency)]);
    for (n, l) in b.lines.iter().enumerate() {
        out.push_str(&c.row(&[(n + 1).to_string(), c.text(&l.code), c.text(&l.description), c.text(&l.unit), c.num(l.price, 2)]));
    }
    out
}

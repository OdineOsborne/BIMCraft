//! DXF: the Drawing Exchange Format, read from the specification Autodesk publishes. A survey, a consultant's plan, the
//! 2D a building is modelled over — what this reads is the geometry, layer by layer, in millimetres.
//!
//! It reads what a record says and no more. `LINE`, `LWPOLYLINE`, `POLYLINE` with its `VERTEX`, `ARC` and `CIRCLE`, each
//! on the layer it names (group code 8), come in; everything else is counted by its own name and listed, never guessed
//! at. A polyline's bulge (an arc between two of its points) is read as the straight line between them and said to
//! be, and one of more points than `MAX_VERTICES` is left out whole rather than cut short.
//! Blocks are not opened: an `INSERT` is listed with the rest, because what it stands for is a definition this does not
//! read yet.
//!
//! The file says its own unit in `$INSUNITS` (inches, feet, millimetres, centimetres, metres); where it says nothing,
//! or something else, its numbers are read as millimetres, which is what a CAD drawing of a building is usually in. A
//! caller who knows better gives `Options::scale`.
//!
//! Binary DXF and DWG are other formats and are refused by name: DWG is Autodesk's own and has no published
//! specification, so it is converted before it comes here.

use crate::geom::P2;

/// What a file may be before it is refused, bytes.
pub const MAX_BYTES: usize = 256 << 20;
/// The most shapes one drawing brings in.
pub const MAX_SHAPES: usize = 500_000;
/// The most points one polyline brings in.
pub const MAX_VERTICES: usize = 20_000;
/// How far a drawing's points may lie from its origin, mm.
pub const MAX_COORD: f64 = 1e9;
/// How many kinds of thing left out are named, the most of each first.
const NAMED: usize = 12;

/// How a file's numbers become millimetres.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Millimetres to one of the file's units; `None` is what the file says, else millimetres.
    pub scale: Option<f64>,
}

/// A piece of a drawing, in millimetres.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A straight line, from one end to the other.
    Line([P2; 2]),
    /// A polyline's points in order, and whether its last point joins its first.
    Path { points: Vec<P2>, closed: bool },
    /// A centre, a radius, and the angles it runs between, degrees counter-clockwise from east. A circle runs 0 to 360.
    Arc { at: P2, radius: f64, from: f64, to: f64 },
}

/// A layer of a drawing: the name the file gives it, and what is drawn on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub shapes: Vec<Shape>,
}

/// What a DXF file holds, once read: its layers in the order they were first drawn on, what it did not read, and the
/// box its geometry lies in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    pub layers: Vec<Layer>,
    /// What was left out, the most of a kind first: `312 TEXT`, `4 SPLINE`.
    pub left_out: Vec<String>,
    /// The corners of what came in, mm: south-west, north-east. None where nothing did.
    pub extent: Option<[P2; 2]>,
    /// Millimetres to one of the file's units, as the file said or as the caller did.
    pub scale: f64,
}

impl Drawing {
    /// How many shapes came in, over every layer.
    pub fn shapes(&self) -> usize {
        self.layers.iter().map(|l| l.shapes.len()).sum()
    }
}

/// `$INSUNITS` as millimetres to one unit: the values of the specification this knows. A file that says nothing, or
/// says something this does not know (microns, miles, a surveyor's foot), is read as millimetres and says nothing about
/// it, because a drawing of a building nearly always is.
fn insunits(code: i64) -> Option<f64> {
    Some(match code {
        1 => 25.4,
        2 => 304.8,
        4 => 1.0,
        5 => 10.0,
        6 => 1000.0,
        _ => return None,
    })
}

/// A pair of lines of a DXF file: its group code, and the value under it.
struct Pairs<'a> {
    lines: std::str::Lines<'a>,
}

impl<'a> Iterator for Pairs<'a> {
    type Item = (i64, &'a str);
    fn next(&mut self) -> Option<(i64, &'a str)> {
        loop {
            let code = self.lines.next()?.trim();
            let value = self.lines.next()?.trim_end_matches(['\r', '\n']);
            if let Ok(code) = code.parse::<i64>() {
                return Some((code, value.trim()));
            }
            // A line that is no group code is a file this does not understand; the next pair may still be one.
        }
    }
}

/// What one entity of the file holds, as its group codes are read.
#[derive(Default)]
struct Entity {
    layer: String,
    points: Vec<P2>,
    /// The second point of a line, under codes 11 and 21.
    second: Option<P2>,
    x: Option<f64>,
    y: Option<f64>,
    radius: Option<f64>,
    from: Option<f64>,
    to: Option<f64>,
    flags: i64,
    /// How many of its points were an arc this read straight.
    bulges: usize,
}

impl Entity {
    /// Keeps the point its x and y have made, where it has both.
    fn point(&mut self) {
        if let (Some(x), Some(y)) = (self.x.take(), self.y.take()) {
            self.points.push(P2(x, y));
        }
    }
}

/// The drawing `bytes` holds, or why it is not one.
pub fn read(bytes: &[u8], o: &Options) -> Result<Drawing, String> {
    if let Some(scale) = o.scale
        && !(scale.is_finite() && scale > 0.0)
    {
        return Err(format!("a unit of {scale} mm isn't one"));
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!("it's {} bytes; a DXF is read up to {MAX_BYTES}", bytes.len()));
    }
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err("this is a binary DXF, which BIMCraft doesn't read: save it as an ASCII DXF".into());
    }
    if bytes.starts_with(b"AC10") || bytes.starts_with(b"MC0.0") {
        return Err("this is a DWG, Autodesk's own format, which has no published specification: convert it to DXF first".into());
    }
    let text = String::from_utf8_lossy(bytes);
    let mut d = Drawing { scale: o.scale.unwrap_or(1.0), ..Drawing::default() };
    let mut layers: Vec<Layer> = Vec::new();
    let mut left_out: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut bulges = 0usize;
    let mut section = String::new();
    let mut header: Option<String> = None;
    let mut kind = String::new();
    let mut e = Entity::default();
    // A POLYLINE being read, while its VERTEX entities come in.
    let mut open: Option<Entity> = None;
    let mut capped = false;

    let mut pairs = Pairs { lines: text.lines() }.peekable();
    while let Some((code, value)) = pairs.next() {
        // The header's variables: the file's own unit.
        if section == "HEADER" {
            match code {
                9 => header = Some(value.to_string()),
                70 if header.as_deref() == Some("$INSUNITS") && o.scale.is_none() => {
                    if let Some(mm) = value.parse::<i64>().ok().and_then(insunits) {
                        d.scale = mm;
                    }
                }
                _ => {}
            }
        }
        if code != 0 {
            if section != "ENTITIES" {
                continue;
            }
            match code {
                8 => e.layer = value.to_string(),
                10 => {
                    e.point();
                    e.x = value.parse().ok();
                }
                20 => e.y = value.parse().ok(),
                11 => e.second = value.parse().ok().map(|x| P2(x, 0.0)),
                21 => {
                    if let (Some(P2(x, _)), Ok(y)) = (e.second, value.parse::<f64>()) {
                        e.second = Some(P2(x, y));
                    }
                }
                40 => e.radius = value.parse().ok(),
                42 => {
                    if value.parse::<f64>().is_ok_and(|b| b != 0.0) {
                        e.bulges += 1;
                    }
                }
                50 => e.from = value.parse().ok(),
                51 => e.to = value.parse().ok(),
                70 => e.flags = value.parse().unwrap_or(0),
                _ => {}
            }
            continue;
        }
        // A new entity, or the end of one: what was being read is finished first.
        e.point();
        bulges += e.bulges;
        // A POLYLINE is a header, and its points are the VERTEX entities that follow it, to its SEQEND. Anything else
        // holds its own points and is finished where it ends.
        let done: Option<(String, Shape)> = match kind.as_str() {
            "POLYLINE" => {
                open = Some(std::mem::take(&mut e));
                None
            }
            "VERTEX" => {
                if let Some(o) = &mut open {
                    o.points.append(&mut e.points);
                    o.bulges += e.bulges;
                }
                None
            }
            "SEQEND" => open.take().and_then(|mut o| {
                bulges += o.bulges;
                let layer = o.layer.clone();
                finish("POLYLINE", &mut o, d.scale).map(|shape| (layer, shape))
            }),
            other => finish(other, &mut e, d.scale).map(|shape| (e.layer.clone(), shape)),
        };
        match done {
            Some(_) if layers.iter().map(|l| l.shapes.len()).sum::<usize>() >= MAX_SHAPES => capped = true,
            Some((layer, shape)) => {
                let name = if layer.trim().is_empty() { "0" } else { layer.trim() };
                match layers.iter_mut().find(|l| l.name == name) {
                    Some(l) => l.shapes.push(shape),
                    None => layers.push(Layer { name: name.to_string(), shapes: vec![shape] }),
                }
            }
            // What it does not read: named by its own kind, once for each of them. A POLYLINE is left out where its
            // vertices made no path, and its VERTEX and SEQEND are parts of it, not kinds of their own.
            // The words that hold a file together are no kind of thing: a section opens while its name is still being
            // read, so its own marker would otherwise be counted among what was left out.
            None if !kind.is_empty()
                && !matches!(
                    kind.as_str(),
                    "VERTEX" | "SEQEND" | "POLYLINE" | "SECTION" | "ENDSEC" | "EOF" | "TABLE" | "ENDTAB" | "BLOCK" | "ENDBLK" | "CLASS"
                )
                && section == "ENTITIES" =>
            {
                *left_out.entry(kind.clone()).or_default() += 1;
            }
            None => {}
        }
        e = Entity::default();
        kind = value.to_string();
        match value {
            "SECTION" => section = String::new(),
            "ENDSEC" => section = String::new(),
            _ => {}
        }
        // The name of a section follows its `0/SECTION` under code 2.
        if value == "SECTION"
            && let Some(&(2, name)) = pairs.peek()
        {
            section = name.to_string();
        }
    }

    if capped {
        left_out.insert(format!("shapes past the {MAX_SHAPES} a drawing holds"), 1);
    }
    if bulges > 0 {
        left_out.insert("polyline arcs, read as the straight line between their points".into(), bulges);
    }
    let mut named: Vec<(String, usize)> = left_out.into_iter().collect();
    named.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    d.left_out = named.into_iter().take(NAMED).map(|(what, n)| format!("{n} {what}")).collect();
    d.extent = extent(&layers);
    d.layers = layers;
    Ok(d)
}

/// The shape an entity of `kind` makes, in millimetres, or none where it makes none: a kind this does not read, or one
/// whose numbers are not a shape (a line of one point, a radius that is no length, a polyline of nothing).
fn finish(kind: &str, e: &mut Entity, scale: f64) -> Option<Shape> {
    let mm = |p: P2| P2(p.0 * scale, p.1 * scale);
    let sane = |p: &P2| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= MAX_COORD && p.1.abs() <= MAX_COORD;
    match kind {
        "LINE" => {
            let (from, to) = (mm(*e.points.first()?), mm(e.second?));
            let line = [from, to];
            line.iter().all(sane).then_some(Shape::Line(line))
        }
        "LWPOLYLINE" | "POLYLINE" => {
            // One of more points than a path holds is left out with its kind, not cut short: half a drawing is worse
            // than none of it, and what was left out is said.
            if e.points.len() > MAX_VERTICES {
                return None;
            }
            let points: Vec<P2> = e.points.iter().map(|p| mm(*p)).collect();
            if points.len() < 2 || !points.iter().all(sane) {
                return None;
            }
            Some(Shape::Path { points, closed: e.flags & 1 == 1 })
        }
        "ARC" | "CIRCLE" => {
            let at = mm(*e.points.first()?);
            let radius = e.radius? * scale;
            if !sane(&at) || !radius.is_finite() || radius <= 0.0 || radius > MAX_COORD {
                return None;
            }
            let (from, to) = match kind {
                "CIRCLE" => (0.0, 360.0),
                _ => (e.from.unwrap_or(0.0), e.to.unwrap_or(360.0)),
            };
            (from.is_finite() && to.is_finite()).then_some(Shape::Arc { at, radius, from, to })
        }
        _ => None,
    }
}

/// The box `layers` lie in, mm.
fn extent(layers: &[Layer]) -> Option<[P2; 2]> {
    let mut box_: Option<[P2; 2]> = None;
    let mut grow = |p: P2| {
        box_ = Some(match box_ {
            None => [p, p],
            Some([lo, hi]) => [P2(lo.0.min(p.0), lo.1.min(p.1)), P2(hi.0.max(p.0), hi.1.max(p.1))],
        });
    };
    for shape in layers.iter().flat_map(|l| &l.shapes) {
        match shape {
            Shape::Line(line) => line.iter().for_each(|p| grow(*p)),
            Shape::Path { points, .. } => points.iter().for_each(|p| grow(*p)),
            Shape::Arc { at, radius, .. } => {
                grow(P2(at.0 - radius, at.1 - radius));
                grow(P2(at.0 + radius, at.1 + radius));
            }
        }
    }
    box_
}

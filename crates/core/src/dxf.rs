//! DXF: the Drawing Exchange Format, read from the specification Autodesk publishes. A survey, a consultant's plan, the
//! 2D a building is modelled over — what this reads is the geometry, layer by layer, in millimetres.
//!
//! It reads what a record says and no more. `LINE`, `LWPOLYLINE`, `POLYLINE` with its `VERTEX`, `ARC` and `CIRCLE`, each
//! on the layer it names (group code 8), come in, and `TEXT` and `MTEXT` as the words the drawing says, where it says
//! them; everything else is counted by its own name and listed, never guessed
//! at. A polyline's bulge (an arc between two of its points) is read as the straight line between them and said to
//! be, and one of more points than `MAX_VERTICES` is left out whole rather than cut short.
//! A block is opened where it is put: a survey draws its doors, its windows and its furniture as blocks, so an
//! `INSERT` brings in what the `BLOCK` of that name holds, from its base point, turned and scaled as the insert
//! says and as many times as its grid of copies says. What a block draws on layer `0` comes in on the layer the
//! block is put on, as a drawing has it; what it draws on a layer of its own keeps that one. A block holds blocks,
//! `MAX_NESTING` deep, past which a drawing is drawing itself; an insert of a block the file does not hold is
//! counted like anything else left out, and so is a round thing a block is scaled unevenly into an ellipse.
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
/// How many blocks deep a block may put another: past that a drawing is drawing itself.
pub const MAX_NESTING: usize = 8;
/// The most letters one word of a drawing brings in.
pub const MAX_WORD: usize = 256;
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
    /// What a survey writes on its drawing, where it writes it: the left of the line it stands on, how tall its
    /// capitals are, and how far it is turned, degrees counter-clockwise from east. It is what the drawing says and
    /// no line of the building, so a plan draws it and the walls are not read from it.
    Words { at: P2, text: String, height: f64, angle: f64 },
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
    /// The name a `BLOCK` gives itself and an `INSERT` names, under code 2.
    name: String,
    /// What a `TEXT` or an `MTEXT` says, under code 1, with the chunks a long `MTEXT` is cut into under code 3
    /// before it.
    text: String,
    /// What an `INSERT` scales a block by, under codes 41 and 42, and the grid of copies it stands for, under 70
    /// and 71 with their spacings under 44 and 45. A block put once at its own size says none of them.
    scale: (Option<f64>, Option<f64>),
    grid: (Option<i64>, Option<i64>),
    spacing: (Option<f64>, Option<f64>),
    /// What a `TEXT`'s point is the point of, under codes 72 and 73, and what an `MTEXT`'s is, under 71: a word is
    /// written from its left, from its middle or from its right, and the file says which.
    align: (Option<i64>, Option<i64>),
}

/// A block put into the drawing by an `INSERT`: which block, on what layer, where, turned and scaled how, and how
/// many copies of it stand across and up from there.
#[derive(Clone, Default)]
struct Put {
    name: String,
    layer: String,
    at: P2,
    scale: P2,
    angle: f64,
    grid: (i64, i64),
    spacing: P2,
}

/// What a block holds: a shape, on the layer it is drawn on, or another block put inside it.
enum Drawn {
    Shape(String, Shape),
    Put(Put),
}

/// Where the points of a block land in the drawing: where its own two axes point, and where its base point goes. A
/// block put inside a block turns and scales what is turned and scaled already, which is no turn and scale of its
/// own any more, so this is kept as the two axes and not as an angle.
#[derive(Clone, Copy)]
struct Placed {
    x: P2,
    y: P2,
    at: P2,
}

impl Placed {
    /// The drawing's own axes: where a block stands that is put nowhere, which is where the drawing's own entities
    /// stand.
    const SAME: Placed = Placed { x: P2(1.0, 0.0), y: P2(0.0, 1.0), at: P2(0.0, 0.0) };

    /// Where the `col`th copy across and the `row`th up of a block put so lands.
    fn of(put: &Put, col: f64, row: f64) -> Placed {
        let (sin, cos) = put.angle.to_radians().sin_cos();
        let (x, y) = (put.spacing.0 * col, put.spacing.1 * row);
        Placed {
            x: P2(cos * put.scale.0, sin * put.scale.0),
            y: P2(-sin * put.scale.1, cos * put.scale.1),
            at: P2(put.at.0 + x * cos - y * sin, put.at.1 + x * sin + y * cos),
        }
    }

    /// Where a point of the block lands.
    fn point(&self, p: P2) -> P2 {
        P2(self.at.0 + self.x.0 * p.0 + self.y.0 * p.1, self.at.1 + self.x.1 * p.0 + self.y.1 * p.1)
    }

    /// This placing, done inside `outer`: the block put inside the block put there.
    fn inside(&self, outer: &Placed) -> Placed {
        let axis = |a: P2| P2(outer.x.0 * a.0 + outer.y.0 * a.1, outer.x.1 * a.0 + outer.y.1 * a.1);
        Placed { x: axis(self.x), y: axis(self.y), at: outer.point(self.at) }
    }

    /// What a radius is multiplied by where a round thing stays round — the two axes as long as each other and
    /// square to one another — and none where it would come out an ellipse, which is no shape this draws.
    fn round(&self) -> Option<f64> {
        let (x, y) = (self.x.0.hypot(self.x.1), self.y.0.hypot(self.y.1));
        let square = self.x.0 * self.y.0 + self.x.1 * self.y.1;
        (x > 0.0 && (x - y).abs() <= x * 1e-9 && square.abs() <= x * y * 1e-9).then_some(x)
    }

    /// Where the angle zero of a circle lands, in degrees, and whether the placing turns it over, which runs an arc
    /// the other way round.
    fn turn(&self) -> (f64, bool) {
        (self.x.1.atan2(self.x.0).to_degrees(), self.x.0 * self.y.1 - self.x.1 * self.y.0 < 0.0)
    }
}

impl Entity {
    /// Keeps the point its x and y have made, where it has both.
    fn point(&mut self) {
        if let (Some(x), Some(y)) = (self.x.take(), self.y.take()) {
            self.points.push(P2(x, y));
        }
    }
}

/// Whether `bytes` are a DWG: `AC10` and two digits, the version code Autodesk writes first. A DWG is not read here —
/// it has no published specification — but it is told apart, so that what can convert one is asked, and what cannot
/// says what the file is.
pub fn is_dwg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"AC10") && bytes.get(4..6).is_some_and(|v| v.iter().all(u8::is_ascii_digit))
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
    // The blocks the file holds, and the one being read: its name, its base point, and what has been drawn in it so
    // far. A block is kept as it is drawn and opened where it is put, because a block may put a block the file
    // holds after it, and because the same block is put in many places.
    let mut blocks: std::collections::HashMap<String, Vec<Drawn>> = std::collections::HashMap::new();
    let mut block: Option<(String, P2, Vec<Drawn>)> = None;

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
            // What a block is drawn out of is read as the drawing's own entities are, because that is what it is.
            if section != "ENTITIES" && section != "BLOCKS" {
                continue;
            }
            // A code says different things of different entities: 42 is a polyline's bulge and an insert's scale
            // across, 70 a polyline's closing and an insert's count across.
            let put = kind == "INSERT";
            match code {
                1 | 3 => e.text.push_str(value),
                2 => e.name = value.to_string(),
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
                41 if put => e.scale.0 = value.parse().ok(),
                42 if put => e.scale.1 = value.parse().ok(),
                42 => {
                    if value.parse::<f64>().is_ok_and(|b| b != 0.0) {
                        e.bulges += 1;
                    }
                }
                44 if put => e.spacing.0 = value.parse().ok(),
                45 if put => e.spacing.1 = value.parse().ok(),
                50 => e.from = value.parse().ok(),
                51 => e.to = value.parse().ok(),
                70 if put => e.grid.0 = value.parse().ok(),
                71 if put => e.grid.1 = value.parse().ok(),
                70 => e.flags = value.parse().unwrap_or(0),
                71 | 72 => e.align.0 = value.parse().ok(),
                73 => e.align.1 = value.parse().ok(),
                _ => {}
            }
            continue;
        }
        // A new entity, or the end of one: what was being read is finished first.
        e.point();
        bulges += e.bulges;
        // A POLYLINE is a header, and its points are the VERTEX entities that follow it, to its SEQEND. Anything else
        // holds its own points and is finished where it ends.
        let mut put: Option<Put> = None;
        let done: Option<(String, Shape)> = match kind.as_str() {
            "POLYLINE" => {
                open = Some(std::mem::take(&mut e));
                None
            }
            // A block opens where it is named and holds what is drawn until its end. Its base point is the point
            // of it that lands where it is put, so what it holds is kept from there.
            "BLOCK" => {
                let at = e.points.first().copied().unwrap_or(P2(0.0, 0.0));
                block = Some((e.name.clone(), P2(at.0 * d.scale, at.1 * d.scale), Vec::new()));
                None
            }
            "ENDBLK" => {
                if let Some((name, _, held)) = block.take() {
                    blocks.insert(name, held);
                }
                None
            }
            "INSERT" => {
                let at = e.points.first().copied().unwrap_or(P2(0.0, 0.0));
                let across = e.grid.0.unwrap_or(1).clamp(0, MAX_SHAPES as i64);
                put = Some(Put {
                    name: e.name.clone(),
                    layer: e.layer.clone(),
                    at: P2(at.0 * d.scale, at.1 * d.scale),
                    scale: P2(e.scale.0.unwrap_or(1.0), e.scale.1.unwrap_or(1.0)),
                    angle: e.from.unwrap_or(0.0),
                    grid: (across, e.grid.1.unwrap_or(1).clamp(0, MAX_SHAPES as i64)),
                    spacing: P2(e.spacing.0.unwrap_or(0.0) * d.scale, e.spacing.1.unwrap_or(0.0) * d.scale),
                });
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
        // What was read is drawn into the block being read, where there is one, and into the drawing otherwise.
        match (done, put) {
            (_, Some(put)) => match &mut block {
                Some((_, _, held)) => held.push(Drawn::Put(put)),
                None => open_up(&put, &blocks, &mut layers, &mut left_out, &mut capped),
            },
            (Some((layer, shape)), None) => match &mut block {
                Some((_, base, held)) => held.push(Drawn::Shape(layer, from_base(shape, *base))),
                None if layers.iter().map(|l| l.shapes.len()).sum::<usize>() >= MAX_SHAPES => capped = true,
                None => draw(&mut layers, &layer, shape),
            },
            // What it does not read: named by its own kind, once for each of them. A POLYLINE is left out where its
            // vertices made no path, and its VERTEX and SEQEND are parts of it, not kinds of their own.
            // The words that hold a file together are no kind of thing: a section opens while its name is still being
            // read, so its own marker would otherwise be counted among what was left out.
            (None, None)
                if !kind.is_empty()
                    && !matches!(
                        kind.as_str(),
                        "VERTEX" | "SEQEND" | "POLYLINE" | "SECTION" | "ENDSEC" | "EOF" | "TABLE" | "ENDTAB" | "BLOCK" | "ENDBLK" | "CLASS"
                    )
                    && (section == "ENTITIES" || block.is_some()) =>
            {
                *left_out.entry(kind.clone()).or_default() += 1;
            }
            (None, None) => {}
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

/// `shape` drawn on `layer`, which the drawing holds already or gains here, so that the layers stand in the order
/// they were first drawn on.
fn draw(layers: &mut Vec<Layer>, layer: &str, shape: Shape) {
    let name = if layer.trim().is_empty() { "0" } else { layer.trim() };
    match layers.iter_mut().find(|l| l.name == name) {
        Some(l) => l.shapes.push(shape),
        None => layers.push(Layer { name: name.to_string(), shapes: vec![shape] }),
    }
}

/// A shape of a block as the block holds it: measured from its base point, which is the point of it that lands
/// where the block is put.
fn from_base(shape: Shape, base: P2) -> Shape {
    let off = |p: P2| P2(p.0 - base.0, p.1 - base.1);
    match shape {
        Shape::Line([a, b]) => Shape::Line([off(a), off(b)]),
        Shape::Path { points, closed } => Shape::Path { points: points.into_iter().map(off).collect(), closed },
        Shape::Arc { at, radius, from, to } => Shape::Arc { at: off(at), radius, from, to },
        Shape::Words { at, text, height, angle } => Shape::Words { at: off(at), text, height, angle },
    }
}

/// A block put into the drawing: every copy of it the grid says, each opened where it stands.
fn open_up(
    put: &Put,
    blocks: &std::collections::HashMap<String, Vec<Drawn>>,
    layers: &mut Vec<Layer>,
    left_out: &mut std::collections::BTreeMap<String, usize>,
    capped: &mut bool,
) {
    // A block holding nothing draws nothing, so nothing would ever stop a grid of a million rows of a million of
    // it, or eight of those inside one another. How many times a block is opened at all is counted and bounded by
    // what a drawing holds, which is the one bound that holds whatever a file asks for.
    let mut tries = 0usize;
    put_in(put, "0", &Placed::SAME, blocks, 0, layers, left_out, capped, &mut tries);
}

/// The same, inside a block put there already: on the layer that one came in on, where that one put it.
#[allow(clippy::too_many_arguments)]
fn put_in(
    put: &Put,
    on: &str,
    outer: &Placed,
    blocks: &std::collections::HashMap<String, Vec<Drawn>>,
    depth: usize,
    layers: &mut Vec<Layer>,
    left_out: &mut std::collections::BTreeMap<String, usize>,
    capped: &mut bool,
    tries: &mut usize,
) {
    let on = match put.layer.trim() {
        "" | "0" => on,
        own => own,
    };
    for row in 0..put.grid.1.max(1) {
        for col in 0..put.grid.0.max(1) {
            if *capped {
                return;
            }
            let placed = Placed::of(put, col as f64, row as f64).inside(outer);
            opened(&put.name, on, &placed, blocks, depth, layers, left_out, capped, tries);
        }
    }
}

/// A block opened: every shape it holds, drawn on the layer it comes in on, where the placing puts it. A block
/// holds blocks, so this goes round again — [`MAX_NESTING`] deep, past which a drawing is drawing itself.
#[allow(clippy::too_many_arguments)]
fn opened(
    name: &str,
    on: &str,
    placed: &Placed,
    blocks: &std::collections::HashMap<String, Vec<Drawn>>,
    depth: usize,
    layers: &mut Vec<Layer>,
    left_out: &mut std::collections::BTreeMap<String, usize>,
    capped: &mut bool,
    tries: &mut usize,
) {
    *tries += 1;
    if *tries > MAX_SHAPES {
        *capped = true;
        return;
    }
    if depth >= MAX_NESTING {
        *left_out.entry(format!("blocks put more than {MAX_NESTING} deep, left unopened")).or_default() += 1;
        return;
    }
    let Some(held) = blocks.get(name) else {
        *left_out.entry("INSERT of a block the file doesn't hold".into()).or_default() += 1;
        return;
    };
    for drawn in held {
        if *capped {
            return;
        }
        match drawn {
            // What a block draws on layer 0 takes the layer the block is put on, as a drawing has it; what it
            // draws on a layer of its own keeps that one.
            Drawn::Shape(layer, shape) => {
                let on = match layer.trim() {
                    "" | "0" => on,
                    own => own,
                };
                match placing(shape, placed) {
                    Ok(_) if layers.iter().map(|l| l.shapes.len()).sum::<usize>() >= MAX_SHAPES => *capped = true,
                    Ok(shape) => draw(layers, on, shape),
                    Err(why) => *left_out.entry(why.into()).or_default() += 1,
                }
            }
            Drawn::Put(inner) => put_in(inner, on, placed, blocks, depth + 1, layers, left_out, capped, tries),
        }
    }
}

/// A shape of a block where the placing puts it, or why it is not drawn there.
fn placing(shape: &Shape, placed: &Placed) -> Result<Shape, &'static str> {
    let sane = |p: &P2| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= MAX_COORD && p.1.abs() <= MAX_COORD;
    let far = "things in a block put farther from the origin than a drawing reaches";
    match shape {
        Shape::Line([a, b]) => {
            let line = [placed.point(*a), placed.point(*b)];
            line.iter().all(sane).then_some(Shape::Line(line)).ok_or(far)
        }
        Shape::Path { points, closed } => {
            let points: Vec<P2> = points.iter().map(|p| placed.point(*p)).collect();
            points.iter().all(sane).then_some(Shape::Path { points, closed: *closed }).ok_or(far)
        }
        Shape::Arc { at, radius, from, to } => {
            // Scaled evenly a circle is a circle, and scaled unevenly it is an ellipse, which this does not draw.
            let radius = radius * placed.round().ok_or("round things in a block scaled into ellipses")?;
            let at = placed.point(*at);
            let (turn, over) = placed.turn();
            // A whole circle stays a whole circle, whatever it is turned by. An arc turns with the block, and a
            // block turned over runs it the other way round, so its two ends change places.
            let (from, to) = match ((to - from).abs() >= 360.0, over) {
                (true, _) => (0.0, 360.0),
                (false, false) => (from + turn, to + turn),
                (false, true) => (turn - to, turn - from),
            };
            let round = (from / 360.0).floor() * 360.0;
            let (from, to) = (from - round, to - round);
            let ok = sane(&at) && radius.is_finite() && radius > 0.0 && radius <= MAX_COORD && from.is_finite() && to.is_finite();
            ok.then_some(Shape::Arc { at, radius, from, to }).ok_or(far)
        }
        Shape::Words { at, text, height, angle } => {
            // A word is as tall as the block makes it and turned with it, however unevenly the block is scaled:
            // what it says is the whole of what it is for, and a word is no shape to come out wrong.
            let (turn, _) = placed.turn();
            let height = height * placed.y.0.hypot(placed.y.1);
            let at = placed.point(*at);
            let ok = sane(&at) && height.is_finite() && height > 0.0 && height <= MAX_COORD;
            ok.then_some(Shape::Words { at, text: text.clone(), height, angle: angle + turn }).ok_or(far)
        }
    }
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
        "TEXT" | "MTEXT" => {
            // A word is what it says, so one that says nothing is none; one longer than a drawing writes is left out
            // whole, as a polyline of too many points is.
            let text = match kind {
                "MTEXT" => plain(&e.text),
                _ => e.text.trim().to_string(),
            };
            if text.is_empty() || text.chars().count() > MAX_WORD {
                return None;
            }
            let height = e.radius? * scale;
            let angle = e.from.unwrap_or(0.0);
            // Where the file says the word stands is the point it is written from, which it says under its
            // justification: a `TEXT` not written from the left of its line gives that point under codes 11 and 21.
            let (across, up) = match kind {
                "MTEXT" => {
                    let at = e.align.0.unwrap_or(1).clamp(1, 9) - 1;
                    (at % 3, 2 - at / 3)
                }
                _ => (
                    match e.align.0.unwrap_or(0) {
                        1 | 4 => 1,
                        2 => 2,
                        _ => 0,
                    },
                    match e.align.1.unwrap_or(0) {
                        2 => 1,
                        3 => 2,
                        _ => 0,
                    },
                ),
            };
            let point = match (kind, across + up > 0, e.second) {
                ("TEXT", true, Some(p)) => p,
                _ => *e.points.first()?,
            };
            let at = mm(point);
            // Back to the left of the line it stands on, which is where a word is drawn from. How wide it is is our
            // own lettering's width, which is the lettering it will be drawn in.
            let wide = f64::from(crate::font::width(&text, height as f32));
            let (dx, dy) = (
                match across {
                    1 => -wide / 2.0,
                    2 => -wide,
                    _ => 0.0,
                },
                match up {
                    1 => -height / 2.0,
                    2 => -height,
                    _ => 0.0,
                },
            );
            let (sin, cos) = angle.to_radians().sin_cos();
            let at = P2(at.0 + dx * cos - dy * sin, at.1 + dx * sin + dy * cos);
            if !sane(&at) || !height.is_finite() || height <= 0.0 || height > MAX_COORD || !angle.is_finite() {
                return None;
            }
            Some(Shape::Words { at, text, height, angle })
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

/// What an `MTEXT` says, without the formatting it says it in: a survey writes a room's name in a paragraph with a
/// font, a colour and a height of its own in it, and none of that is the name. What is left of the paragraph is one
/// line, because a drawing's words are read for what they say and not for how they are set.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text.chars();
    while let Some(c) = rest.next() {
        match c {
            // The braces a paragraph groups its runs with say nothing themselves.
            '{' | '}' => {}
            '\\' => match rest.next() {
                // A new paragraph, and the spaces a drawing writes with a mark of their own.
                Some('P' | 'X' | '~') => out.push(' '),
                // What a run is set in, which runs to a semicolon.
                Some(c) if "fFHCcAQWTpS".contains(c) => {
                    for c in rest.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                }
                // Underlining, overlining and the like, which is the one letter and nothing more.
                Some(c) if "LlOoKkNn".contains(c) => {}
                // A brace or a backslash of its own, written with one before it.
                Some(c) => out.push(c),
                None => {}
            },
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<&str>>().join(" ")
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
            Shape::Words { at, text, height, angle } => {
                let wide = f64::from(crate::font::width(text, *height as f32));
                let (sin, cos) = angle.to_radians().sin_cos();
                for (x, y) in [(0.0, 0.0), (wide, 0.0), (wide, *height), (0.0, *height)] {
                    grow(P2(at.0 + x * cos - y * sin, at.1 + x * sin + y * cos));
                }
            }
        }
    }
    box_
}

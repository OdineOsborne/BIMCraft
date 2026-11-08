//! DXF: the Drawing Exchange Format, read from the specification Autodesk publishes. A survey, a consultant's plan, the
//! 2D a building is modelled over — what this reads is the geometry, layer by layer, in millimetres.
//!
//! It reads what a record says and no more. `LINE`, `LWPOLYLINE`, `POLYLINE` with its `VERTEX`, `ARC` and `CIRCLE`, each
//! on the layer it names (group code 8), come in, and `TEXT` and `MTEXT` as the words the drawing says, where it says
//! them; `DIMENSION` as what the drawing measures and the figure it writes over it, and `HATCH` as the areas it
//! fills and the pattern it fills them with. Everything else is counted by its own name and listed, never guessed
//! at. A polyline's bulge is the arc it draws between two of its points and is read as that arc ([`bow`]), which
//! splits the polyline into the pieces it is drawn in; one that is no arc is left as the line between them and said
//! to be, and a polyline of more points than `MAX_VERTICES` is left out whole rather than cut short.
//! What a drawing says of itself is drawn and is no line of the building: a word, a measurement and a hatch are
//! drawn with the plan and left out of the lines traced and snapped to, or a survey's lettering would come in as
//! walls, a dimension's two lines as a wall's two faces, and a hatch's boundary as the faces it runs along.
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
/// How many of a drawing's figures are tried as the unit the rest are held against ([`measured`]).
const TRIED: usize = 64;
/// The most group codes one hatch is read out of.
const MAX_RAW: usize = 200_000;
/// The most areas one hatch fills, and the most families of lines its pattern is drawn with.
const MAX_RINGS: usize = 256;
const MAX_RULINGS: usize = 64;
/// The most dashes one family of a pattern's lines is drawn with.
const MAX_DASHES: usize = 64;

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
    /// What a survey measures on its drawing: the two points it measures between, the line it writes the measurement
    /// along — those two points brought onto it, so that how long it is is what the drawing measured — and the figure
    /// it writes there, with how tall that figure is written (zero where the drawing does not say).
    ///
    /// The drawing's own word on a distance, and no line of the building: a plan draws it and the walls are not read
    /// from it, or the two lines a dimension takes its measurement out along would be read as a wall.
    Measure { from: P2, to: P2, line: [P2; 2], text: String, height: f64 },
    /// What a drawing fills an area with to say what it is of: the name its pattern goes by, whether it is filled
    /// solid, the areas it fills, and the families of parallel lines that pattern is drawn with, which the file
    /// gives whether or not this knows the name.
    ///
    /// A fill is drawn and no line of the building: a hatch runs along the faces of the wall it fills, and read as
    /// geometry it would draw every one of them twice.
    Fill { pattern: String, solid: bool, rings: Vec<Vec<P2>>, rulings: Vec<Ruling> },
}

/// One family of a hatch pattern's lines: how far it is turned, degrees counter-clockwise from east, how far apart
/// its lines are across themselves, how far along itself each line stands from the one before it, and the dashes it
/// is drawn with, mm — a length to draw and a negative one to leave, over and over, and none for a line with no
/// break in it. A pattern of dashes drawn without them is a web of lines and not the stipple a drawing shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Ruling {
    pub angle: f64,
    pub gap: f64,
    pub shift: f64,
    pub dashes: Vec<f64>,
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

/// What a drawing's measurements say, held against what it draws: see [`measured`].
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// How many measurements were read whose figure is a number.
    pub read: usize,
    /// What one of those figures' units is, in millimetres: 10 where a drawing prints centimetres.
    pub unit: f64,
    /// The scale the drawing's own measurements say it is drawn at, millimetres to one of the file's units, where
    /// the units it was read with cannot be right. Empty where they can, which is nearly always.
    pub scale_says: Vec<f64>,
    /// The measurements whose figure the drawing is not drawn to, the furthest off first.
    pub disagree: Vec<Disagreed>,
}

/// A measurement that says a length the drawing is not drawn to.
#[derive(Clone, Debug, PartialEq)]
pub struct Disagreed {
    /// What it writes.
    pub says: String,
    /// How far apart the two points it was taken between are, mm.
    pub drawn: f64,
    /// The middle of the line it is written along: where to go and look.
    pub at: P2,
}

/// Every measurement a drawing writes, held against the geometry it is written over.
///
/// A drawing prints a length in units and to a precision of its own and says neither in the dimension. But what
/// nearly every figure on a drawing is, is the length it stands over printed in those units, so the unit is what most
/// of its figures agree on: of the units its own figures make, held one at a time against all the others, the one the
/// most of them come out right under. The middle one of them would do on most drawings and not on all, because the
/// figures that are not the length under them are the whole of what this is for, and a survey written over in half
/// its places has no middle worth taking.
///
/// Held against that unit, a figure disagrees where it is further from the length under it than the rounding it is
/// printed to explains — half of its own last digit. That is the survey saying something its drawing does not: a
/// surveyor who measured 8.70 and drew 8.80 writes `870` over the figure, and a wall traced from that drawing comes
/// out 100 mm too long. None where the drawing writes no figure that is a number.
///
/// That unit is also what catches a file whose units are not what it says they are, which a survey drawn in metres
/// and saved as millimetres is: read as it says, the building comes out a hand's width across and nothing traces.
/// A length is written in millimetres, centimetres or metres, both the one a drawing is drawn in and the one it
/// prints; so a drawing whose figures come out in none of them was read at the wrong scale, and the scales at which
/// both come out a unit somebody writes in are what its own measurements say it is drawn at ([`Measured::scale_says`]).
/// Nothing is said of a drawing whose figures already come out in one of the three, which is nearly every drawing.
pub fn measured(d: &Drawing) -> Option<Measured> {
    let read: Vec<(f64, &str, f64, P2)> = d
        .layers
        .iter()
        .flat_map(|l| &l.shapes)
        .filter_map(|shape| {
            let Shape::Measure { line, text, .. } = shape else { return None };
            let (says, drawn) = (figure(text)?, line[0].dist(line[1]));
            let at = P2((line[0].0 + line[1].0) / 2.0, (line[0].1 + line[1].1) / 2.0);
            (drawn > 0.0).then_some((says, text.as_str(), drawn, at))
        })
        .collect();
    let mut units: Vec<f64> = read.iter().map(|(says, _, drawn, _)| says / drawn).collect();
    units.sort_by(f64::total_cmp);
    // The unit the most figures agree on, each held against it to the rounding it is printed to, and the closest
    // agreement where two units are agreed on by as many. Tried from across the whole spread of them and not at the
    // middle of it, because a drawing a surveyor has written over in half its places has no middle worth taking.
    let tries = units.len().clamp(1, TRIED);
    let agreement = |per_mm: f64| {
        read.iter()
            .filter(|(says, text, drawn, _)| (says - drawn * per_mm).abs() <= rounding(text))
            .fold((0usize, 0.0f64), |(n, off), (says, _, drawn, _)| (n + 1, off + (says - drawn * per_mm).abs()))
    };
    let per_mm = (0..tries)
        .filter_map(|i| units.get(i * units.len() / tries).copied())
        .filter(|per_mm| per_mm.is_finite() && *per_mm > 0.0)
        .max_by(|a, b| {
            let (an, aoff) = agreement(*a);
            let (bn, boff) = agreement(*b);
            an.cmp(&bn).then(boff.total_cmp(&aoff))
        })?;
    let off = |says: f64, drawn: f64| says - drawn * per_mm;
    let mut disagree: Vec<Disagreed> = read
        .iter()
        .filter(|(says, text, drawn, _)| off(*says, *drawn).abs() > rounding(text))
        .map(|(_, text, drawn, at)| Disagreed { says: (*text).to_string(), drawn: *drawn, at: *at })
        .collect();
    disagree.sort_by(|a, b| {
        let far = |x: &Disagreed| figure(&x.says).map_or(0.0, |says| off(says, x.drawn).abs());
        far(b).total_cmp(&far(a))
    });
    Some(Measured { read: read.len(), unit: 1.0 / per_mm, scale_says: scale_says(d.scale, 1.0 / per_mm), disagree })
}

/// The units a length is written in, millimetres to one: millimetres, centimetres and metres. What a drawing is
/// drawn in is one of them and what it prints is one of them, and a decimetre is neither.
const WRITTEN_IN: [f64; 3] = [1.0, 10.0, 1000.0];

/// How near a unit a measured one must come to be it: a figure printed to the nearest centimetre over a wall a
/// metre long is a fortieth out on its own, and the three units are ten times apart, so nothing is ever two.
const NEARLY: f64 = 0.05;

/// The scales a drawing's figures allow, where the one it was read at is not among them: for each unit it could be
/// printing in, the scale that would make it so, kept where that scale is itself a unit a drawing is drawn in.
fn scale_says(read_at: f64, unit: f64) -> Vec<f64> {
    let nearly = |a: f64, b: f64| (a - b).abs() <= b * NEARLY;
    if !unit.is_finite() || unit <= 0.0 || WRITTEN_IN.iter().any(|written| nearly(unit, *written)) {
        return Vec::new();
    }
    WRITTEN_IN
        .iter()
        .filter_map(|prints| {
            let scale = read_at * prints / unit;
            WRITTEN_IN.iter().copied().find(|drawn_in| nearly(scale, *drawn_in))
        })
        .collect()
}

/// The number a figure is, where it is one and nothing else. A drawing made in Italy prints `2,80` where one made
/// elsewhere prints `2.80`, and both say the same length; a figure with a word in it (`var.`, `8.70 m`, `~350`) is
/// what the drawing says and no number, and nothing is worked out from it.
fn figure(text: &str) -> Option<f64> {
    let text = text.trim();
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let ok = match text.strip_prefix('-').unwrap_or(text).split(['.', ',']).collect::<Vec<&str>>().as_slice() {
        [whole] => digits(whole),
        [whole, part] => digits(whole) && digits(part),
        _ => false,
    };
    ok.then(|| text.replace(',', ".").parse().ok())?
}

/// How far a figure may stand from the length under it and still be that length: half of its own last digit, which is
/// the rounding a drawing prints with, and a hair for the arithmetic.
fn rounding(text: &str) -> f64 {
    let places = match text.trim().rsplit_once(['.', ',']) {
        Some((_, part)) => part.chars().count() as i32,
        None => 0,
    };
    0.5 * 10f64.powi(-places) * (1.0 + 1e-9)
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
    /// The bend of each of its points that has one: which point it is, counted from the first, and the bulge the
    /// file writes there, which is an arc from that point to the next.
    bulge: Vec<(usize, f64)>,
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
    /// The two points a `DIMENSION` measures between, under codes 13 and 23 and 14 and 24.
    between: (Option<P2>, Option<P2>),
    /// Every group code of a `HATCH`, in the order the file gives them. A hatch's boundary is loops of edges and its
    /// pattern is families of lines, and neither can be read out of the codes gathered one by one: the same code
    /// says a different thing at each turn, and what it says is the turn it comes at.
    raw: Vec<(i64, String)>,
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
    // The bends of a polyline that are no arc, left as the line between their points.
    let mut straight = 0usize;
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
            // A dimension says in 3 what it is drawn in and in 42 what it measures, where the one is a chunk of what
            // an `MTEXT` says and the other an arc of a polyline read straight.
            let dim = kind == "DIMENSION";
            if kind == "HATCH" && e.raw.len() < MAX_RAW {
                e.raw.push((code, value.to_string()));
            }
            match code {
                1 => e.text.push_str(value),
                3 if !dim => e.text.push_str(value),
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
                13 => e.between.0 = value.parse().ok().map(|x| P2(x, 0.0)),
                23 => {
                    if let (Some(P2(x, _)), Ok(y)) = (e.between.0, value.parse::<f64>()) {
                        e.between.0 = Some(P2(x, y));
                    }
                }
                14 => e.between.1 = value.parse().ok().map(|x| P2(x, 0.0)),
                24 => {
                    if let (Some(P2(x, _)), Ok(y)) = (e.between.1, value.parse::<f64>()) {
                        e.between.1 = Some(P2(x, y));
                    }
                }
                40 => e.radius = value.parse().ok(),
                41 if put => e.scale.0 = value.parse().ok(),
                42 if put => e.scale.1 = value.parse().ok(),
                42 if dim => {}
                42 => {
                    // The point it bends from is the one being read, which is not in its points until it is done.
                    if let Ok(b) = value.parse::<f64>()
                        && b != 0.0
                        && e.bulge.len() < MAX_VERTICES
                    {
                        e.bulge.push((e.points.len(), b));
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
        // A POLYLINE is a header, and its points are the VERTEX entities that follow it, to its SEQEND. Anything else
        // holds its own points and is finished where it ends.
        let mut put: Option<Put> = None;
        let done: Vec<(String, Shape)> = match kind.as_str() {
            "POLYLINE" => {
                open = Some(std::mem::take(&mut e));
                Vec::new()
            }
            // A block opens where it is named and holds what is drawn until its end. Its base point is the point
            // of it that lands where it is put, so what it holds is kept from there.
            "BLOCK" => {
                let at = e.points.first().copied().unwrap_or(P2(0.0, 0.0));
                block = Some((e.name.clone(), P2(at.0 * d.scale, at.1 * d.scale), Vec::new()));
                Vec::new()
            }
            "ENDBLK" => {
                if let Some((name, _, held)) = block.take() {
                    blocks.insert(name, held);
                }
                Vec::new()
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
                Vec::new()
            }
            "VERTEX" => {
                if let Some(o) = &mut open {
                    // Its bend is its own point's, which stands where this vertex lands among the polyline's.
                    let at = o.points.len();
                    o.bulge.extend(e.bulge.iter().map(|(_, b)| (at, *b)));
                    o.points.append(&mut e.points);
                }
                Vec::new()
            }
            // What the drawing measures. The figure it prints is read out of the picture a CAD drew it as, which is a
            // block of its own, so this is here and not in `finish`, where the blocks are not to hand.
            "DIMENSION" => measurement(&e, d.scale, &blocks).map(|shape| (e.layer.clone(), shape)).into_iter().collect(),
            // What the drawing fills an area with. Its boundary and its pattern are read out of its group codes in
            // the order they come, so this is here with the dimension and not in `finish`.
            "HATCH" => filled(&e, d.scale, &mut left_out).map(|shape| (e.layer.clone(), shape)).into_iter().collect(),
            "SEQEND" => match open.take() {
                None => Vec::new(),
                Some(mut o) => {
                    let layer = o.layer.clone();
                    pieces("POLYLINE", &mut o, d.scale, &mut straight).into_iter().map(|shape| (layer.clone(), shape)).collect()
                }
            },
            other => pieces(other, &mut e, d.scale, &mut straight).into_iter().map(|shape| (e.layer.clone(), shape)).collect(),
        };
        // What was read is drawn into the block being read, where there is one, and into the drawing otherwise.
        let read = !done.is_empty();
        for (layer, shape) in done {
            match &mut block {
                Some((_, base, held)) => held.push(Drawn::Shape(layer, from_base(shape, *base))),
                None if layers.iter().map(|l| l.shapes.len()).sum::<usize>() >= MAX_SHAPES => capped = true,
                None => draw(&mut layers, &layer, shape),
            }
        }
        match (read, put) {
            (_, Some(put)) => match &mut block {
                Some((_, _, held)) => held.push(Drawn::Put(put)),
                None => open_up(&put, &blocks, &mut layers, &mut left_out, &mut capped),
            },
            (true, None) => {}
            // What it does not read: named by its own kind, once for each of them. A POLYLINE is left out where its
            // vertices made no path, and its VERTEX and SEQEND are parts of it, not kinds of their own.
            // The words that hold a file together are no kind of thing: a section opens while its name is still being
            // read, so its own marker would otherwise be counted among what was left out.
            (false, None)
                if !kind.is_empty()
                    && !matches!(
                        kind.as_str(),
                        "VERTEX" | "SEQEND" | "POLYLINE" | "SECTION" | "ENDSEC" | "EOF" | "TABLE" | "ENDTAB" | "BLOCK" | "ENDBLK" | "CLASS"
                    )
                    && (section == "ENTITIES" || block.is_some()) =>
            {
                // A dimension is counted by what it measures, because the kinds of it this does not read measure
                // something else entirely and a count of them says so.
                let what = match kind.as_str() {
                    "DIMENSION" => unread_dimension(&e),
                    _ => kind.clone(),
                };
                *left_out.entry(what).or_default() += 1;
            }
            (false, None) => {}
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
    if straight > 0 {
        left_out.insert("polyline bends that are no arc, read as the straight line between their points".into(), straight);
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
        Shape::Measure { from, to, line, text, height } => {
            Shape::Measure { from: off(from), to: off(to), line: [off(line[0]), off(line[1])], text, height }
        }
        Shape::Fill { pattern, solid, rings, rulings } => {
            Shape::Fill { pattern, solid, rings: rings.into_iter().map(|r| r.into_iter().map(off).collect()).collect(), rulings }
        }
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
        Shape::Measure { from, to, line, text, height } => {
            // A measurement goes where the block puts it, and its figure is as tall as the block makes it: what it
            // says of a distance is the whole of what it is for.
            let height = height * placed.y.0.hypot(placed.y.1);
            let (from, to) = (placed.point(*from), placed.point(*to));
            let line = [placed.point(line[0]), placed.point(line[1])];
            let ok = [from, to, line[0], line[1]].iter().all(sane) && height.is_finite() && height <= MAX_COORD;
            ok.then_some(Shape::Measure { from, to, line, text: text.clone(), height }).ok_or(far)
        }
        Shape::Fill { pattern, solid, rings, rulings } => {
            // A pattern goes with the block: its lines turn with it and spread as it is scaled. Scaled unevenly
            // they would be a family of lines no longer evenly apart, which is no pattern this draws.
            let spread = placed.round().ok_or("hatches in a block scaled into ellipses")?;
            let (turn, over) = placed.turn();
            let rings: Vec<Vec<P2>> = rings.iter().map(|r| r.iter().map(|p| placed.point(*p)).collect()).collect();
            let rulings = rulings
                .iter()
                .map(|r| Ruling {
                    angle: if over { turn - r.angle } else { r.angle + turn },
                    gap: r.gap * spread,
                    shift: r.shift * spread,
                    dashes: r.dashes.iter().map(|d| d * spread).collect(),
                })
                .collect();
            match rings.iter().flatten().all(sane) {
                true => Ok(Shape::Fill { pattern: pattern.clone(), solid: *solid, rings, rulings }),
                false => Err(far),
            }
        }
    }
}

/// What a `DIMENSION` measures, in millimetres, or none where it is a kind of dimension that measures no length
/// between two points.
///
/// A dimension is written along a line of its own, and that line is not always the line between the two points it
/// measures: a surveyor measuring the width of a room between two corners that are not level with one another writes
/// the width. A linear dimension says how far that line is turned (code 50); an aligned one runs along its two
/// points. So the two points are brought onto it, and how long it is between them is what the drawing measured.
///
/// What it says there is its own word on it (code 1), where a survey writes one over the figure, and otherwise the
/// figure the picture a CAD drew it as prints. The figure is read where it was written and never worked out again from
/// what was measured: a drawing prints in units and to a precision of its own — this one prints centimetres — and
/// keeps them in its dimension style, not in the dimension.
fn measurement(e: &Entity, scale: f64, blocks: &std::collections::HashMap<String, Vec<Drawn>>) -> Option<Shape> {
    let mm = |p: P2| P2(p.0 * scale, p.1 * scale);
    let sane = |p: &P2| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= MAX_COORD && p.1.abs() <= MAX_COORD;
    let (from, to) = (mm(e.between.0?), mm(e.between.1?));
    let at = mm(*e.points.first()?);
    let along = match e.flags & 7 {
        0 => {
            let turn = e.from.unwrap_or(0.0);
            let (sin, cos) = turn.to_radians().sin_cos();
            turn.is_finite().then_some(P2(cos, sin))?
        }
        1 => {
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);
            let len = dx.hypot(dy);
            (len > 0.0).then_some(P2(dx / len, dy / len))?
        }
        _ => return None,
    };
    let onto = |p: P2| {
        let out = (p.0 - at.0) * along.0 + (p.1 - at.1) * along.1;
        P2(at.0 + along.0 * out, at.1 + along.1 * out)
    };
    let line = [onto(from), onto(to)];
    // The figure, as the picture prints it. A word written over the figure may have the figure in it, written `<>`.
    let printed = blocks.get(&e.name).into_iter().flatten().find_map(|drawn| match drawn {
        Drawn::Shape(_, Shape::Words { text, height, .. }) => Some((text.clone(), *height)),
        _ => None,
    });
    let (figure, height) = printed.unwrap_or_default();
    let over = e.text.trim();
    let text = match over.is_empty() {
        true => figure,
        false => over.replace("<>", &figure),
    };
    let ok = [from, to, line[0], line[1]].iter().all(sane) && height.is_finite() && (0.0..=MAX_COORD).contains(&height);
    (ok && text.chars().count() <= MAX_WORD).then_some(Shape::Measure { from, to, line, text, height })
}

/// What a `HATCH` fills, in millimetres, or none where it fills nothing this can read.
///
/// A hatch is read out of its group codes in the order the specification gives them, because the same code says a
/// different thing at each turn of it: after the number of boundary loops (91) each loop says what kind it is (92)
/// and then either the points of a polyline or one edge after another, and after the boundary the pattern says how
/// many families of lines it is drawn with (78) and then each family's angle and offset.
///
/// Only the loops the file marks as a boundary of their own — external, or outermost — are filled. A loop that is
/// neither is an island left out of the hatch, and filling one would fill what the drawing left empty. A loop drawn
/// with a spline or an ellipse is not read and is counted: half a boundary is a ring that is not the shape it fills.
fn filled(e: &Entity, scale: f64, left_out: &mut std::collections::BTreeMap<String, usize>) -> Option<Shape> {
    let raw = &e.raw;
    let at = |i: usize| raw.get(i).map(|(code, _)| *code);
    let num = |i: usize| raw.get(i).and_then(|(_, v)| v.trim().parse::<f64>().ok());
    let int = |i: usize| raw.get(i).and_then(|(_, v)| v.trim().parse::<i64>().ok()).unwrap_or(0);
    let find = |from: usize, code: i64| (from..raw.len()).find(|i| at(*i) == Some(code));
    // A point under `x` and the code after it: a hatch writes its y under the x, always.
    let point = |i: usize, x: i64, y: i64| match (num(i), at(i + 1) == Some(y) && at(i) == Some(x)) {
        (Some(px), true) => num(i + 1).map(|py| P2(px * scale, py * scale)),
        _ => None,
    };
    let mut rings: Vec<Vec<P2>> = Vec::new();
    let mut unread = 0usize;
    let mut i = find(0, 91)?;
    let loops = int(i).clamp(0, MAX_RINGS as i64) as usize;
    i += 1;
    for _ in 0..loops {
        let Some(f) = find(i, 92) else { break };
        let (kind, mut ring, mut whole) = (int(f), Vec::new(), true);
        i = f + 1;
        let Some(n) = find(i, 93) else { break };
        let count = int(n).clamp(0, MAX_VERTICES as i64) as usize;
        i = n + 1;
        // A polyline boundary is its points; any other is one edge after another, each saying its own kind first.
        if kind & 2 != 0 {
            for _ in 0..count {
                let Some(x) = find(i, 10) else { break };
                i = x + 2;
                ring.extend(point(x, 10, 20));
            }
        } else {
            for _ in 0..count {
                let Some(t) = find(i, 72) else { break };
                i = t + 1;
                match int(t) {
                    1 => {
                        let (from, to) = (find(i, 10).and_then(|x| point(x, 10, 20)), find(i, 11).and_then(|x| point(x, 11, 21)));
                        match (from, to) {
                            (Some(from), Some(to)) => {
                                ring.push(from);
                                ring.push(to);
                            }
                            _ => whole = false,
                        }
                        i = find(i, 11).map_or(i, |x| x + 2);
                    }
                    2 => {
                        let centre = find(i, 10).and_then(|x| point(x, 10, 20));
                        let radius = find(i, 40).and_then(num).map(|r| r * scale);
                        let (from, to) = (find(i, 50).map(|x| num(x).unwrap_or(0.0)), find(i, 51).map(|x| num(x).unwrap_or(360.0)));
                        match (centre, radius, from, to) {
                            (Some(centre), Some(radius), Some(from), Some(to)) if radius > 0.0 => {
                                ring.extend(crate::geom::along_arc(centre, radius, from, to));
                            }
                            _ => whole = false,
                        }
                        i = find(i, 51).map_or(i, |x| x + 1);
                    }
                    // An ellipse or a spline: a shape this does not draw, so the ring it is part of is not the
                    // shape the drawing fills and is left out whole.
                    _ => whole = false,
                }
            }
        }
        let sane = |p: &P2| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= MAX_COORD && p.1.abs() <= MAX_COORD;
        ring.dedup();
        match whole && ring.len() >= 3 && ring.iter().all(sane) {
            true if kind & 1 != 0 || kind & 16 != 0 => rings.push(ring),
            true => {}
            false => unread += 1,
        }
    }
    if unread > 0 {
        *left_out.entry("hatch boundaries drawn with a spline or an ellipse".into()).or_default() += unread;
    }
    // The families of lines the pattern is drawn with. The offset (45 and 46) is the way one line of a family stands
    // from the next, turned as the family itself is, so how far apart they are across themselves is the part of it
    // that lies across the line — the rest is how far the dashes along one are shifted from those along the next.
    let mut rulings = Vec::new();
    if let Some(n) = find(0, 78) {
        let mut i = n + 1;
        for _ in 0..int(n).clamp(0, MAX_RULINGS as i64) {
            let Some(a) = find(i, 53) else { break };
            let angle = num(a).unwrap_or(0.0);
            let Some(o) = find(a, 45) else { break };
            i = o + 2;
            let (Some(across), Some(up)) = (num(o), num(o + 1).filter(|_| at(o + 1) == Some(46))) else { continue };
            let (sin, cos) = angle.to_radians().sin_cos();
            let gap = (cos * up - sin * across).abs() * scale;
            let shift = (cos * across + sin * up) * scale;
            // The dashes the lines of this family are drawn with, under 49, as many as 79 says.
            let dashes: Vec<f64> = match find(o, 79) {
                Some(n) => (0..int(n).clamp(0, MAX_DASHES as i64))
                    .filter_map(|k| find(n + 1 + k as usize, 49).filter(|d| *d == n + 1 + k as usize).and_then(num))
                    .map(|d| d * scale)
                    .collect(),
                None => Vec::new(),
            };
            if angle.is_finite() && gap.is_finite() && gap > 0.0 && shift.is_finite() && dashes.iter().all(|d| d.is_finite()) {
                rulings.push(Ruling { angle, gap, shift, dashes });
            }
        }
    }
    let solid = e.flags & 1 != 0;
    (!rings.is_empty()).then(|| Shape::Fill { pattern: e.name.clone(), solid, rings, rulings })
}

/// What a dimension this does not read measures, to be counted by: a count of them says what they were, where a count
/// of `DIMENSION` would say only that a drawing has some.
fn unread_dimension(e: &Entity) -> String {
    match e.flags & 7 {
        2 | 5 => "DIMENSION of an angle",
        3 => "DIMENSION of a diameter",
        4 => "DIMENSION of a radius",
        6 => "DIMENSION of a coordinate",
        _ => "DIMENSION",
    }
    .to_string()
}

/// What an entity of `kind` is drawn as, in millimetres: one shape, or, where a polyline bends, the several that a
/// bend splits it into. `straight` counts the bends that are no arc and are left as the line between their points.
fn pieces(kind: &str, e: &mut Entity, scale: f64, straight: &mut usize) -> Vec<Shape> {
    match kind {
        "LWPOLYLINE" | "POLYLINE" if !e.bulge.is_empty() => bent(e, scale, straight),
        _ => finish(kind, e, scale).into_iter().collect(),
    }
}

/// The pieces a polyline that bends is drawn in: an arc for each bend of it, and a path for each run of points
/// between them. A closed one is drawn with the run back to its first point, so none of it is lost with the flag.
fn bent(e: &Entity, scale: f64, straight: &mut usize) -> Vec<Shape> {
    let mm = |p: P2| P2(p.0 * scale, p.1 * scale);
    let points: Vec<P2> = e.points.iter().map(|p| mm(*p)).collect();
    let sane = |p: &P2| p.0.is_finite() && p.1.is_finite() && p.0.abs() <= MAX_COORD && p.1.abs() <= MAX_COORD;
    if points.len() < 2 || points.len() > MAX_VERTICES || !points.iter().all(sane) {
        *straight += e.bulge.len();
        return Vec::new();
    }
    let closed = e.flags & 1 == 1;
    let (mut out, mut run): (Vec<Shape>, Vec<P2>) = (Vec::new(), Vec::new());
    for i in 0..points.len() - usize::from(!closed) {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        let bulge = e.bulge.iter().find(|(at, _)| *at == i).map(|(_, bulge)| *bulge);
        match bulge.and_then(|bulge| bow(a, b, bulge)) {
            Some(arc) => {
                if run.len() > 1 {
                    out.push(Shape::Path { points: std::mem::take(&mut run), closed: false });
                }
                run.clear();
                out.push(arc);
            }
            None => {
                *straight += usize::from(bulge.is_some());
                if run.is_empty() {
                    run.push(a);
                }
                run.push(b);
            }
        }
    }
    if run.len() > 1 {
        out.push(Shape::Path { points: run, closed: false });
    }
    out
}

/// The arc a bulge draws from `a` to `b`, or none where it draws none. A bulge is the tangent of a quarter of the
/// angle the arc takes up, signed: counter-clockwise from `a` where it is positive, and clockwise where it is
/// negative — so a positive one bows to the right of the way the polyline runs, which the half circle shows
/// outright (counter-clockwise from the west end of a chord to the east end passes under it, never over). An arc
/// is held counter-clockwise from the first of its angles, so a negative bulge is the same arc read from `b`
/// round to `a`.
fn bow(a: P2, b: P2, bulge: f64) -> Option<Shape> {
    let (across, turn) = (a.dist(b), 4.0 * bulge.atan());
    let (sin, tan) = ((turn / 2.0).sin(), (turn / 2.0).tan());
    if !(across.is_finite() && across > 0.0 && sin.abs() > 1e-12 && tan.abs() > 1e-12 && tan.is_finite()) {
        return None;
    }
    let at = (a + b) * 0.5 + ((b - a) * (1.0 / across)).perp() * (across / (2.0 * tan));
    let radius = (across / (2.0 * sin)).abs();
    let deg = |p: P2| (p.1 - at.1).atan2(p.0 - at.0).to_degrees();
    let (from, to) = match bulge > 0.0 {
        true => (deg(a), deg(b)),
        false => (deg(b), deg(a)),
    };
    let sane = at.0.is_finite() && at.1.is_finite() && at.0.abs() <= MAX_COORD && at.1.abs() <= MAX_COORD;
    (sane && radius.is_finite() && radius > 0.0 && radius <= MAX_COORD && from.is_finite() && to.is_finite()).then_some(Shape::Arc {
        at,
        radius,
        from,
        to,
    })
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
            Shape::Fill { rings, .. } => rings.iter().flatten().for_each(|p| grow(*p)),
            // The points of a measurement, and how far its figure is written beyond the line it is written along.
            Shape::Measure { from, to, line, height, .. } => {
                [*from, *to, line[0], line[1]].iter().for_each(|p| grow(*p));
                let out = height * 2.0;
                for p in [line[0], line[1]] {
                    grow(P2(p.0 - out, p.1 - out));
                    grow(P2(p.0 + out, p.1 + out));
                }
            }
        }
    }
    box_
}

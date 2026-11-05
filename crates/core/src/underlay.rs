//! CAD drawings under a plan, as Revit's Link CAD: a survey or a consultant's drawing shown under the model, to be
//! traced over and snapped to, never edited. The file is read by [`crate::dxf`] and held in `Model::drawings`, keyed by
//! its path; the project's file holds the path alone, as a linked model's does, so a drawing of any size costs a
//! project nothing.
//!
//! What a view draws, and what a point snaps to, is [`lines`]: the drawing's shapes where the underlay stands, turned
//! by its angle, in this project's millimetres. An arc becomes the line through the points along it, as a plan draws
//! every curve.

use crate::geom::P2;
use crate::model::{Model, Underlay};

/// The lines the underlay draws, in this project's millimetres: each a run of points and whether it closes. An underlay
/// whose drawing is not loaded draws nothing, as a link that is not loaded shows nothing.
pub fn lines(m: &Model, u: &Underlay) -> Vec<(Vec<P2>, bool)> {
    lines_by_layer(m, u).into_iter().map(|(_, points, closed)| (points, closed)).collect()
}

/// The same, each line with the layer it is drawn on: what a view needs to draw a layer as its phase says.
pub fn lines_by_layer(m: &Model, u: &Underlay) -> Vec<(String, Vec<P2>, bool)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    // The drawing was read in its own units; `scale` is what it was read with, so its points are millimetres already.
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out: Vec<(String, Vec<P2>, bool)> = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            match shape {
                crate::dxf::Shape::Line(line) => out.push((layer.name.clone(), line.iter().map(|p| place(*p)).collect(), false)),
                crate::dxf::Shape::Path { points, closed } => {
                    out.push((layer.name.clone(), points.iter().map(|p| place(*p)).collect(), *closed))
                }
                crate::dxf::Shape::Arc { at, radius, from, to } => {
                    out.push((
                        layer.name.clone(),
                        crate::geom::along_arc(*at, *radius, *from, *to).into_iter().map(place).collect(),
                        false,
                    ));
                }
                // A word is what the drawing says and no line of the building: it is drawn ([`words`]) and never
                // traced over or snapped to, or a survey's lettering would come in as walls.
                crate::dxf::Shape::Words { .. } => {}
                // Nor is a measurement: the two lines it takes its figure out along are a pair of parallel lines a
                // wall's thickness apart, which is the very thing a trace looks for. It is drawn ([`measures`]).
                crate::dxf::Shape::Measure { .. } => {}
                // Nor is a hatch: its boundary runs along the faces of the wall it fills, and traced it would draw
                // every one of them twice. It is drawn ([`fills`]).
                crate::dxf::Shape::Fill { .. } => {}
            }
        }
    }
    out
}

/// The words the underlay writes, each with the layer it is written on, as the strokes our own lettering draws them
/// with, in this project's millimetres. They are drawn and nothing else: [`lines`] leaves them out.
pub fn words(m: &Model, u: &Underlay) -> Vec<(String, Vec<P2>)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out: Vec<(String, Vec<P2>)> = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Words { at, text, height, angle } = shape else { continue };
            // Our lettering is drawn with y down, from the left of the line it stands on; a drawing has y up.
            let (s, c) = angle.to_radians().sin_cos();
            let turn = |x: f64, y: f64| place(P2(at.0 + x * c - y * s, at.1 + x * s + y * c));
            for stroke in crate::font::strokes(text, 0.0, 0.0, *height as f32) {
                out.push((layer.name.clone(), stroke.into_iter().map(|(x, y)| turn(f64::from(x), f64::from(-y))).collect()));
            }
        }
    }
    out
}

/// What the drawing writes inside `region` that is the name of what is drawn there: of the words written in it, the
/// one written largest, and the lowest where two are written as large. A survey captions each sheet under it, in
/// lettering taller than the names of the rooms inside the plan, so that caption is what the part is called; a part
/// the survey captions nothing has no name of its own to take.
pub fn caption(m: &Model, u: &Underlay, region: [P2; 2]) -> Option<String> {
    let drawing = m.drawings.get(&u.path)?;
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let [sw, ne] = region;
    let mut best: Option<(f64, f64, String)> = None;
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Words { at, text, height, .. } = shape else { continue };
            let said = text.trim();
            let at = place(*at);
            if said.is_empty() || at.0 < sw.0 || at.0 > ne.0 || at.1 < sw.1 || at.1 > ne.1 {
                continue;
            }
            let better = match &best {
                None => true,
                Some((tall, y, _)) => *height > *tall + 1e-9 || ((*height - *tall).abs() <= 1e-9 && at.1 < *y),
            };
            if better {
                best = Some((*height, at.1, said.to_string()));
            }
        }
    }
    best.map(|(_, _, said)| said)
}

/// A height an elevation writes on itself: what it says, in metres above whatever it is measured from, and where in
/// the project the drawing writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub says: f64,
    pub at: P2,
    /// How far out it is from where the others put it, in millimetres: 0 where they all agree.
    pub out: f64,
}

/// What an elevation says about how high things are: the heights written on it, and the one number that turns the
/// drawing into the project's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heights {
    pub marks: Vec<Mark>,
    /// Millimetres to add to a point's y in the project to get the height it stands at, as the drawing writes
    /// heights: `height_mm = y + datum`. None where the drawing writes fewer than one height.
    pub datum: Option<f64>,
    /// The worst a mark is out of what the rest say, in millimetres.
    pub worst: f64,
}

/// How far out of true a height written by hand may be and still be believed: a mark is written to the centimetre
/// and its text is placed by eye against the line it measures, so a couple of centimetres is nothing and a tenth of
/// a metre is another storey's mark read as this one's.
pub const HEIGHTS_AGREE: f64 = 50.0;

/// What a word says as a height, or none: a height is written in metres with its decimals \u{2014} `69.80`, `+3.20`,
/// `70,10` \u{2014} so a word with no decimal point is a door's number or a room's and no height at all.
fn height_said(text: &str) -> Option<f64> {
    let said = text.trim().replace(',', ".");
    let (whole, fraction) = said.split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let whole = whole.strip_prefix(['+', '-']).unwrap_or(whole);
    if !digits(whole) || !digits(fraction) || fraction.len() > 3 {
        return None;
    }
    said.parse().ok()
}

/// The heights the drawing writes inside `region`, held against one another. An elevation marks the heights it is
/// drawn to in metres against the lines it measures, so every mark in it should put the datum in the same place; one
/// that does not is a mark read as something it is not, or a drawing not drawn to its own heights, and the answer
/// says which mark and by how much rather than averaging the trouble away.
pub fn heights(m: &Model, u: &Underlay, region: [P2; 2]) -> Heights {
    let Some(drawing) = m.drawings.get(&u.path) else { return Heights::default() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let [sw, ne] = region;
    let mut marks: Vec<Mark> = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Words { at, text, .. } = shape else { continue };
            let at = place(*at);
            if at.0 < sw.0 || at.0 > ne.0 || at.1 < sw.1 || at.1 > ne.1 {
                continue;
            }
            if let Some(says) = height_said(text) {
                marks.push(Mark { says, at, out: 0.0 });
            }
        }
    }
    marks.sort_by(|a, b| a.says.total_cmp(&b.says));
    // Each mark puts the datum somewhere; where they agree it is theirs, and the middle one of them is taken rather
    // than the average, which one mark read wrong would drag off on its own.
    let mut datums: Vec<f64> = marks.iter().map(|mark| mark.says * 1000.0 - mark.at.1).collect();
    datums.sort_by(f64::total_cmp);
    let datum = datums.get(datums.len() / 2).copied();
    let mut worst = 0.0f64;
    if let Some(datum) = datum {
        for mark in &mut marks {
            mark.out = (mark.says * 1000.0 - mark.at.1 - datum).abs();
            worst = worst.max(mark.out);
        }
    }
    Heights { marks, datum, worst }
}

/// A height a drawing draws a line across at: in a section, a floor, a ceiling, the ground; in an elevation, the
/// eaves and the top of a plinth. What makes one is how much is drawn at it, which is why `span` is reported and
/// not thrown away — a floor runs the width of the building and a window sill runs the width of a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Course {
    /// Where it is in the project, in millimetres.
    pub y: f64,
    /// What height that is, as the drawing writes heights, where its marks said: `y + datum`, in millimetres.
    pub height: Option<f64>,
    /// How much line is drawn at it, in millimetres, taking every piece together.
    pub span: f64,
}

/// How far from level a line may be drawn and still be one: a drawing's floors are drawn flat, and a millimetre over
/// a run is the rounding of the file and nothing else.
const COURSE_FLAT: f64 = 1.0;

/// How near two lines must be to be drawn at the same height: a floor drawn in two pieces is one height, and two
/// floors are never a centimetre apart.
const COURSE_SAME: f64 = 10.0;

/// The shortest line that is a height of its own and not a tick, a hatch's stroke or a symbol's edge.
const COURSE_SHORTEST: f64 = 500.0;

/// The heights the drawing draws across inside `region`, the most drawn first and at most `most` of them, each with
/// what height it is where `datum` says. A section says the heights it is drawn to twice over: in the marks it
/// writes, which [`heights`] reads, and in the lines it draws, which are the floors themselves and are marked only
/// where the draughtsman thought to.
pub fn courses(m: &Model, u: &Underlay, region: [P2; 2], datum: Option<f64>, most: usize) -> Vec<Course> {
    let [sw, ne] = region;
    let inside = |p: P2| p.0 >= sw.0 && p.0 <= ne.0 && p.1 >= sw.1 && p.1 <= ne.1;
    let mut at: std::collections::BTreeMap<i64, (f64, f64)> = std::collections::BTreeMap::new();
    for (_, points, closed) in lines_by_layer(m, u) {
        let close = match (closed, points.len() > 2, points.first(), points.last()) {
            (true, true, Some(a), Some(b)) => Some((*b, *a)),
            _ => None,
        };
        let runs = points
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => Some((*a, *b)),
                _ => None,
            })
            .chain(close);
        for (a, b) in runs {
            let len = (b.0 - a.0).abs();
            if !inside(a) || !inside(b) || (a.1 - b.1).abs() > COURSE_FLAT || len < COURSE_SHORTEST || !len.is_finite() {
                continue;
            }
            let y = (a.1 + b.1) / 2.0;
            let key = (y / COURSE_SAME).round() as i64;
            let held = at.entry(key).or_insert((y, 0.0));
            held.1 += len;
        }
    }
    let mut found: Vec<Course> = at.values().map(|(y, span)| Course { y: *y, height: datum.map(|d| y + d), span: *span }).collect();
    found.sort_by(|a, b| b.span.total_cmp(&a.span));
    found.truncate(most);
    found.sort_by(|a, b| a.y.total_cmp(&b.y));
    found
}

/// The measurements the underlay writes, each with the layer it is written on, as the strokes they are drawn with, in
/// this project's millimetres: the two lines that take the measurement out from the points it was taken between, the
/// line it is written along, a tick at each end of that line, and the figure itself in our own lettering.
///
/// A survey's dimensions are drawn in our own style, as its words are drawn in our own lettering: what is read of a
/// dimension is what it measures and what it says, and the picture a CAD drew it as is the CAD's. Where the drawing
/// does not say how tall the figure is written, the lines of the measurement are drawn and the figure is not. They are
/// drawn and nothing else: [`lines`] leaves them out.
pub fn measures(m: &Model, u: &Underlay) -> Vec<(String, Vec<P2>)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out: Vec<(String, Vec<P2>)> = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Measure { from, to, line, text, height } = shape else { continue };
            let height = *height;
            let mut stroke = |points: [P2; 2]| out.push((layer.name.clone(), points.into_iter().map(place).collect()));
            stroke([*from, line[0]]);
            stroke([*to, line[1]]);
            stroke([line[0], line[1]]);
            let (dx, dy) = (line[1].0 - line[0].0, line[1].1 - line[0].1);
            let len = dx.hypot(dy);
            if len <= 0.0 || height <= 0.0 {
                continue;
            }
            let along = P2(dx / len, dy / len);
            // Which side of the line the figure is written on: the side away from the points it was measured between.
            let off = P2(line[0].0 - from.0, line[0].1 - from.1);
            let gap = off.0.hypot(off.1);
            let side = match gap > 0.0 {
                true => P2(off.0 / gap, off.1 / gap),
                false => P2(-along.1, along.0),
            };
            // A tick where the measurement ends, slanted across the line as a drawing of a building slants one.
            let tick = height / 4.0;
            let slant = P2((along.0 + side.0) * tick, (along.1 + side.1) * tick);
            for p in [line[0], line[1]] {
                stroke([P2(p.0 - slant.0, p.1 - slant.1), P2(p.0 + slant.0, p.1 + slant.1)]);
            }
            if text.is_empty() {
                continue;
            }
            // The figure reads the right way up whichever way the line runs, and stands a quarter of its own height
            // clear of it, which puts the middle of it three quarters out: where a CAD writes it.
            let read = match along.0 > 0.0 || (along.0 == 0.0 && along.1 > 0.0) {
                true => along,
                false => P2(-along.0, -along.1),
            };
            let up = P2(-read.1, read.0);
            let wide = f64::from(crate::font::width(text, height as f32));
            let mid = P2((line[0].0 + line[1].0) / 2.0, (line[0].1 + line[1].1) / 2.0);
            let base = P2(
                mid.0 + side.0 * height * 0.75 - read.0 * wide / 2.0 - up.0 * height / 2.0,
                mid.1 + side.1 * height * 0.75 - read.1 * wide / 2.0 - up.1 * height / 2.0,
            );
            // Our lettering is drawn with y down, from the left of the line it stands on; a drawing has y up.
            for drawn in crate::font::strokes(text, 0.0, 0.0, height as f32) {
                let points: Vec<P2> = drawn
                    .into_iter()
                    .map(|(x, y)| {
                        let (x, y) = (f64::from(x), f64::from(y));
                        place(P2(base.0 + read.0 * x - up.0 * y, base.1 + read.1 * x - up.1 * y))
                    })
                    .collect();
                out.push((layer.name.clone(), points));
            }
        }
    }
    out
}

/// The areas the underlay fills, each with the layer it is filled on, the ring it fills, whether it is filled solid
/// and the families of lines its pattern is drawn with, in this project's millimetres. One for every area a hatch
/// fills, so a hatch that fills two stands twice. They are drawn and nothing else: [`lines`] leaves them out.
pub fn fills(m: &Model, u: &Underlay) -> Vec<(String, Vec<P2>, bool, Vec<crate::dxf::Ruling>)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Fill { solid, rings, rulings, .. } = shape else { continue };
            // The underlay is turned as a whole, so the pattern turns with it.
            let turned: Vec<crate::dxf::Ruling> =
                rulings.iter().map(|r| crate::dxf::Ruling { angle: r.angle + u.angle, ..r.clone() }).collect();
            for ring in rings {
                out.push((layer.name.clone(), ring.iter().map(|p| place(*p)).collect(), *solid, turned.clone()));
            }
        }
    }
    out
}

/// What the underlay says, and where: the middle of each word, how tall it is, and what it reads, in this project's
/// millimetres. The middle of a word and not its start, because that is the point of it that stands in the room it
/// names: a long name written across a small room begins outside it.
pub fn said(m: &Model, u: &Underlay) -> Vec<(P2, f64, String)> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    let (sin, cos) = u.angle.to_radians().sin_cos();
    let place = |p: P2| P2(u.at.0 + p.0 * cos - p.1 * sin, u.at.1 + p.0 * sin + p.1 * cos);
    let mut out: Vec<(P2, f64, String)> = Vec::new();
    for layer in drawing.layers.iter().filter(|l| !u.hidden.iter().any(|h| h == &l.name)) {
        for shape in &layer.shapes {
            let crate::dxf::Shape::Words { at, text, height, angle } = shape else { continue };
            let (s, c) = angle.to_radians().sin_cos();
            let (x, y) = (f64::from(crate::font::width(text, *height as f32)) / 2.0, height / 2.0);
            out.push((place(P2(at.0 + x * c - y * s, at.1 + x * s + y * c)), *height, text.clone()));
        }
    }
    out
}

/// What a layer's own name says it is of the project's history. A survey says it in words (`Stato di fatto`,
/// `Demolizioni`, `Progetto`) and, in Italy, in colours — a drawing is read with yellow for what comes down and red for
/// what goes up, and the layers are often named after them. A name that says nothing is `None`, and a layer that says
/// nothing stands in every phase, as the drawing does.
///
/// A long word is looked for inside a name, because `demolizioni` and `da demolire` are the same word to a reader; a
/// short one has to be the whole of a word in it, because `red` lies inside `arredi` and `new` inside `newel`, and a
/// layer of furniture is not a layer of new work.
pub fn says(layer: &str) -> Option<crate::phasing::Status> {
    let name = layer.trim().to_lowercase();
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let said = |word: &str| match word.chars().count() >= 6 {
        true => name.contains(word),
        false => words.contains(&word),
    };
    // `Demolished` is looked for first: `stato di fatto da demolire` is a demolition, and so is `existing to be
    // demolished`.
    const WORDS: [(crate::phasing::Status, &[&str]); 3] = [
        (crate::phasing::Status::Demolished, &["demoli", "rimozion", "giallo", "yellow", "demolish", "demolition", "remove", "strip"]),
        (crate::phasing::Status::New, &["progetto", "nuovo", "nuove", "costruire", "rosso", "red", "new", "proposed", "proposal"]),
        (crate::phasing::Status::Existing, &["stato di fatto", "esistente", "rilievo", "existing", "survey", "built"]),
    ];
    WORDS.iter().find(|(_, words)| words.iter().any(|w| said(w))).map(|(status, _)| *status)
}

/// The phases a status stands for in `m`: what a layer of it is built in, and what it is taken down in. The project's
/// first phase is what is there already and its last is what is being built, which is what a survey's two layers mean.
pub fn phases_of(m: &Model, status: crate::phasing::Status) -> (Option<String>, Option<String>) {
    let first = m.phases.first().cloned();
    let last = m.phases.last().cloned();
    match status {
        crate::phasing::Status::Existing => (first, None),
        crate::phasing::Status::Demolished => (first, last),
        crate::phasing::Status::New => (last, None),
        crate::phasing::Status::Temporary => (last.clone(), last),
    }
}

/// What every layer of the drawing `u` reads is, where its name says: the phases to give it, ready for the user to
/// change. A drawing that is not loaded says nothing.
pub fn read_phases(m: &Model, u: &Underlay) -> Vec<crate::model::LayerPhase> {
    let Some(drawing) = m.drawings.get(&u.path) else { return Vec::new() };
    drawing
        .layers
        .iter()
        .filter_map(|l| {
            let (created, demolished) = phases_of(m, says(&l.name)?);
            Some(crate::model::LayerPhase { layer: l.name.clone(), created, demolished })
        })
        .collect()
}

/// What the layers a wall's two faces are drawn on make of the wall: the phases they agree on. A wall is not in two
/// moments of a project's history, so layers that disagree give nothing and are reported; a layer that says nothing
/// leaves the other to speak, and two that say nothing leave the wall in the project's first phase, as anything
/// unsaid is.
#[allow(clippy::type_complexity)]
pub fn phase_across<'a>(
    u: &'a Underlay,
    a: &str,
    b: &str,
) -> Result<Option<&'a crate::model::LayerPhase>, (&'a crate::model::LayerPhase, &'a crate::model::LayerPhase)> {
    let of = |name: &str| u.phases.iter().find(|l| l.layer == name);
    match (of(a), of(b)) {
        (Some(x), Some(y)) if x.created == y.created && x.demolished == y.demolished => Ok(Some(x)),
        (Some(x), Some(y)) => Err((x, y)),
        (Some(x), None) | (None, Some(x)) => Ok(Some(x)),
        (None, None) => Ok(None),
    }
}

/// What layer `layer` of `u` is in the phase at `at` in the project's order; none when it isn't there: built after it,
/// or taken down before it. A layer the underlay says nothing about is there in every phase.
pub fn status(m: &Model, u: &Underlay, layer: &str, at: usize) -> Option<crate::phasing::Status> {
    let Some(l) = u.phases.iter().find(|l| l.layer == layer) else { return Some(crate::phasing::Status::Existing) };
    let created = l.created.as_deref().and_then(|n| crate::phasing::index(m, n)).unwrap_or(0);
    let demolished = l.demolished.as_deref().and_then(|n| crate::phasing::index(m, n));
    if created > at {
        return None;
    }
    match demolished {
        Some(d) if d < at => None,
        Some(d) if d == at => Some(if created == at { crate::phasing::Status::Temporary } else { crate::phasing::Status::Demolished }),
        _ => Some(if created == at { crate::phasing::Status::New } else { crate::phasing::Status::Existing }),
    }
}

/// The box an underlay's lines lie in, mm: what a view needs to fit one. None where it draws nothing.
pub fn extent(m: &Model, u: &Underlay) -> Option<[P2; 2]> {
    let mut found: Option<[P2; 2]> = None;
    for p in lines(m, u).iter().flat_map(|(points, _)| points.clone()) {
        found = Some(match found {
            None => [p, p],
            Some([lo, hi]) => [P2(lo.0.min(p.0), lo.1.min(p.1)), P2(hi.0.max(p.0), hi.1.max(p.1))],
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dxf::{Drawing, Layer, Shape};
    use std::sync::Arc;

    /// A model with one underlay, of one shape, where `at` and turned `angle`.
    fn with(shape: Shape, at: P2, angle: f64) -> (Model, Underlay) {
        let mut m = Model::new("Test");
        let drawing = Drawing { layers: vec![Layer { name: "MURI".into(), shapes: vec![shape] }], scale: 1.0, ..Drawing::default() };
        m.drawings.insert("flat.dxf".into(), Arc::new(drawing));
        let u = Underlay { name: "Survey".into(), path: "flat.dxf".into(), at, angle, scale: 1.0, ..Underlay::default() };
        (m, u)
    }

    #[test]
    fn a_drawing_stands_where_the_underlay_does_and_turns_with_it() {
        let line = Shape::Line([P2(0.0, 0.0), P2(1000.0, 0.0)]);
        let (m, u) = with(line.clone(), P2(500.0, 200.0), 0.0);
        assert_eq!(lines(&m, &u), vec![(vec![P2(500.0, 200.0), P2(1500.0, 200.0)], false)]);
        // A quarter turn takes the line north; the underlay's own point stays where it is.
        let (m, u) = with(line, P2(0.0, 0.0), 90.0);
        let [(points, _)] = &lines(&m, &u)[..] else { panic!() };
        assert_eq!(points[0], P2(0.0, 0.0));
        assert!((points[1].0).abs() < 1e-9 && (points[1].1 - 1000.0).abs() < 1e-9, "{:?}", points[1]);
    }

    #[test]
    fn an_arc_becomes_the_line_through_the_points_along_it() {
        let (m, u) = with(Shape::Arc { at: P2(0.0, 0.0), radius: 1000.0, from: 0.0, to: 90.0 }, P2(0.0, 0.0), 0.0);
        let [(points, closed)] = &lines(&m, &u)[..] else { panic!() };
        assert!(!closed && points.len() == 19, "{} points for a quarter at 5°", points.len());
        assert!((points[0].0 - 1000.0).abs() < 1e-9 && (points[18].1 - 1000.0).abs() < 1e-9, "{:?}", points);
        // Every point is on the circle.
        assert!(points.iter().all(|p| (p.0.hypot(p.1) - 1000.0).abs() < 1e-9));
    }

    #[test]
    fn a_hidden_layer_draws_nothing_and_a_drawing_that_is_not_loaded_draws_nothing_either() {
        let (m, mut u) = with(Shape::Line([P2(0.0, 0.0), P2(1.0, 0.0)]), P2(0.0, 0.0), 0.0);
        u.hidden = vec!["MURI".into()];
        assert!(lines(&m, &u).is_empty());
        u.hidden.clear();
        u.path = "elsewhere.dxf".into();
        assert!(lines(&m, &u).is_empty(), "a file that was never read draws nothing");
    }

    #[test]
    fn a_circle_closes_on_itself() {
        let (m, u) = with(Shape::Arc { at: P2(0.0, 0.0), radius: 500.0, from: 0.0, to: 360.0 }, P2(0.0, 0.0), 0.0);
        let [(points, _)] = &lines(&m, &u)[..] else { panic!() };
        assert_eq!(points.len(), 73, "a circle at 5°");
        assert!((points[0].0 - points[72].0).abs() < 1e-9 && (points[0].1 - points[72].1).abs() < 1e-9);
        assert_eq!(extent(&m, &u).map(|[lo, hi]| (lo.0.round(), hi.0.round())), Some((-500.0, 500.0)));
    }
}

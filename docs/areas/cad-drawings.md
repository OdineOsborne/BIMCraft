# CAD drawings: reading DXF

The 2D a building is modelled over — a survey, a consultant's plan, a sketch from another office — arrives as a CAD
drawing. `crates/core/src/dxf.rs` reads it: `dxf::read(bytes, &Options)` gives a `Drawing` of `Layer`s, each with the
`Shape`s drawn on it, in millimetres. The `dxf` command reports one without changing the project — its layers and how
many shapes are on each, the box it lies in, the unit it was read in and what was left out — which is what to run on a
file before deciding what to do with it.

**Why DXF is read here and DWG is not.** DXF is the format Autodesk publishes a specification for, so it is read under
the rule that formats come from public specifications. DWG is Autodesk's own and has none, so BIMCraft does not read one
itself: `crates/dwg` turns a DWG into DXF bytes and this reader reads those. That crate holds the one dependency that
can do it, `acadrust` (MPL-2.0, used unmodified: it covers its own files, so BIMCraft's stay MIT OR Apache-2.0 —
`NOTICE`), and nothing of its types leaves that crate, so what reads a drawing is still one reader. LibreDWG, the other
open reader, is GPL-3 and could not be used this way.

The dependency decodes what a file claims, and a file may be damaged or hostile, so it is guarded from the outside: a
DWG past `Limits::dwg` is refused before the decoder is handed anything, the DXF it writes is cut off at `Limits::dxf`
while it is written, and the whole of it runs inside `catch_unwind`. What that does not bound, and the crate says so, is
a small file that declares a huge table inside itself: capping that needs the decode to run where an operating system
can limit its memory.

The applications hand the converter to the session (`Session::dwg`), as the desktop hands it the Revit reader: the core
never depends on `bimcraft-dwg`, and a build made `--no-default-features` refuses a DWG by name instead of converting
it. So `dxf` and `underlay` take a `.dwg` wherever they take a `.dxf`, and `converted` is what the DWG reader said it
could not read.

**What it reads.** `LINE`, `LWPOLYLINE`, `POLYLINE` with the `VERTEX` entities that follow it to its `SEQEND`, `ARC` and
`CIRCLE`, each on the layer its group code 8 names (an entity that names none is on layer `0`, as a drawing has it).
Everything else is counted by its own name and listed in `left_out`, the most of a kind first: `312 TEXT`, `4 SPLINE`.
Blocks are not opened — an `INSERT` is listed, because what it stands for is a definition this does not read yet — and
hatches, dimensions and splines are listed too. A polyline's bulge is read as the straight line between its points and
said to be; a polyline of more points than `MAX_VERTICES` is left out whole rather than cut short, because half a
drawing is worse than none of it.

**The unit.** The file says its own in `$INSUNITS`, and inches, feet, millimetres, centimetres and metres are read from
it. A file that says nothing, or says something else, is read as millimetres, which a drawing of a building nearly
always is; a caller who knows better gives `Options::scale`, millimetres to one of the file's units, and that wins.

**Hostile by default.** The file's size, its points' distance from the origin, how many shapes a drawing holds and how
many points a polyline holds are all bounded, a value that is no number leaves its entity out, and a file that is no
drawing comes back empty rather than breaking. A binary DXF is refused by name.

**Looking at the result.** `dxf_tests.rs` reads files written by hand, pair by pair. A drawing as a CAD really writes
one — with its `TABLES`, `BLOCKS` and `OBJECTS` sections — is another thing, and the reader was measured against one
written with `ezdxf` (a flat with double-line walls, closed room outlines, an old-style `POLYLINE`, text, a block
insert, a dimension, a hatch and a spline): the walls and the outlines came in, and the rest was counted and named.

## The underlay: a drawing under a plan

Manage > Link CAD (LC) puts a drawing under the plan on screen, as Revit's Link CAD does: `Element::Underlay`, with the
path, the level whose plan draws it, where its own origin stands, how far it is turned and which of its layers it leaves
out. It is the project's reference and none of its substance — no tool edits it, nothing of it is scheduled, checked or
exported, and a drawing of any size costs the project's file nothing, because **the file holds the path alone**
(`Model::drawings`, keyed by path, is `serde(skip)`, as `Model::loaded` is for linked models). Opening a project reads
its drawings again, and one that cannot be read is named in `drawings_not_loaded`, as a link that cannot be read is.

What a plan draws, and what the view fits itself to, is `underlay::lines`: the shapes where the underlay stands, turned
by its angle, in millimetres, an arc as the line through the points along it (5° apart). They are drawn first of all, in
a hairline and faint, so what is modelled stands out from what is only drawn over. A level's plan draws the underlays on
that level and no others.

**Snapping to it** is the point of having it: a picked point sticks to the drawing's ends, to the middle of each piece
of it, to a circle's centre and to where two of its lines cross, as it does to the project's own walls and grids
(`snap::underlay`). The snaps in force decide, as ever — an underlay is one more thing in the plan, not a mode.

That runs at every move of the mouse, and a survey holds tens of thousands of lines, so what is gathered is bounded
twice over: a piece is turned where the underlay stands only when the box round it, turned, reaches the cursor's window,
and at most `MAX_UNDERLAY` pieces are taken. Measured on 50,000 lines (`underlay_snap_speed`, ignored like the other
measurements): the file reads in about 45 ms and a snap takes about 0.2 ms, against a frame's 16.

## The phases a survey already carries

A survey is drawn in layers that say what each line is of the project's history — `Stato di fatto`, `Demolizioni`,
`Progetto` — and in Italy also in the colours a drawing is read with, yellow for what comes down and red for what goes
up, with the layers often named after them. That is the phase information, written by hand by whoever drew it, and it
is read rather than guessed at: `underlay::says` gives a layer's status from its own name, `read_phases` turns that into
the pair an element holds (the phase a thing is built in, the one it is taken down in), and the `underlay` command fills
it in unless the caller says otherwise — what it read comes back in `phases`, to be corrected.

A long word is looked for inside a name, because `demolizioni` and `da demolire` are one word to a reader. A short one
has to be a whole word in it: `red` lies inside `arredi` and `new` inside `newel`, and a layer of furniture is not a
layer of new work — a test holds that line. A layer no word of this names says nothing, and stands in every phase, as
the drawing does.

A plan then draws a layer as it draws an element of that status: `underlay::status` answers by the same rule
`phasing::status` answers for an element, the view's own phase and phase filter decide (`Look::at`, `Look::filter`),
what is not there in this phase is not drawn, and what the filter overrides is drawn as the project's override for that
status says — dashed and red for what comes down, by default. So a plan set to the first phase shows the building as it
stands, and one set to the last shows it as it will be, out of a single drawing.

## The walls a survey already draws

A draughtsman draws a wall as its two faces, so a wall in a drawing is a pair of lines running alongside one another a
wall's thickness apart. `trace::read` takes every straight run of the drawing long enough to be a face, pairs each with
the nearest face across it, and gives the line down the middle of each pair with the thickness it measured; the
`trace_walls` command builds a wall on each, on the drawing's own level. `How` is what it looks for, all in the
project's millimetres: `thin` and `thick` (60 and 800) bound a wall's thickness, so a closer pair is a line's own
thickness or a hatch and a farther one is a room; `shortest` (400) is the shortest face, and the shortest stretch two
faces must run alongside one another; `skew` (2 degrees) is how far out of parallel a wall's faces may be drawn.

Four rules keep it from finding walls where there are none, and each of them was put there by a real survey.

- **The nearest face across.** In a corridor 600 wide between two walls 100 thick, every distance from 100 to 800 is
  there to be mistaken for a wall. A face's wall is the nearest face across it, within a tenth of that distance, so the
  far side of a room is never one.
- **The faces of a wall keep the same distance apart** over the stretch they share. A pair that splays is a kerb, a
  ditch, a roof's pitch — not a wall.
- **A ladder is no wall.** A stair's treads, a hatch, a fence and a run of joists are parallel lines evenly spaced,
  and every neighbouring pair of them measures the same thickness: on the survey this was written against, a flight of
  nine treads 280 apart read as eight walls. A wall's face has a room on one side of it, so the asymmetry tells them
  apart — a face with a face across it on *both* sides, as far away as each other and over the same stretch, is a rung,
  and no pair it is in is a wall. The cost is a wall with a cavity behind it exactly as wide as the wall, which reads
  as a ladder of four and is drawn by hand.
- **An axis is stretched, never shortened to nothing.** The faces that make a wall stop half a wall short of a corner,
  so the axis between them does too; an end within half the thickness of another wall's axis is taken to the crossing.
  An end whose crossing would leave less than half the wall, or turn it round, is left where it was.

Pieces of one wall, where the drawing broke a face in two, are joined where they touch; a gap stays a gap, so a doorway
is not bricked up.

Each wall takes the phase the layers of its two faces name, as Link CAD read them (`underlay::phase_across`), so a
survey's `Stato di fatto` comes in built and its `Progetto` comes in new, from one drawing. A wall whose two layers
name different phases is left in the project's first and reported in `disagreed`: a wall is not in two moments of a
project's history, and which one it is in is the user's to say. Each thickness found, to the nearest 10 mm, comes in as
the project's own wall type within a tenth of it, or as `Generic - <n>mm`, made for it.

What it found is a reading, like the phases: `preview` reports every wall it would build and builds nothing, and the
answer always carries `faces`, how many lines were long enough to be a wall's face, and `longest`, the longest line in
the drawing. That last one is what tells a drawing that is not in the units it claims — the survey this was written
against says `$INSUNITS` millimetres and is drawn in metres, so its longest line is 13.73 mm and there is no wall in
it until Link CAD is given `scale`. Nothing guesses at a scale.

Measured on that survey (280 shapes, 204 faces): the DWG reads and links in about 50 ms and the trace takes about 9 ms,
finding 56 walls and 101 m of them, or 48 walls and 88 m with the surveyor's own layers hidden. Every pair of faces is
weighed, so the work is the square of their number: a synthetic plan of 1,200 rooms (9,600 faces) traces 4,800 walls in
about 0.9 s, and more than `MAX_FACES` (10,000) faces is refused with what to do about it.

## The window: Trace Walls

All of this is a reading, and a reading is to be looked at before it is believed, so the window (`crates/ui-egui/src/trace.rs`,
Trace Walls, `TW`, beside Link CAD on the Manage tab) shows it and lets it be put right. It opens on the drawing under
the plan on screen — with no plan open, or no drawing under it, it says so and opens nothing, as Link CAD does — and
holds two tabs.

**Layers** is a row for each layer the drawing has: whether it is drawn, how many shapes are on it, and the two phases
it stands in, each picked from the project's own. It is what Link CAD read from the layers' names, there to be
corrected, because a name is a guess and a surveyor's `rosso` may be a property line and not new work. Each click is
its own `cad_layers` command and its own undo step, as the Phasing dialog's clicks are; `<None>` is nothing said, and a
layer with neither phase said of it stands in every phase and leaves the other face of a wall to say which phase the
wall is in. **Read the Layer Names Again** puts back what the names say, for a change thought better of. A layer that
is not drawn is not traced either, which is how a surveyor's own layers — levels, GPS points, a north arrow — are kept
out of the walls.

**Walls** is what to look for (`How`, as above), the wall type every wall is to come in as or `By thickness`, and two
buttons. **Find** runs `trace_walls` with `preview`: it builds nothing, says how much it found of what thickness, type
and phase, and draws every wall it found over the plan — a band as thick as it measured with its axis down it. **Build**
builds them and closes. What was found was found with the measurements, the type and the layers as they were, so
changing any of them drops it and it has to be found again: the drawing on screen is never a reading older than the
window. A refusal is said inside the window, because the status box in a view's corner can lie over a modal's buttons.

**What is still missing**: openings and rooms. A drawing's blocks (`INSERT`) are not read either, so a survey that
draws its doors as blocks draws nothing of them.

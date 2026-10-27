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

**What is still missing**: finding walls, openings and rooms in a drawing, which is the wizard this was the first step
of; a drawing is traced over by hand until then.

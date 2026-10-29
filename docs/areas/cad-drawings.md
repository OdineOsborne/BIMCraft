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

Six rules keep it from finding walls where there are none, and each of them was put there by a real survey.

- **The nearest face across.** In a corridor 600 wide between two walls 100 thick, every distance from 100 to 800 is
  there to be mistaken for a wall. A face's wall is the nearest face across it, within a tenth of that distance, so the
  far side of a room is never one.
- **The faces of a wall keep the same distance apart** over the stretch they share. A pair that splays is a kerb, a
  ditch, a roof's pitch — not a wall.
- **A ladder is no wall.** A stair's treads, a hatch, a fence and a run of joists are parallel lines evenly spaced,
  and every neighbouring pair of them measures the same thickness: on the survey this was written against, a flight of
  nine treads 280 apart read as eight walls. A wall's face has a room on one side of it, so the asymmetry tells them
  apart — a face with a face across it on *both* sides, as far away as each other and over the same stretch, is a rung,
  and no pair it is in is a wall.
- **A mark inside a wall is no face of one.** A survey draws a window by carrying the wall's two faces across the
  opening and drawing the glass down the middle of it; the glass is 125 from a face where the far face is 300, so the
  nearest-face rule takes glass and face for a wall 130 thick and loses the 300 wall they are drawn in. On the survey
  this was written against that happened at eight windows. Again it is the asymmetry that tells them apart: a face
  with a face closer than `INSIDE` (200 mm) on *both* sides, over the whole of it, is inside something, and the pair
  that has it between them is the wall. Hatching, a cavity's lining and a duct read the same way, and a wall with a
  cavity behind it now comes in as one wall as thick as both. What it costs is two walls less than 200 mm apart,
  which come in as one wall as thick as the pair of them: a space narrower than that is not a room.
- **A wall of two leaves is one wall.** The same thing a step further out. The survey this was written against draws
  every wall of its outside as three lines — the outside of it, a line 100 in, and the inside at 400 — which is a
  hundred of facing on three hundred of wall. The middle line has a face 100 away on the one side and 300 on the
  other, too far for `INSIDE` and too near for the pair to be a room, so the nearest-face rule takes the facing for
  the wall and leaves the inside of the wall over. A line left over is the sign of a wrong reading: the wall is the
  two outer lines, 400 thick, and the one between them is the leaves' own. It is read so only where neither of the
  two has anything beyond it that would make a thinner wall with it — on that survey the plot's boundary runs 590
  outside the east wall, which is why that one wall needs the tightest-wall test to come out right — and where it
  does not lie halfway between them, which is a ladder's rung and not a leaf. A lining and a cavity's inner leaf read
  the same way. What it costs is the wall of two equal leaves, which stays what the nearest pair makes of it.
- **An axis is stretched, never shortened to nothing.** The faces that make a wall stop half a wall short of a corner,
  so the axis between them does too; an end within half the thickness of another wall's axis is taken to the crossing.
  The crossing it is taken to is the one the two walls have least to go to meet, and not simply the one nearest the
  end: on the survey this was written against a wall runs north from 60 inside the north-east corner, so its axis
  passes through the end of the top wall exactly, and taking the nearest crossing held that end 60 short of the
  corner and left the building open — which is a building with no rooms in it. An end whose crossing would leave
  less than half the wall, or turn it round, is left where it was.

## The openings in those walls

Pieces of one wall, where the drawing broke a face in two, are joined where they touch, or where the gap between
them is too narrow to walk through — under `NARROWEST`, below — because that is where the drawing broke the line and
not where it left a doorway. The two halves of one wall drawn on two layers break at different places, and on the
survey this was written against the top wall of the house is drawn in five pieces whose faces stagger by up to 180:
joined, it is one wall 8.6 m long, and the house has a top to it. A gap that is left is a
doorway, and a doorway is not a gap between two walls but a door in one wall, which is what the building is: the two
pieces are made one wall and `Hole` records the opening in it. A survey marks an opening the other way too, carrying
the wall's faces across it and drawing two lines across the wall at the jambs, and that is read as well — but only
with the glass between them, because the space between two windows is a pair of lines at a pair of jambs as well.

A jamb is drawn from the one face of the wall to the other, so it is as long as the wall is thick (`t * 0.8..=t *
1.3`); a mark half as long again is the survey's own — which way the door opens, where the sill is — and is no jamb.

**Glass is what tells a window from a door.** A line drawn alongside the wall inside an opening, clear of both faces,
is glass or a sill, and the opening is a window; an opening with nothing drawn in it is a hole you can walk through,
which is a door. Nothing else in a plan says which it is: the survey this was written against draws no door swings at
all, so an arc is not looked for. A line that runs more than half the length of the wall at that distance off its
axis is the wall's own and no glass (`runs_along`): the line between a wall's two leaves lies inside its thickness
from end to end, so without that the pier between two windows reads as a window itself.

**A wall that stops short of the wall it runs into has a doorway at its end** (`trace::open_ends`). The piece of
wall between a door and the corner is a few centimetres long, which is shorter than anything a draughtsman draws as a
wall's face, so the drawing shows the wall ending at the door — and a building is a wall with a door in it, not a wall
that stops in mid-air. So an end that holds on to nothing and stands within a doorway's width (`DOORWAY`, 1,200 mm: a
double door, and wider than that the one room is open to the next) of the wall it was going to meet is taken to it,
and what is left between where it stopped and the face of that wall is the opening. What says the wall carries on is
the drawing itself: one of its own two faces drawn across the gap (`face_across`, four fifths of it), where the wall
between the two faces was too short to be read. Without that nothing is built across a room that nothing was drawn
across — and with `WIDEST` in place of `DOORWAY` that one survey gained 8.6 m of wall it does not have, and a pair of
two-metre doorways where its two faces merely stagger.

An opening is between `NARROWEST` (500 mm) and `WIDEST` (3,000 mm) wide — narrower is where the drawing broke a line,
wider is where a wall stops — and those, like `INSIDE`, are measures of buildings and not the user's to set. A plan
gives no heights, so a door is built 2,100 high and a window 1,500 high on a 900 sill, which are the sizes the
project's own starter types carry; an opening that would stand higher than the wall it is in is left out and counted
in `too_high`. Each width found, to the nearest 10 mm, comes in as the project's own door or window type within a
tenth of it, or as `Single <w> x 2100` or `Fixed <w> x 1500`, made for it. `openings: false` leaves the drawing as it
is: two walls with a gap between them.

## The land around the building, and what else is no wall

What the pairing leaves is then tidied of three things that pair as a wall does and are no wall of the building
(`trace::tidy`). None of them was thought of in advance: each is what the survey this was written against actually
draws, and together they take what the pairing leaves down to the walls the house has.

- **Nothing is a wall across itself.** A pair shorter than it is far apart is a pier drawn as a box, a plinth, a
  chimney, or the two jambs of an opening read as a pair of faces — on that survey, lines 262 long and 542 apart. The
  thickness is what the pair measured, so it is the length that has to answer for itself.
- **Two walls cannot be in the same place.** Where one wall's axis runs inside another over most of the shorter of
  them, the longer is the wall and the shorter is what the drawing draws in it: a survey draws a window's frame across
  the opening it fills, from the outside of the wall to the inside, and the wall's own inner face stops at the jambs,
  so the frame has nothing nearer and reads as a wall 400 thick lying over a wall 100 thick. Four windows of that
  survey are drawn so.
- **A wall of a building meets another wall**, at a corner or running into one, and a survey draws the land around the
  building too: the boundary of the plot, the kerb of the road, the neighbour's wall, the lines the surveyor took his
  levels along. Those hold on to nothing, so a wall that meets none of the others (`trace::joined`, an end within half
  the two thicknesses of the other's own stretch, which is where the axes have already been taken) is left out and
  counted in `alone` — on that survey five lines: a boundary 13.6 m long, one across the plot, and the lines the
  surveyor took his levels along. `alone: true` takes them, for a survey of a garden wall or a boundary to be built. Unless nothing in
  the drawing meets anything at all, when the drawing is of a wall or two rather than of a building and every one of
  them is taken.

The three run in that order, and they carry one another: once a symbol's own lines are gone the pair that was drawn
against them meets nothing either, which is how the two jamb boxes and the frame between them go together. What it
leaves is the same whether or not the surveyor's symbol layers are turned off in the window, which is the point — a
reading that needs the layers right to come out right is a reading that cannot be trusted.

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

Measured on that survey (280 shapes, 204 faces): the DWG reads and links in about 50 ms and the trace takes about
9 ms, finding 20 walls, 5 doors and 6 windows, with its outside 400 thick all round. Every pair of faces is weighed,
so the work is the square of their number: a synthetic plan of 1,200 rooms (9,600 faces) traces 4,800 walls in about
2.6 s, and more than `MAX_FACES` (10,000) faces is refused with what to do about it. Looking for the openings in
them as well takes it to about 10 s, because `glazed_openings` weighs every line of the drawing against every wall
found; that is where to look first if a big drawing is ever to be traced quickly.

## The rooms those walls shut in

A survey is traced for the building, and a building is rooms. The walls are what says where they are: a room is an
area the walls of a level shut in, which is a bounded face of the planar graph their centre lines make
(`geom::regions`, `Model::enclosures`), with the separate pieces standing inside it — a shaft, a duct — as its
islands. That is the same geometry a room has always used to measure itself (`geom::region_at`, the boundary from the
walls around a room's point, Revit's "At wall centerline"), asked the other way round: not *what is around this
point* but *where are all the areas*, in one pass over the drawing instead of one pass for each room.

**Place Rooms Automatically** (`place_rooms`, Revit's own tool, which has no shortcut there either; in the window it
is **Place Rooms**, beside Room on the Architecture tab, and it fills the plan on screen) puts a room in
every one of them that holds none. Each room is placed at its region's `middle`: its centre of area, which is where
anyone would point at a room, or — where the region bends round that, as an L-shaped room does, or an island stands
on it — the middle of the largest triangle its outline cuts into, and failing that a step in from its longest edge,
which is what is left of a room that runs all the way round a core. They are numbered from the back of the plan to
the front and from the left across, as a plan is read.

Two things are passed over and counted rather than placed. An area that already holds a room is that room's
(`held`), so the command can be asked again after a wall is drawn by hand and only the new rooms appear — which is
how a survey that leaves a gap is finished off. And an area smaller than `SMALLEST_ROOM` (a quarter of a square
metre, which is a duct's shaft and the least of a building that is anywhere at all) is counted in `small`: it is the
sliver two walls leave between their centre lines where one is drawn a hair off the other, and no room.

`trace_walls` takes `rooms` and does the same with the walls it has just built, in the one command, so one undo step
takes the lot back. On the survey this was written against that gives two rooms, 8.06 m² and 58.56 m², which is the
main block of the house in two; the rest of it stays open, because the survey draws partitions that stop two metres
short of the walls they run into and nothing is built across a gap that wide. A wall drawn by hand across one of
those, and `place_rooms` again, is what finishes it.

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

**Walls** is what to look for (`How`, as above), whether to find the openings as well, whether to take the lines that
stand alone too, whether to place the rooms too, the wall type every wall is to
come in as or `By thickness`, and two buttons. **Find** runs `trace_walls` with `preview`: it builds nothing, says how
much it found of what thickness, type and phase, how many doors and windows, and how many lines it left out for
meeting no wall of the building, and draws it all over the plan — each
wall a band as thick as it measured with its axis down it, each opening a lighter stretch of that band, with a line
down the middle of a window and none down a door. **Build** builds them, places the rooms if they were asked for, says how many of each, and closes. Asking for the
rooms does not drop what was found, because the rooms are of the walls and not of the drawing: a Find builds nothing,
so it places nothing either. What was found was found with the measurements, the type and the layers as they were, so
changing any of them drops it and it has to be found again: the drawing on screen is never a reading older than the
window. A refusal is said inside the window, where it is read beside the button that was pressed.

It is the one window here that is not a modal, and for a reason: what it shows is on the plan behind it, and walls
drawn under a dim box that stands in the middle of them and cannot be moved are no use. So it floats, is dragged by
its title, rolls up to that title to get out of their way, and takes no keys — the plan pans, zooms and fits (`ZF`)
while it is open, and the only typing it takes is into numbers.

**What is still missing**: rooms. A drawing's blocks (`INSERT`) are not read either, so a survey that draws its
doors and its furniture as blocks draws nothing of them, and the openings in such a drawing are not found.

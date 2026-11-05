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
`TEXT` and `MTEXT` come in as the words the drawing says, `DIMENSION` as what it measures and `HATCH` as what it
fills with (all below). Everything else is counted by its own name and listed
in `left_out`, the most of a kind first: `3251 POINT`, `4 SPLINE`. A polyline's bulge is read as the straight line between its points and
said to be; a polyline of more points than `MAX_VERTICES` is left out whole rather than cut short, because half a
drawing is worse than none of it.

**The words a survey writes.** A survey writes on its drawing — the name of each room, what it measures, the height of
a sill — and a plan that shows none of that is half a plan. A `TEXT` or an `MTEXT` comes in as `Shape::Words`: what it
says, the left of the line it stands on, how tall its capitals are and how far it is turned. The file says where a word
stands under its own justification (an `MTEXT` hangs from its top left unless it says otherwise, and a `TEXT` not
written from the left of its line gives that point under codes 11 and 21), and it is brought back to the left of the
line with the width our own lettering gives it, which is the lettering it will be drawn in. An `MTEXT`'s formatting —
the font, the colour, the height, the paragraph breaks — is not what it says, so it is stripped and what is left is one
line. A word longer than `MAX_WORD` is left out whole, as a polyline of too many points is.

They are drawn with the drawing, as the strokes our own lettering draws them with (`underlay::words`), and they are
nothing else: `underlay::lines` leaves them out, so nothing snaps to lettering and no wall is ever traced from it — a
survey's title, drawn 500 mm tall, would otherwise pair with itself as walls.

**Blocks.** A survey draws its doors, its windows and its furniture as blocks, so a block is opened where it is put: an
`INSERT` brings in what the `BLOCK` of that name holds, measured from the block's base point, turned and scaled as the
insert says and as many times over as its grid of copies (codes 70, 71, 44 and 45) says. What a block draws on layer `0`
comes in on the layer the block is put on, as a drawing has it, so it reads with the other walls of that layer and takes
its phase; what a block draws on a layer of its own keeps that one. A block holds blocks, which are opened too, up to
`MAX_NESTING` deep — past that a drawing is drawing itself, and what is left unopened is counted. An `INSERT` naming a
block the file does not hold is counted like anything else left out. What a block cannot bring in is a round thing it
scales unevenly, which is an ellipse and no shape this draws: those are counted too. How many times a block is opened
at all is bounded by `MAX_SHAPES`, and not only how many shapes it brings in, because a block holding nothing draws
nothing however many million times over a grid asks for it.

**The measurements a survey writes.** A survey measures on its drawing, and what it measured is often the only place a
length is said exactly: the drawing is drawn to what was surveyed, and the figure beside it is what was read off the
tape. A `DIMENSION` comes in as `Shape::Measure`: the two points it measures between (codes 13 and 14), the line it
writes the measurement along, and the figure it writes there.

The line is not always the line between those two points, and this is the whole of why the two are kept apart. A
linear dimension is written along a line turned as its code 50 says, and measures what falls on it: a surveyor
measuring the width of a room between two corners that are not level with one another writes the width, not the way
between the corners. So the two points are brought onto that line, and how long it is between them is what the drawing
measured. On the survey this was built against, 124 of its 566 dimensions measure along a line the two points do not
lie on, 108 of them by more than a degree, so taking the distance between the points instead would have said the wrong
length for one dimension in five.

The figure is read where it was written and never worked out again from what was measured. A drawing prints a length
in units and to a precision of its own, kept in its dimension style and not in the dimension: that survey is drawn in
metres and prints centimetres, so a wall 2.795 drawn says `280`. Its code 42, the measurement, is in those printed
units too and not in the drawing's, which is why it is not read as a length. What is read is what is written: the
drawing's own word on it (code 1) where a surveyor wrote one over the figure, and otherwise the figure the picture a
CAD drew the dimension as prints — that picture is a block of its own, holding the lines, the ticks and the figure as
an `MTEXT`, and the figure is taken out of it. Where a survey writes its own word over the figure, the geometry is
still there beside it to be held against what it says: 167 of those 566 have one, and one of them says `870` over a
line drawn 8800 long.

**What a drawing measures, held against what it draws.** This is what reading the dimensions is for. `dxf::measured`
takes every measurement whose figure is a number and holds it against the length under it, and the `dxf` command
reports what it finds, so that a survey is read before it is traced. The unit is the difficulty: a drawing prints in
units of its own and says so nowhere in the dimension. But what nearly every figure is, is the length under it
printed in those units, so the unit is taken as the one the most figures agree on — of the units the drawing's own
figures make, each held in turn against all the others, the one that comes out right for the most of them. The middle
one of them would do on most drawings and not on all: on a 14-dimension corner of that survey, where half the figures
are written over by hand, the middle one made a figure of `405` over a length of 405.43 disagree, which is the
rounding a drawing prints with and no disagreement at all.

A figure disagrees where it is further from the length under it than half of its own last digit — the rounding it is
printed to. On the whole of that survey, 427 of its 567 measurements are the length under them to the digit, and 140
are not; every one of the 140 is a figure a surveyor wrote over the one the CAD worked out, and the furthest off says
`870` over a wall drawn 8800. That is the survey saying something its drawing does not, and a wall traced from that
drawing comes out 100 mm too long. It is also a check on this reading: of the 427 figures no one touched, not one
disagrees, which the projection, the unit and the stripping of an `MTEXT`'s formatting all have to be right for.

A figure that is no number is read and drawn like any other and nothing is worked out from it: a survey writes `var.`
where a wall is not square. A figure written `2,80` is the number `2.80`, because a drawing made in Italy prints its
decimal point as a comma.

A dimension is drawn in our own style, as a survey's words are drawn in our own lettering — the two lines out to where
the measurement is written, the line it is written along, a tick at each end and the figure, which stands a quarter of
its own height clear of the line, where a CAD writes it (`underlay::measures`). And it is nothing else: `underlay::lines`
leaves it out, so nothing snaps to a dimension and no wall is traced from one. That matters more here than it does for
lettering — the two lines a dimension takes its figure out along are a pair of parallel lines a wall's thickness apart,
square to a third, which is exactly what a trace looks for.

The kinds of dimension that measure no length between two points — an angle, a diameter, a radius, a coordinate — are
counted by what they measure (`1 DIMENSION of an angle`) and never guessed at. Two of a dimension's group codes say
something else of another entity, and both were read as that other thing before they were read as a dimension's: code
3 is the style it is drawn in, where an `MTEXT`'s is a chunk of what it says, and code 42 is what it measures, where a
polyline's is an arc read straight. A drawing of 601 dimensions reported 601 arcs it had read straight.

**A drawing whose units are not the ones its file says.** That same unit is what catches it. The survey here says
`$INSUNITS` 4, millimetres, and is drawn in metres: linked as it says, a window cut out of it — a ground floor
plan, the elevations and the sections — comes in 124 mm across, its longest line is 96 mm, and Trace Walls finds
not one face. Read at a thousand, the same window gives
1316 faces, 212 walls, 44 doors and 10 windows. The figures are what say so — read as the file says, they come out
0.01 mm to one, and nobody writes a length in hundredths of a millimetre. A length is written in millimetres,
centimetres or metres, the one a drawing is drawn in and the one it prints both; so for each of the three the
drawing could be printing, the scale that would make it so is worked out and kept where that scale is itself one of
the three (`dxf::scale_says`). Here one survives, a thousand, and that is what the drawing is drawn at. The `dxf`
command reports them in `measures.scale_says` and Link CAD returns them beside the scale it read with, because that
is where the trap is sprung. Nothing is said of a drawing whose figures already come out in one of the three, which
is nearly every drawing, and nothing is corrected behind the caller's back: the scale a drawing is read at stays
theirs.

**What a drawing fills an area with.** A survey hatches to say what something is: this wall is brick, this is the
ground, this is the roof. A `HATCH` comes in as `Shape::Fill` — the name its pattern goes by, whether it is filled
solid, the areas it fills, and the families of parallel lines that pattern is drawn with.

It is read out of its group codes in the order the specification gives them, and not gathered code by code as every
other entity is, because the same code says a different thing at each turn of a hatch: after the number of boundary
loops (91) each loop says what kind it is (92) and then either the points of a polyline or one edge after another,
each saying its own kind first. Line edges and arc edges are read, an arc as the points along it; a loop drawn with a
spline or an ellipse is not read and is counted, because half a boundary is a ring that is not the shape it fills.
Only the loops the file marks as a boundary of their own — external, or outermost — are filled: a loop that is
neither is an island left out of the hatch, and filling one would fill what the drawing left empty.

The pattern is drawn as the file draws it and not as the nearest of our own, so a pattern this has no name for comes
out right. The file gives each family of lines its angle (53) and the offset from one line of it to the next (45 and
46), turned as the family itself is: how far apart the lines are is the part of that offset lying across them, and
the rest is how far along itself each line stands from the one before it. It gives the dashes the lines are drawn
with, too (79 and 49), and those matter: that survey's ground is `AR-CONC`, thirteen families of dashes, and drawn
with unbroken lines instead it came out a dense web of crossings where the drawing shows a stipple. A dash of no
length is a dot, which is not drawn.

A fill is drawn and nothing else: `underlay::lines` leaves it out, so nothing snaps to a hatch and no wall is traced
from one. A hatch runs along the faces of the wall it fills, and read as geometry it would draw every one of them
twice. Of that survey's 415 hatches — `ANSI31`, `SOLID`, `AR-CONC`, `AR-RSHKE`, `HONEY`, `AR-SAND`, `ANSI37` — two are
not read and four boundaries are drawn with a spline or an ellipse.

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

**Which file, and at what scale.** The window takes a DXF and, where the build has what turns one into the other, a
DWG (`Session::dwg`, `crates/dwg`): the file picker offers both, and a DWG becomes DXF bytes on the way in. The scale
is asked for before anything comes in, and only when there is a reason to ask: Link CAD reads the file first with the
`dxf` command, which changes nothing, and where that comes back with `measures.scale_says` — the drawing measuring
on itself and saying it is not in the units its file declares — a small window says both numbers and offers
either. Whichever is pressed brings the drawing in with one `underlay` command, so one undo step takes it back out,
and Cancel brings in nothing. Where the drawing's measurements hold up, or it has none, nothing is asked and it comes
in as the file says. This is why the scale is a question and not a correction: a drawing that comes in a hand's width
across is unusable, and a scale changed behind the user's back is worse.

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
  far side of a room is never one. What it costs is a face that runs along two walls — on the survey this was written
  against, the back of the house is one line with a lining 130 thick against the first 1.3 m of it and the wall
  itself, 300 thick, against the next 1.8 m. The nearer of the two is taken for the whole of it and that 300 wall is
  lost. Reading each pair over the stretch the two of them share recovers it, and was tried: it also brings back the
  pairs this rule is here to stop, because a face whose own wall is not drawn over some stretch of it then takes the
  far side of the room across it — on that survey a wall 790 thick across a corridor, and one 690 thick between the
  house and the boundary of the plot. It recovered one wall and invented two, so the rule stays as it is.
- **The faces of a wall keep the same distance apart** over the stretch they share. A pair that splays is a kerb, a
  ditch, a roof's pitch — not a wall.
- **A ladder is no wall.** A stair's treads, a hatch, a fence and a run of joists are parallel lines evenly spaced,
  and every neighbouring pair of them measures the same thickness: on the survey this was written against, a flight of
  nine treads 280 apart read as eight walls. A wall's face has a room on one side of it, so the asymmetry tells them
  apart — a face with a face across it on *both* sides, as far away as each other, is a rung; and so are the first and
  the last of a ladder, which have one side only, where the nearest line across them has another twice as far away on
  that same side. A rung is no face of a wall at all, so it is not the nearest face across another one either, which
  is what keeps a stair drawn against a wall from taking the wall's own face away from it. Both ask only after the
  lines that run alongside the whole of this one and have the whole of themselves alongside it, because a ladder's
  rungs are drawn alike: on that survey the treads of a stair, 1000 long, fall 360 under the inner face of the north
  wall, 350 thick, whose own outer face lies 350 over it and runs 7600. Reading that face as a rung lost the north
  wall east of the stair, left the house open at its north-east corner, and left the north block of it with no room.
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
  corner and left the building open — which is a building with no rooms in it. How far short of a corner an axis may
  stop and still be taken to it is `NARROWEST`, a gap nobody could walk through, because a gap at a corner that no
  one can pass is no gap but a corner drawn carelessly; and because a corner is two axes both stopping short of it,
  taking the one to the other moves the mark the next is measured from, so it is done again until nothing moves (at
  most `ROUNDS`, which is there so that nothing can run away with a drawing). The two together — a reach of
  `NARROWEST` and the rounds — close all but five of that survey's wall ends, and those five stop 304 mm and more
  short of anything. An end that has found
  its crossing is not weighed again — stretching an axis leaves the line it runs along where it was, so the crossings
  do not move — which is why the rounds cost nothing on a large plan. An end whose crossing would leave
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
9 ms, finding 21 walls, 7 doors and 6 windows, with its outside 400 thick all round. A synthetic plan of 1,200 rooms
(9,600 faces) reads and links in about 80 ms and traces 4,800 walls in about 250 ms with the openings in them, or
100 ms without them; a drawing with more than `MAX_FACES` (10,000) lines as long as a wall is refused, with what to
do about it.

That trace took 9.7 s, and every second of it was one thing: a drawing weighed against the whole of itself. Nothing
read here reaches far — a wall is two faces no farther apart than `thick`, two pieces of one wall are a doorway
apart, a corner is a crossing a step from an end, one wall stands in another only where their axes are within a
thickness of each other — so the drawing is laid out in squares (`Squares`) and each of those questions is asked of
the lines and the walls near the one asking. On that plan, before and now:

| | before | now |
|---|---|---|
| `inside`, the marks and the leaves | 560 ms | 15 ms |
| `across`, the pairing itself | 160 ms | 7 ms |
| `join`, the pieces of one wall | 140 ms | 5 ms |
| `open_up`, the doorways | 8,700 ms | 150 ms |
| `meet`, the corners | 600 ms | 11 ms |
| `tidy`, what is no wall | 860 ms | 13 ms |

Each is measured where it does its work: `open_up` with the openings asked for, `meet` and `tidy` without them,
where the pieces of a wall stay apart and there are 4,800 walls to weigh against each other and not 140.

Two of those were not the squares. **`open_up` began again at the first wall after every piece of a wall it took
in** — 4,660 restarts on that plan — and it needs no restart at all, because taking a piece in makes an opening for
no wall but the one that took it: `beyond` finds the next piece and the wall is asked again until there is nothing
beyond it. And **`glazed`**, which says whether an opening has glass in it, **read every line of the drawing for
every opening**; the lines are in the squares too (`Marks`), and a wall is handed those drawn within the widest of
its openings of its axis. The guess written here before was wrong, which is the reason for measuring: the one named
as the bottleneck, `glazed_openings`, costs 7 ms.

The squares change nothing that is read: the same 21 walls, 7 doors, 6 windows and three rooms on the survey, and
the same 4,800 walls and 4,660 openings on the plan of 1,200 rooms. Each question keeps the reach it had, and the
squares only spare it the asking of what lies farther off than that. Two things to know about them. They are cut
from the drawing's own size, so on a drawing a few centimetres across one of them is a fraction of a millimetre, and
nothing outside the drawing is ever asked for. And a line in too much of the drawing to be put in a square — the
boundary of a plot, a grid line drawn right across the plan, a long diagonal — is in none of them and seen by
everything, because a drawing holds few of those and cutting each into the hundred squares it crosses would cost
more than keeping it whole.

What is left on that plan is `open_up`'s 150 ms, which is `glazed` asking the squares once for every opening it
finds, over walls that are by then as long as the plan.

## One sheet at a time

A survey is rarely one plan in a file. The one this was written against next, a `permesso di costruire` as it was
issued (`Elaborati Grafici Asborno PDC Rilasciato.dwg`, AutoCAD 2004, 1.1 MB, 8,290 shapes on 21 layers), holds the
whole set of sheets side by side in model space: 521 m across and 1,889 m from the first to the last, with the plans,
the elevations, the sections, the site plan and the points the surveyor took by GPS all in the one drawing.

Everything above then works against itself, because **an elevation's lines pair as a wall's faces do**. A window drawn
in elevation is two uprights the width of the window apart; a storey's floor and ceiling lines are parallel and a
storey apart; a section is a plan stood on its side. Traced whole, that file gives 729 walls, 127 doors and 42 windows
from 4,773 faces — and in the one sheet looked at closely, of the 45 openings found, 19 are in the two plans and 26
are in the four elevations and the section. More than half of what it finds is drawn in a view that has no walls in it
at all. Nor is any of it reachable by turning layers off: the plans and the elevations of one sheet are drawn on the
same layers, because they are one drawing.

So `region` says where to look: `[[x, y], [x, y]]`, the south-west corner and the north-east, and a run of the drawing
with an end outside it is left out whole (`How::within`, `How::holds`). Both ends, not one, and no clipping: a line
that leaves the area belongs to what is drawn beside it, and half a wall at the edge of the area is worse than none.
Draw the area a little wider than the plan. Everything the answer reports is then of what was read inside it,
`longest` included, so the measure of the drawing is the measure of the sheet being traced.

The rule it rests on is the user's eye and not a heuristic. Finding the sheets in a file automatically — by the frames
drawn round them, by the clusters the shapes fall into — was not done: a sheet set has no mark in a DXF that says so,
the frames are lines like any others, and a wrong guess would silently trace half a building. What the user does is
crop the plan to the sheet, which they can see.

## What the drawing is made of

An area is enough to trace one sheet, but a survey is worked through sheet by sheet, and the second time you want the
same rectangle back. So a drawing is told what its parts are (`Underlay::parts`, `cad_parts`): each is a `name`, a
`kind` (`plan`, `elevation` or `section`), a `region`, the `level` it is the plan of, and the `phase` what it draws is
in. They are the drawing's own and the project's file carries them, as it carries its layers' phases.

Trace Walls then takes one by name: `trace_walls {"part": "PIANTA PIANO TERRA"}` is its area, its level and its phase
in one word, so two plans in one file become two floors in two commands. A part the drawing hasn't is refused, and the
refusal names the ones it has.

The phase is on the part and not only on the layers because of what a real file does. The permesso di costruire this
was measured against draws its existing state on its own sheets, not on its own layers: of 8,290 shapes, the layer
`Stato di fatto` holds 89, while `progetto` and `rosso` hold 1,967 between them. Read by layer, nearly everything in
it stands in every phase. Read by part, the sheet says it once. So where a part says a phase, every wall traced from
it comes in that phase whatever its layers say, and the disagreement between two layers is not reported, because it
has been overruled rather than left unsettled.

**The name comes from the drawing.** A survey captions each of its sheets under it — `PIANTA PIANO TERRA`,
`PROSPETTO SUD-EST`, `SEZIONE A-A` — in lettering taller than the room names inside the plan, so `underlay::caption`
takes the words written inside a region, the largest, and the lowest where two are written as large. That is what a
new part is called, and it is a proposal like any other here: it is written into a field the user can change.

What is not done is finding the parts by itself. A sheet has no mark in a DXF that says it is a sheet; its frame is
lines like any others, and the clusters the shapes fall into are not sheets either (a plan and the elevation beside it
are one cluster as readily as two). A wrong guess would silently trace half a building, so the user draws the area —
which they can see — and the drawing only names it.

A room takes the name the survey writes inside it, and which survey that is depends on who is asking. `place_rooms`
takes the drawings under the plan it is filling, as it always has. Trace Walls takes **the drawing it has just
traced**, wherever that drawing stands: a part traced onto a floor of its own is a plan of that floor, though the
file itself is linked under one level only. Without that, the basement of a sheet set comes in as `Locale 4` and
`Locale 5` while the ground floor comes in as `CANTINA` and `SOGGIORNO/CUCINA`, which is how this was found.

## The heights an elevation writes

A plan says nothing about how high anything is; the elevation beside it says it all, and says it in writing. An
Italian survey marks its heights in metres against the lines it measures — `69.80`, `69.85`, `70.10`, each with
its own little triangle on the line — and those marks are words like any other, so they can be read
(`underlay::heights`, `cad_heights {"part": …}`).

The reading rests on one number. Within one view, the drawing is to scale in both directions, so every mark says the
same thing: `height_mm = y + datum`. Each mark puts that datum somewhere, the middle one of them is taken (the
median, which one mark read wrong cannot drag off as a mean would), and every mark is then held against it and
reported with how far `out` it is. That is the whole of the check, and it is worth more than the reading: the marks
of one view agree to a centimetre or two, because they were written against lines drawn to scale, and nothing else in
a drawing does.

Measured on the permesso di costruire: its SEZIONE A-A writes three heights, 69.70, 69.85 and 70.10, and they put the
datum within **13.5 mm** of one another — on marks written to the centimetre and placed against their lines by eye.
Asked instead for a region holding all three views of that sheet at once, the same reading comes back **29 metres**
out, because the same height is written again 29 m away in another view. So the check catches the one mistake this
invites, which is drawing the area too big, and says it in metres rather than quietly averaging three datums into a
fourth that is none of them.

A section says its heights twice over, and the second saying is the one that is always there. It writes marks where
the draughtsman thought to, and it draws a line at every floor, every ceiling and the ground whether or not anyone
marked them, so `courses` reads those too: every run drawn level (within a millimetre) and long enough not to be a
tick (500 mm), gathered by height to the centimetre, with how much line is drawn at each. What makes a height
worth having is how much is drawn at it — a floor runs the width of the building and a window sill runs the width
of a window — so they come back most-drawn first, at most sixteen of them, and the datum from the marks turns each
into a height.

On the permesso di costruire's SEZIONE A-A the most-drawn line is at **69.81 m with 11.8 m of line**, which is the
ground floor; the marks written on the same section say **69.85**. Two readings that share nothing but the drawing
agree to four centimetres. Above it a cluster at 72.5–72.96 is the eaves, which makes the storey about 3.15 m, and
nothing in the drawing writes that number anywhere.

It also shows where the region has to be tight. Asked for a region 17 m tall around that section, `courses` came
back with lines at 80 m: the view above it, whose own marks were outside the region, so the marks agreed with one
another and the lines were another drawing's. The marks check what the marks can check. The lines are read from
wherever the region reaches, so the region is drawn around one view and no more.

What is a height and what is not: a word is a height where it reads as a number with its decimals (`69.80`, `70,10`,
`+3.20`), which is how a height is written and how a door number, a room number and an area with its unit are not.
`HEIGHTS_AGREE` is 50 mm, which is wider than a mark written to the centimetre and placed by eye, and narrower than
one storey's mark read as another's.

## Standing an elevation against its plan

Reading heights is one thing; knowing which window they belong to is another. In the vertical the drawing anchors
itself, because its marks say what height they are. Sideways it says nothing: no mark in a DXF ties a point of an
elevation to a point of the plan beside it. So the user says it, with two points (`Part::anchor`, `cad_anchor`): two
points of the building as the plan draws them, and the same two as the elevation draws them.

Two is exactly enough, and only if both lie on the face the elevation shows. An elevation is that face seen square
on at the plan's own scale, so the direction the face runs in plan is the direction between the two plan points, and
the offset is what carries one to the other. Where a plan point falls off the face, it falls onto it: `drawn_x`
projects, which is what an elevation does.

And because the scale is the plan's, **the two distances must come out the same** — which is the check that they
are the same two points. It is worth its keep. Anchoring the permesso di costruire's PROSPETTO SUD-EST by the ends
of the traced south wall against the ends of the elevation's outline is refused: *the plan has those two points 8850
mm apart and the part draws them 9287 mm apart*. The 437 mm is half a wall at each end — a traced wall's axis runs
down the middle of it, and the elevation draws the outside. Anchored by the outer corners instead, it comes out
`out: 0`, `along: 0°`, and a point halfway along the facade lands halfway along the drawing.

What is not attempted is finding the anchor by itself. Matching the plan's outline to the elevation's would need to
know which of them is the outside of what, and the two differ by a wall's thickness, an eaves overhang and a plinth,
none of which the drawing labels. Two clicks settle it, and the arithmetic says at once whether they were the right
two.

## The sills and the heads of the openings

With the elevation stood against its plan, the two readings can be put together: the plan says where an opening is
and how wide, the elevation says how high (`underlay::drawn_lines`, `underlay::opening`,
`cad_openings {"part": …, "plan": …}`). It reads and reports and builds nothing.

What a line of the drawing has to be to be an opening's own is the whole of it. Three things say it:

- **It is level and drawn as one stretch.** Every run drawn flat inside the region is gathered by the height it is
  drawn at *and by touching*: pieces at one height that meet, or all but meet (100 mm, which is a mullion, a
  down-pipe, a shutter's hinge), are one line, and two pieces of that height with a window between them are two
  lines. That is the difference from `courses`, which gathers a height's pieces wherever in the region they are
  drawn, because a floor drawn across two wings is one floor.
- **It is as wide as the opening**, to within a tenth of its width or 100 mm, whichever is more. This is what
  separates a sill from the facade's own lines, which run the width of the building, and — measured on the real
  file — a ground-floor window from a dormer drawn in the same facade.
- **It stands where the plan puts it**, within `slack` of it: by default the opening's own width, held between 500
  and 1500 mm, which is nearer than the next opening of a facade stands.

The sill is the lowest of what is left, the head is the highest, and an opening the elevation draws fewer than two
lines of gets neither: it is counted in `missed` and said, not invented. A door is that case by nature — its sill is
the floor itself and the floor runs the width of the facade — so a door takes its sill from `floor`, the height the
storey's floor stands at as the drawing writes heights, which `cad_heights` reports as a course. `floor` earns its
keep twice, because with it an opening is read only between that floor and `storey` (3000 mm) above it, and an
elevation draws every storey of the facade at once.

Three numbers come back that nobody asked for and that are the point of the exercise:

- `off` and `plane`: how far behind the line the elevation was stood against each opening lies, and the middle of
  where those with a sill and a head lie. A wall's openings are drawn in its one face, so they agree; `off_plane`
  counts those that do not, whose lines may be those of whatever the elevation draws in front of them.
- `shift`: how far the lines found lie from where the plan put them. One shift for all of them is two anchor points
  picked that much along the face, and worth putting right.

Measured on the permesso di costruire's PROSPETTO SUD-EST against its PIANTA PIANO TERRA, which is where all three
checks were learned. Of the 13 openings the plan finds, 9 are counted elsewhere — 6 whose walls run across the face
and 3 that stand off the plane and that the elevation draws nothing of — and the facade's three come out:

| the plan | the elevation |
|---|---|
| window 1014 mm | sill 70.69, head 72.19, **1500 mm high** |
| door 962 mm | sill 69.75, head 71.90, **2150 mm high** |
| window 1085 mm | sill 70.61, head 72.19, **1580 mm high** |

A 1.5 m window on a 0.89 m sill and a 2.15 m door, off a drawing that writes none of those numbers anywhere. All
three stand 1852 mm behind the line the elevation was stood against, and all three have their lines about 450 mm
from where the plan put them (−458, −482, −423, so `shift` −453): the two points this elevation was anchored by are
that much along the face, and the openings were found anyway because the slack is the opening's own width. The
first reading of the same file found **nothing at all**, because the rule then was that a line had to begin and end
within the opening's own band; the 450 mm shift put every band beside its window. And the one opening found off the
plane is an interior door 10.8 m behind the facade whose band happens to fall under a window of it — `off_plane: 1`
is that, said rather than passed off as a reading.

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

A room is called what the survey writes inside it, because that is why a surveyor writes it there: of the words
standing in the area (above), the name is the one written nearest the middle of it, and the bigger of two as near.
Nearest the middle and not the biggest, because a survey writes more in a room than its name: on the one this was
written against the big room holds `Soggiorno/Cucina` across the middle of it and a surveyor's single `K` in a corner,
both the same size, and `Camera 1`, `Camera 2` and two `Bagno` fall where they should. What it costs is a note written
across the middle of a room, which would be taken for its name. A room an area holds nothing written in takes the
project's own word for a room, and a `name` the caller gives is the name of every room it places, whatever the drawing
says.

Two things are passed over and counted rather than placed. An area that already holds a room is that room's
(`held`), so the command can be asked again after a wall is drawn by hand and only the new rooms appear — which is
how a survey that leaves a gap is finished off. And an area smaller than `SMALLEST_ROOM` (a quarter of a square
metre, which is a duct's shaft and the least of a building that is anywhere at all) is counted in `small`: it is the
sliver two walls leave between their centre lines where one is drawn a hair off the other, and no room.

`trace_walls` takes `rooms` and does the same with the walls it has just built, in the one command, so one undo step
takes the lot back. On the survey this was written against that gives four rooms, 28.55 m², 4.37 m², 8.06 m² and
54.19 m², 95.17 m² together: the north block of the house whole, and the main block in three. What stays open is
inside that last one, where the survey draws the partition between the front room and the back of the house as a
single line over the five metres between the two ends of it it draws as a pair, and a line is no wall; the back of
the house is a face the nearest-face rule reads as a lining for the whole of its length as well. A wall drawn by
hand across one of those, and `place_rooms` again, is what finishes it.

## The window: Trace Walls

All of this is a reading, and a reading is to be looked at before it is believed, so the window (`crates/ui-egui/src/trace.rs`,
Trace Walls, `TW`, beside Link CAD on the Manage tab) shows it and lets it be put right. It opens on the drawing under
the plan on screen — with no plan open, or no drawing under it, it says so and opens nothing, as Link CAD does — and
holds four tabs.

**Layers** is a row for each layer the drawing has: whether it is drawn, how many shapes are on it, and the two phases
it stands in, each picked from the project's own. It is what Link CAD read from the layers' names, there to be
corrected, because a name is a guess and a surveyor's `rosso` may be a property line and not new work. Each click is
its own `cad_layers` command and its own undo step, as the Phasing dialog's clicks are; `<None>` is nothing said, and a
layer with neither phase said of it stands in every phase and leaves the other face of a wall to say which phase the
wall is in. **Read the Layer Names Again** puts back what the names say, for a change thought better of. A layer that
is not drawn is not traced either, which is how a surveyor's own layers — levels, GPS points, a north arrow — are kept
out of the walls.

**Parts** is a row for each part the drawing is made of: its name, what it is, the level it is the plan of and the
phase it draws, each put right where the reading was wrong, with **Add the plan's crop as a part** above them. That is
the whole of marking up a sheet set: crop the plan around a sheet (Crop View, dragging the grips on its edges), press
the button, and the part is there with the survey's own caption as its name and the plan's own level as its level.
The button is greyed where the plan has no crop, with the hover saying what to do, and a change is its own `cad_parts`
carrying the whole list, as the Layers tab's clicks are their own `cad_layers`.

**Walls** is what to read, what to look for (`How`, as above), whether to find the openings as well, whether to take
the lines that stand alone too, whether to place the rooms too, the wall type every wall is to
come in as or `By thickness`, and two buttons. **What to read** is the whole drawing, what the plan's crop holds, or
one of its parts by name: the crop is an area and nothing more, a part carries its level and its phase as well. It
stands on the crop where the plan was cropped before the window was opened, because a plan cropped before tracing was
cropped to what is to be traced; and a crop taken off, or a part done away with, while the window stands open puts it
back to the whole drawing, so the button never reads the whole file when it says it is reading a part. **Find** runs `trace_walls` with `preview`: it builds nothing, says how
much it found of what thickness, type and phase, how many doors and windows, and how many lines it left out for
meeting no wall of the building, and draws it all over the plan — each
wall a band as thick as it measured with its axis down it, each opening a lighter stretch of that band, with a line
down the middle of a window and none down a door. **Build** builds them, places the rooms if they were asked for, says how many of each, and closes. Asking for the
rooms does not drop what was found, because the rooms are of the walls and not of the drawing: a Find builds nothing,
so it places nothing either. What was found was found with the measurements, the type and the layers as they were, so
changing any of them drops it and it has to be found again: the drawing on screen is never a reading older than the
window. A refusal is said inside the window, where it is read beside the button that was pressed.

**Openings** is the two readings put together: the elevation to read the sills and the heads off, the plan the
openings are found in, the height the storey's floor stands at (0 is not said), and **Read the Sills and Heads**,
which runs `cad_openings`. It needs a plan and an elevation among the parts, and says so where there is not one of
each; the two combos hold the parts of each kind and fall back to the first of them, so a part done away with while
the window stands open leaves nothing stale behind. What comes back is said in the window as it is: how many
openings are on the face and how many have both a sill and a head, each of them a line of its own (*1500 mm Window:
sill 70.70 m, head 71.90 m, 1200 mm high*), and an opening the elevation draws nothing of said to be that and no
more. The three checks are said too, where they have something to say: how many openings stand off the plane the
others agree on, and how far the lines found fall from where the plan puts them, which is the anchor's own error and
is read on the Parts tab. It builds nothing, as Find builds nothing.

A Find that comes back with no wall at all is nearly always a drawing linked at the wrong scale, and the window says
so. Where the drawing measures on itself, it says what the drawing says: `trace_walls` passes `scale_says` on and the
window reads *the measurements written on it say it is drawn at a scale of 1000*. Where it does not, the longest line
in it is all there is to go on, and that is what the window shows instead.

It is the one window here that is not a modal, and for a reason: what it shows is on the plan behind it, and walls
drawn under a dim box that stands in the middle of them and cannot be moved are no use. So it floats, is dragged by
its title, rolls up to that title to get out of their way, and takes no keys — the plan pans, zooms and fits (`ZF`)
while it is open, and the only typing it takes is into numbers.

**What is still missing**: what a drawing says is read and drawn but not yet used. A wall is traced from the
geometry alone — the measurement written over it and the hatch filling it are reported and drawn and go no further,
so a wall that a survey hatches `AR-CONC` does not come out concrete, and one a surveyor wrote `870` over comes out
the 8800 it is drawn.

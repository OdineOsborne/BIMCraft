# CAD drawings: reading DXF

The 2D a building is modelled over — a survey, a consultant's plan, a sketch from another office — arrives as a CAD
drawing. `crates/core/src/dxf.rs` reads it: `dxf::read(bytes, &Options)` gives a `Drawing` of `Layer`s, each with the
`Shape`s drawn on it, in millimetres. The `dxf` command reports one without changing the project — its layers and how
many shapes are on each, the box it lies in, the unit it was read in and what was left out — which is what to run on a
file before deciding what to do with it.

**Why DXF and not DWG.** DXF is the format Autodesk publishes a specification for, so it is read here under the rule
that formats come from public specifications. DWG is Autodesk's own and has none: a DWG is refused by name, saying to
convert it (that conversion, and whether it ever happens inside BIMCraft, is a decision of its own — the Revit reader's
fence in [docs/rvt-provenance.md](../rvt-provenance.md) is what such a thing would have to live under). LibreDWG, the
open reader, is GPL-3 and cannot be linked into this project.

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

**What this is for.** It is the first step of the 2D the model is traced over: nothing in the project holds these shapes
yet. What they become — an underlay drawn under a plan and snapped to, and then walls, openings and rooms found in them
— is the work that follows.

# Project parameters and schedules

Moved from the root `CLAUDE.md`, word for word: the root keeps the rules and the map, and this file is read when the area is touched.
Managing parameters as a set (dictionaries, import and export, batch changes): [parameters](parameters.md).

(A distribution board's panel schedule is not one of these: it is a view of its own, `panel_schedule.rs`, in [structure and MEP](structure-and-mep.md).)

A project parameter (`Model::parameters`) is a value each element of its categories may hold in its `params`, of its kind;
a component's `params` hold its family's instance values too, so a parameter for components can't take a family
parameter's name, nor any category's own field's (`schedule::own_field`). A schedule's fields are its category's own
(`OWN` in `schedule.rs`, read from `element_json`), the project parameters for it and, for components, the families'
parameters, or calculated: a formula over the row's others, read as the family formula engine reads (lengths in mm, areas in
m²), a field with no value leaving it with none (an unset yes or no is no). Filters compare numbers as the schedule shows
them, so two that show the same are equal. The seven schedules every project has (`schedule::VIEWS`) aren't elements
until changed or placed (`schedule::standard`), and a schedule named as one of them, in any language or case, is that one; renaming a parameter renames it in schedules and in the formulas naming it. The last of them is the sheet list (`of: "sheet"`), the project's sheets by their numbers, a category
no view draws.
A component's Room Number and Room Name (as Revit's Room: Number and Room: Name) are the room it is in, found where it is
read and never stored (`Rooms::component`, the one rule `elements`, schedules, `load_points` and IFC share): the room on its
level whose boundary at the walls' centre lines holds its family's room calculation point (else its insertion point) in plan,
and whose height holds it, so an outlet on a wall's face is in the room on that side whatever the project measures rooms at
([families](families.md), `room_calculation_point`). `element_json` gives it as `room` (the room's GUID), `room_number` and
`room_name`; a component outside every room, or over or under it, has none.
A multi-category schedule (`of: "multicategory"`, `schedule::MULTI`, as Revit's Multi-Category Schedule) has a row for each
element of the categories it lists (`Schedule::categories`; none, every built one, `schedule::built`: not rooms, levels,
masses and sheets) and the fields they share (`SHARED`): Category (the element's, as elements name theirs), Family, Type, Level, Count,
Width, Height, Length, Thickness and Area, each its own category's field of that name where it has one holding the same and
empty where not (an area worked out for walls, length by height, and doors and windows, width by height; a roof's plan
area), and Material, its type's structure layer's (or its thickest's), a run's type's or a family's first material
parameter's. Its filters, sorting, grouping, totals and calculated fields are any schedule's; only it lists categories.
A table goes to a file as CSV (`schedule::to_csv`, text a spreadsheet would read as a formula guarded with `'`) or, for a path
ending `.xlsx`, as a workbook of one sheet (`schedule::to_xlsx` over `xlsx.rs`, which writes the parts and the zip by hand on
`flate2`): headings in bold, numbers as numbers so they sum, text as inline strings, which a spreadsheet never takes for a formula.

## The price list

A bill of quantities needs prices, and a published price list holds thousands of them, so they are the project's data and
not something typed into a formula: `Model::price_lists` holds a list for each one read, under the name it was given, and
an item for each article code with its wording, its unit as the list writes it and its unit price. A list and every one of
its items is a library entry (`Kind::PriceList`, `Kind::PriceItem`), so each has an id and a GUID, a file holds them like
any other entry, and `Model::check` holds the caps (`MAX_PRICE_LISTS`, `MAX_PRICE_ITEMS`), the names, and a price being a
finite number that is not negative.

**The columns are the caller's.** `price_list` takes `code` and `price` (and `description` and `unit` where the file has
them) as the *headings* that hold them, matched ignoring case and surrounding space, so a price list of another region,
another language or another shape needs no code written for it; `separator` is the character between the columns, and
without it whichever of `;`, `,` and a tab splits the heading row into the most. A price may be written with a comma for
its decimal point, as an Italian price list writes one. The mapping is kept with the list, so the same file reads the same
way again, and a row with no code or whose price is no number is left out and named, never guessed.

**The mapping is picked, not typed.** `price_columns` reads a file's headings, its first rows and which column each role
looks like — guessed from what a price list calls the role in the languages a project is written in here, a heading equal
to one of those words winning over a heading that merely holds it — and changes nothing. That is what Manage > Price List
(PL, `crates/ui-egui/src/prices.rs`) offers: the file, a drop-down for each role filled from the file's own headings, the
separator, the first rows to look at, and the lists the project already holds. Nothing in the window reads a file itself.

## The article code, and a bill of quantities

An element is joined to its price by the **article code it carries**, not by a rule worked out again at every reading:
`price_code` writes the code on the elements as the value of a project parameter, `Model::code_parameter` holds which
parameter that is, and `Model::article_code` reads it. So the price is a fact about the element — it shows in the
Properties palette, it is a column of a schedule, it goes out in IFC with the other parameters, and a wall that is priced
wrongly is answered for by looking at the wall.

What the rules do is **give the code to many elements at once**: `price_code` takes the elements from `ids`, from a view
filter by name, or from `categories` with the same `rules` a schedule and a view filter use (`schedule::picks`). Nothing
new was invented for picking elements out, and the window where those rules are written is the one that was already
there. Running it again after a model comes in from Revit prices what came in. A code no price list holds is refused, so a
typo is not a silent hole in the bill.

Where a price reaches a category (`Model::prices_reach`: the project has a list, and the code's parameter is for that
category) a schedule gains three fields from the price list (`schedule::PRICE_FIELDS`): `Price Description`, `Price Unit`
and `Unit Price`, read through the code the element carries. There is deliberately **no amount field**: which quantity a
price is for is the price list's business, so the amount is a calculated field the user writes over the quantity they
mean — `Length * Height / 1000000 * Unit Price` for a wall priced by the square metre. An element with no code, or with a
code no list holds, has no value in those fields, so its amount comes to nothing instead of counting as zero: a hole in
the bill is visible.

In the window, Manage > Price List is also where a code is given: the project's items under a search, the picked one
assigned to the selection (or taken off it). Select All Instances (SA) over a type is what makes that a whole type at
once.

The quantity a wall is priced by is its own: `Area` is one face of it and `Net Area` is that face less the doors and
windows in it ([walls](levels-walls-roofs.md)), so `Net Area * Unit Price` is the amount of a wall priced by the square
metre, and the openings are not paid for twice.

## The bill of quantities in IFC

The project leaves as a bill of quantities as well as a model (`ifc::cost`, ISO 16739): one `IfcCostSchedule`, of
`PredefinedType` `.PRICEDBILLOFQUANTITIES.` and named in the project's language (Computo metrico), with an `IfcCostItem`
for each article code the elements carry — the code as its `Identification`, the price list's wording as its `Name`, the
unit price as an `IfcCostValue` of category `Unit cost` — and the elements it prices assigned to it by
`IfcRelAssignsToControl`, the items in turn assigned to the schedule. The money is an `IfcMonetaryUnit` in the project's
units, from the price list's `currency` (EUR unless it says otherwise). The code also goes out on each element, in the
`Project Parameters` property set, so a reader can tell where a price came from without the cost items.

**The quantity an item carries follows the price list's own unit**, because only the list knows what its price is for:
`m2`, `mq` or `m²` is the elements' area (a wall's net of its openings), `m3`, `mc` or `m³` their volume (likewise net),
`m` or `ml` their length, and anything else their count — which is always true, so a unit this does not know costs
nothing. A code the price list no longer holds is written with its count and no price, rather than dropped.

A project whose elements carry no code writes none of this. CI prices `ci/ifc-coverage.txt`'s beams from `ci/prices.csv`
(the IFC job's `coverage-bill.ifc`), so IfcOpenShell checks the cost entities against the EXPRESS rules on every run.

## The same numbers as Revit

`crosscheck.rs` holds the numbers schedules show against Revit's ([tools/revit-crosscheck](../../tools/revit-crosscheck/README.md)):
`export_crosscheck` describes a project in plain numbers (walls by their centre lines, floors, shafts, doors and windows,
rooms by their points), a script of ours builds it again in Revit and writes what Revit reports, `kind,id,quantity,value`
by our element ids, and `compare_crosscheck` (and the test `the_small_tower_has_the_numbers_revit_gives_it`, once Revit's
table of the small two-storey tower is committed) compares them quantity by quantity within `crosscheck::tolerance`.
Compared: each room's area, perimeter, whether it is enclosed, its unbounded height and its volume, each wall's length
along its centre line and its doors and windows, each floor's area less its openings and shafts, and its volume. Revit
reports more (a wall's area and volume) than we work out yet: those come back as `only_theirs`, the list of what is still
to do. Where the two define a quantity differently, the difference goes here, with the rule the comparison applies.

- A room's volume is its area times its unbounded height (from its level and base offset up to its upper limit and limit
  offset), which is Revit's while nothing under its top bounds it. Revit also stops a room's volume at a room-bounding
  ceiling, floor or roof lower than its top, and a ceiling bounds rooms unless told not to; we don't yet. The description
  gives Revit each room's limits as ours. The tower's rooms reach 300 mm under the level above (3.7 m, the `room`
  command's default), over their ceilings at 2.7 m: Revit's volume of them is their area times 2.7 m where ours is times
  3.7 m, the one difference the comparison is expected to show until volumes stop at ceilings.
- Each level's computation height (where walls and columns are cut to bound rooms) goes to Revit as the level's.

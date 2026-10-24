# Project parameters and schedules

Moved from the root `CLAUDE.md`, word for word: the root keeps the rules and the map, and this file is read when the area is touched.

A project parameter (`Model::parameters`) is a value each element of its categories may hold in its `params`, of its kind;
a component's `params` hold its family's instance values too, so a parameter for components can't take a family
parameter's name, nor any category's own field's (`schedule::own_field`). A schedule's fields are its category's own
(`OWN` in `schedule.rs`, read from `element_json`), the project parameters for it and, for components, the families'
parameters, or calculated: a formula over the row's others, read as the family formula engine reads (lengths in mm, areas in
m²), a field with no value leaving it with none (an unset yes or no is no). Filters compare numbers as the schedule shows
them, so two that show the same are equal. The six schedules every project has (`schedule::VIEWS`) aren't elements
until changed or placed (`schedule::standard`), and a schedule named as one of them, in any language or case, is that one; renaming a parameter renames it in schedules, but a formula naming it holds
the rename back.

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

What is still missing is the join: an element does not yet find its price item by itself, so a schedule's calculated field
is where a price meets a quantity. Giving an element its article code (a project parameter on the type, or a filter per
item) is the next step, and `IfcCostSchedule` with its `IfcCostItem`s is how a finished bill of quantities should leave
the project.

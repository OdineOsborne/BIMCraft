# bimcraft-dwg

The bridge that turns a DWG into DXF bytes, which [`bimcraft-core`](../core/CLAUDE.md)'s own reader then reads. Policy
comes first, as it does for the Revit reader: DWG is Autodesk's own format and has no published specification, so
BIMCraft does not read one itself. The area write-up is [CAD drawings](../../docs/areas/cad-drawings.md).

- **The dependency lives here and nowhere else.** `acadrust` is MPL-2.0 and is used **unmodified**, which leaves
  BIMCraft's own files MIT OR Apache-2.0 (`NOTICE` says so). Modifying its files would put those files under the MPL, so
  don't: anything this crate needs differently is wrapped here, on this side of it.
- **Bytes in, bytes out.** Nothing of the dependency's types appears in this crate's public interface, so what reads a
  drawing stays one reader (`bimcraft_core::dxf`) and what depends on `acadrust` stays one crate. Do not re-export its
  document, its entities or its errors.
- **The core never depends on this.** An application hands the converter to the session (`Session::dwg`), behind its own
  `dwg` feature, as the desktop hands over the Revit reader; a build made `--no-default-features` refuses a DWG by name.
- **The decoder is guarded from the outside**, because it sizes its tables from what a file claims and a file may be
  damaged or hostile: a DWG past `Limits::dwg` never reaches it, the DXF it writes is cut off at `Limits::dxf` while it
  is written, and both directions run inside `catch_unwind`. What this does not bound — a small file declaring a huge
  table — is written down in the crate's own documentation; fixing it needs a worker process with an OS memory limit,
  not a bigger number here.
- A real drawing is not in this repository: `BIMCRAFT_DWG_SAMPLE` names one for the ignored test that converts it and
  reads it back, as `BIMCRAFT_RVT_SAMPLES` does for Revit. `cargo test -p bimcraft-dwg` runs in a moment.

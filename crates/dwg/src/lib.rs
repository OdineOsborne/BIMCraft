//! DWG through a DXF bridge: a drawing in Autodesk's own format turned into the DXF bytes BIMCraft's own reader
//! ([`bimcraft_core::dxf`]) already understands. Nothing of the dependency's types leaves this crate — the whole of its
//! surface here is bytes in, bytes out — so what reads a drawing stays one reader, and what is depended on stays in one
//! place.
//!
//! **Why a dependency at all.** DWG has no published specification, so it cannot be read under the rule the rest of
//! BIMCraft's formats are read under. `acadrust` is MPL-2.0, a copyleft of each file rather than of a whole work: used
//! unmodified, as it is here, BIMCraft's own files stay `MIT OR Apache-2.0`, and `NOTICE` says where it came from.
//! LibreDWG, the other open reader, is GPL-3 and could not be.
//!
//! **The guard around it.** The decoder sizes its tables and buffers from what a file claims, and a file may be damaged
//! or hostile:
//!
//! - a file bigger than [`Limits::dwg`] is refused before the decoder is handed anything;
//! - the DXF it writes is cut off at [`Limits::dxf`] as it is written, not after;
//! - the whole of it runs inside `catch_unwind`, so a decode that gives up is an error and not the end of the window.
//!
//! What this does not bound: a small file that declares a huge table inside itself can still make the decoder allocate
//! before any size of ours is known. Bounding that needs the decode to run where an operating system can cap its
//! memory — a worker of its own — which is work this does not do yet. The limits here keep what it can cost in reach.

use std::io::{Cursor, Write};
use std::panic::AssertUnwindSafe;

/// How big a drawing may be, in bytes, in each form. A DWG is compressed and the DXF it becomes is several times
/// larger, so the two are not the same number.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The DWG read in.
    pub dwg: usize,
    /// The DXF written out.
    pub dxf: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { dwg: 64 << 20, dxf: 512 << 20 }
    }
}

/// Whether `bytes` begin as a DWG does: `AC10` and two digits, the version code Autodesk writes first.
pub fn is_dwg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"AC10") && bytes.get(4..6).is_some_and(|v| v.iter().all(u8::is_ascii_digit))
}

/// The version code of a DWG — `AC1032` — or none where `bytes` are no DWG.
pub fn version(bytes: &[u8]) -> Option<String> {
    is_dwg(bytes).then(|| String::from_utf8_lossy(&bytes[0..6]).into_owned())
}

/// What the version code of a DWG is called, for saying which drawing could not be read.
pub fn release(code: &str) -> &'static str {
    match code {
        "AC1009" => "AutoCAD R11/R12",
        "AC1012" => "AutoCAD R13",
        "AC1014" => "AutoCAD R14",
        "AC1015" => "AutoCAD 2000",
        "AC1018" => "AutoCAD 2004",
        "AC1021" => "AutoCAD 2007",
        "AC1024" => "AutoCAD 2010",
        "AC1027" => "AutoCAD 2013",
        "AC1032" => "AutoCAD 2018 or later",
        _ => "an unknown release",
    }
}

/// A writer that stops at `cap` bytes, so a drawing that unfolds without end is cut off while it is written and not
/// after it has filled the machine.
struct Capped {
    out: Vec<u8>,
    cap: usize,
}

impl Write for Capped {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.out.len() + buf.len() > self.cap {
            return Err(std::io::Error::other(format!("the DXF it makes is past {} bytes", self.cap)));
        }
        self.out.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// `bytes`, a DWG, as the bytes of an ASCII DXF, and what the reader said about the drawing on the way: how much of it
/// it could not read, in its own words, the first few.
pub fn to_dxf(bytes: &[u8], limits: &Limits) -> Result<(Vec<u8>, Vec<String>), String> {
    let Some(code) = version(bytes) else {
        return Err("this is no DWG: it does not begin with a version code".into());
    };
    if bytes.len() > limits.dwg {
        return Err(format!("it holds {} bytes; a DWG is read up to {}", bytes.len(), limits.dwg));
    }
    let read = std::panic::catch_unwind(AssertUnwindSafe(|| {
        acadrust::DwgReader::from_stream_with_options(Cursor::new(bytes), acadrust::DwgReadOptions::failsafe()).read_with_stats()
    }));
    let outcome = match read {
        Ok(Ok(got)) => got,
        Ok(Err(e)) => return Err(format!("{} ({code}) could not be read: {e}", release(&code))),
        Err(_) => return Err(format!("{} ({code}) stopped the reader", release(&code))),
    };
    let (doc, stats) = (outcome.document, outcome.stats);
    let mut out = Capped { out: Vec::new(), cap: limits.dxf };
    let written = std::panic::catch_unwind(AssertUnwindSafe(|| acadrust::DxfWriter::new(&doc).write_to_writer(&mut out)));
    match written {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(format!("{} ({code}) could not be written as DXF: {e}", release(&code))),
        Err(_) => return Err(format!("{} ({code}) stopped the writer", release(&code))),
    }
    Ok((out.out, notes(&stats)))
}

/// What the reader said it could not read, in its own words, the first few: a drawing says how much of itself it lost
/// on the way in, rather than losing it quietly.
fn notes(stats: &acadrust::ReadStats) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in stats.diagnostics.iter().take(8) {
        out.push(format!("{d:?}"));
    }
    if stats.diagnostics.len() > out.len() {
        out.push(format!("and {} more", stats.diagnostics.len() - out.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_no_dwg_is_said_to_be_none() {
        assert!(!is_dwg(b"0\nSECTION\n"));
        assert!(!is_dwg(b"AC10"));
        assert_eq!(version(b"ACXXXX\0"), None);
        assert!(to_dxf(b"hello", &Limits::default()).is_err());
    }

    #[test]
    fn a_dwg_says_which_release_it_is() {
        assert!(is_dwg(b"AC1032\0\0\0"));
        assert_eq!(version(b"AC1032\0\0\0").as_deref(), Some("AC1032"));
        assert_eq!(release("AC1032"), "AutoCAD 2018 or later");
        assert_eq!(release("AC1015"), "AutoCAD 2000");
        assert_eq!(release("AC9999"), "an unknown release");
    }

    #[test]
    fn a_file_past_the_limit_is_refused_before_the_decoder_sees_it() {
        let big = [b"AC1032".as_slice(), &[0u8; 64]].concat();
        let err = to_dxf(&big, &Limits { dwg: 16, dxf: 1 << 20 }).expect_err("refused");
        assert!(err.contains("70 bytes") && err.contains("up to 16"), "{err}");
    }

    #[test]
    fn a_dwg_that_is_only_a_header_comes_out_as_a_drawing_of_nothing() {
        // The reader is a failsafe one: a file cut short gives back what it could make of it rather than an error, and
        // what it could make of this is a drawing with nothing in it. What matters here is that it is DXF our own
        // reader reads, and that nothing crashed on the way.
        let (dxf, _notes) = to_dxf(b"AC1032\0\0\0\0\0\0\0\0\0\0\0", &Limits::default()).expect("it makes a DXF of it");
        assert!(
            dxf.starts_with(b"  0\r\nSECTION") || dxf.starts_with(b"0\nSECTION"),
            "{:?}",
            String::from_utf8_lossy(&dxf[..40.min(dxf.len())])
        );
        let drawing = bimcraft_core::dxf::read(&dxf, &bimcraft_core::dxf::Options::default()).expect("our own reader reads it");
        assert_eq!(drawing.shapes(), 0, "a header alone draws nothing");
    }

    /// A real drawing, which this repository holds none of: `BIMCRAFT_DWG_SAMPLE` names one, as `BIMCRAFT_RVT_SAMPLES`
    /// names Revit's. `cargo test -p bimcraft-dwg a_real_drawing -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_real_drawing_comes_through_as_a_drawing_our_own_reader_reads() {
        let Ok(path) = std::env::var("BIMCRAFT_DWG_SAMPLE") else {
            eprintln!("no sample: set BIMCRAFT_DWG_SAMPLE to a .dwg");
            return;
        };
        let bytes = std::fs::read(&path).expect("the file");
        let started = std::time::Instant::now();
        let (dxf, notes) = to_dxf(&bytes, &Limits::default()).expect("it converts");
        let took = started.elapsed();
        let drawing = bimcraft_core::dxf::read(&dxf, &bimcraft_core::dxf::Options::default()).expect("our own reader reads it");
        eprintln!("{} ({}): {} bytes of DWG -> {} of DXF in {took:?}", path, version(&bytes).unwrap_or_default(), bytes.len(), dxf.len());
        eprintln!("  unit: {} mm, extent: {:?}", drawing.scale, drawing.extent);
        for layer in &drawing.layers {
            eprintln!("  layer {:<24} {:>6} shapes", layer.name, layer.shapes.len());
        }
        eprintln!("  left out: {:?}", drawing.left_out);
        eprintln!("  the reader's own notes: {notes:?}");
        assert!(drawing.shapes() > 0, "a real drawing holds something this reads");
    }

    #[test]
    fn the_dxf_it_writes_is_cut_off_where_it_is_told_rather_than_filling_the_machine() {
        let err = to_dxf(b"AC1032\0\0\0\0\0\0\0\0\0\0\0", &Limits { dwg: 1 << 20, dxf: 64 }).expect_err("cut off");
        assert!(err.contains("past 64 bytes"), "{err}");
    }
}

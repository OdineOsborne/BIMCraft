//! Linked models: Manage > Link Model asks for a project or an IFC and links it; Reload Links reads the files again.

use std::path::Path;

use egui::Context;
use serde_json::json;

use crate::BimApp;
use crate::lang::{t, tf};

impl BimApp {
    /// Link Model (LK): the file the user picks.
    pub(crate) fn link_model_dialog(&mut self, ctx: &Context) {
        let dialog = rfd::AsyncFileDialog::new().add_filter(t("BIMCraft project or IFC"), &["bimcraft", "ifc"]);
        self.ask_file(ctx, dialog, false, |app, path| app.link_path(&path));
    }

    /// Link CAD (LC): the DXF the user picks, under the plan on screen. A window with no plan open says so, since a
    /// drawing comes under one.
    pub(crate) fn link_cad_dialog(&mut self, ctx: &Context) {
        if self.plan_level().is_none() {
            self.status = crate::Status { text: t("Link CAD: open a plan first").to_string(), error: true };
            return;
        }
        let dialog = rfd::AsyncFileDialog::new().add_filter(t("CAD drawing"), &["dxf"]);
        self.ask_file(ctx, dialog, false, |app, path| {
            let folder = app.session.path.as_deref().and_then(Path::parent);
            let kept = folder.and_then(|dir| path.strip_prefix(dir).ok()).unwrap_or(&path);
            let level = app.plan_level().map(serde_json::Value::from);
            let reply = app.run("underlay", json!({"path": kept.to_string_lossy(), "level": level}));
            if let Some(r) = reply {
                let (shapes, name) = (r["shapes"].as_u64().unwrap_or(0).to_string(), r["name"].as_str().unwrap_or_default().to_string());
                let mut text = tf("{} shapes came in from {}.", &[&shapes, &name]);
                if let Some(left) = r["left_out"].as_array().filter(|l| !l.is_empty()) {
                    let what: Vec<&str> = left.iter().filter_map(serde_json::Value::as_str).collect();
                    text.push(' ');
                    text.push_str(&tf("Left out: {}.", &[&what.join(", ")]));
                }
                app.status = crate::Status { text, error: false };
            }
        });
    }

    /// Links the project at `path`, kept relative to this project's folder where it lies under it, so the two move together.
    pub(crate) fn link_path(&mut self, path: &Path) {
        let folder = self.session.path.as_deref().and_then(Path::parent);
        let kept = folder.and_then(|dir| path.strip_prefix(dir).ok()).unwrap_or(path);
        self.run("link", json!({"path": kept.to_string_lossy()}));
    }
}

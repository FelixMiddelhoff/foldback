// SPDX-License-Identifier: MIT OR Apache-2.0
//! The other half of reflective hashing's visibility tooling (plan §4,
//! alongside `foldback-cli lint`): a live view of exactly what
//! [`super::hash_reflected`] captured on the last call for a given
//! entity — closes the loop between "I tagged this field" and "I can
//! see it's actually being captured," instead of trusting the tag
//! silently worked.
//!
//! [`ReflectionPreview`] is plain data (a `Vec<(String, u64)>` of
//! field-path/hash pairs, as returned by `hash_reflected`) with no Bevy
//! `Resource`/`Component` derive of its own — this crate doesn't depend
//! on full `bevy_ecs`/`bevy_app`, only `bevy_reflect`, so a game wires
//! this into its own `Resource` or `Component` type. [`render`] (feature
//! `bevy-debug-panel`) draws it as an `egui` table a game's own
//! `bevy_egui` integration point can call into directly.

/// One entity's most recently captured field paths and hashes, in the
/// order `hash_reflected` produced them.
#[derive(Debug, Default, Clone)]
pub struct ReflectionPreview {
    pub entity_id: u64,
    pub tick: u64,
    pub fields: Vec<(String, u64)>,
}

impl ReflectionPreview {
    /// Builds a preview from a `hash_reflected` call's own return value —
    /// the same data that was actually recorded, not a re-derived guess.
    pub fn new(entity_id: u64, tick: u64, fields: Vec<(String, u64)>) -> Self {
        ReflectionPreview {
            entity_id,
            tick,
            fields,
        }
    }
}

#[cfg(feature = "bevy-debug-panel")]
pub use panel::render;

#[cfg(feature = "bevy-debug-panel")]
mod panel {
    use super::ReflectionPreview;
    use egui::{Context, Ui, Window};

    /// Draws `preview` as a small table (`field path | hash`) inside an
    /// `egui::Window`. A game embeds this by calling it from wherever it
    /// already runs `egui` (a `bevy_egui::EguiContexts::ctx_mut()`, most
    /// commonly) — this crate doesn't take a `bevy_egui` dependency
    /// itself to stay usable from any `egui`-driven host.
    pub fn render(ctx: &Context, preview: &ReflectionPreview) {
        Window::new(format!("Foldback: entity {}", preview.entity_id)).show(ctx, |ui| {
            render_body(ui, preview);
        });
    }

    fn render_body(ui: &mut Ui, preview: &ReflectionPreview) {
        ui.label(format!("tick {}", preview.tick));
        if preview.fields.is_empty() {
            ui.label("(no tracked fields — check #[foldback(hash)] tags)");
            return;
        }
        egui::Grid::new("foldback_reflection_preview")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("field");
                ui.strong("hash");
                ui.end_row();
                for (path, hash) in &preview.fields {
                    ui.label(path);
                    ui.monospace(format!("{hash:016x}"));
                    ui.end_row();
                }
            });
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Headless: builds a real `egui::Context`, runs one frame, and
        /// draws the panel — proves the widget code actually executes
        /// end-to-end without needing a window or GPU, rather than only
        /// type-checking.
        #[test]
        fn renders_a_frame_without_panicking() {
            let ctx = Context::default();
            let preview = ReflectionPreview::new(
                7,
                42,
                vec![("unit.pos.x".to_string(), 0xdead_beef_u64)],
            );
            let mut output = ctx.run_ui(Default::default(), |ui| {
                render(ui.ctx(), &preview);
            });
            // egui asserts unhandled texture deltas aren't silently
            // dropped — there's no renderer here to hand them to, so
            // clear them explicitly rather than let a headless test
            // panic on cleanup for a reason unrelated to what it checks.
            output.textures_delta.clear();
        }

        #[test]
        fn renders_an_empty_preview_without_panicking() {
            let ctx = Context::default();
            let preview = ReflectionPreview::new(1, 0, vec![]);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                render(ui.ctx(), &preview);
            });
            output.textures_delta.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_carries_the_hash_reflected_output_through_unchanged() {
        let fields = vec![("a".to_string(), 1u64), ("b".to_string(), 2u64)];
        let preview = ReflectionPreview::new(3, 9, fields.clone());
        assert_eq!(preview.entity_id, 3);
        assert_eq!(preview.tick, 9);
        assert_eq!(preview.fields, fields);
    }
}

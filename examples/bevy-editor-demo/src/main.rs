// SPDX-License-Identifier: MIT OR Apache-2.0
//! A real, running `bevy_egui` dock for reflective hashing's visibility
//! tooling (foldback-reflective-hashing.md §4) — closes the gap the
//! standalone `foldback_rs::bevy::debug::render` panel left open: that
//! function draws into any `egui::Context`, headless-tested against one
//! directly, but nothing in this repo actually wired it into a live
//! Bevy `App` with a window before now. This example does exactly that:
//! `EguiPlugin` + one system calling `hash_reflected` each frame on a
//! demo entity, one system drawing the panel from the real
//! `EguiContexts` a running game would use.
//!
//! Run: `cargo run -p bevy-editor-demo`.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use foldback_core::session::Session;
use foldback_core::FoldbackHash;
use foldback_rs::bevy::debug::{render, ReflectionPreview};
use foldback_rs::bevy::hash_reflected;

#[derive(Reflect, FoldbackHash, Clone)]
struct Position {
    #[foldback(hash)]
    x: f32,
    #[foldback(hash)]
    y: f32,
}

#[derive(Component, Reflect, FoldbackHash, Clone)]
struct Unit {
    #[foldback(reflect)]
    pos: Position,
    #[foldback(hash)]
    hp: i32,
    // Deliberately untagged — the dock should never show this row,
    // proving the opt-in boundary holds in a real running app too, not
    // just in the headless unit tests.
    debug_label: String,
}

#[derive(Resource)]
struct FoldbackDemoSession {
    session: Session,
    tick: u64,
}

impl Default for FoldbackDemoSession {
    fn default() -> Self {
        FoldbackDemoSession {
            session: Session::builder().peer_count(1).build().unwrap(),
            tick: 0,
        }
    }
}

#[derive(Resource, Default)]
struct LatestPreview(Option<ReflectionPreview>);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .init_resource::<FoldbackDemoSession>()
        .init_resource::<LatestPreview>()
        .add_systems(Startup, spawn_unit)
        .add_systems(Update, (drift_unit, hash_unit).chain())
        .add_systems(EguiPrimaryContextPass, draw_dock)
        .run();
}

fn spawn_unit(mut commands: Commands) {
    commands.spawn(Unit {
        pos: Position { x: 0.0, y: 0.0 },
        hp: 100,
        debug_label: "not hashed".to_string(),
    });
}

/// Fakes gameplay: moves the demo unit a little every frame, so the
/// dock's hashes visibly change over time instead of sitting static.
fn drift_unit(time: Res<Time>, mut units: Query<&mut Unit>) {
    for mut unit in &mut units {
        unit.pos.x += time.delta_secs();
    }
}

fn hash_unit(
    mut demo: ResMut<FoldbackDemoSession>,
    mut preview: ResMut<LatestPreview>,
    units: Query<(Entity, &Unit)>,
) {
    let FoldbackDemoSession { session, tick } = &mut *demo;
    for (entity, unit) in &units {
        let entity_id = entity.to_bits();
        match hash_reflected(session, *tick, entity_id, "unit", unit) {
            Ok(fields) => preview.0 = Some(ReflectionPreview::new(entity_id, *tick, fields)),
            Err(e) => error!("hash_reflected failed: {e}"),
        }
    }
    *tick += 1;
}

fn draw_dock(mut contexts: EguiContexts, preview: Res<LatestPreview>) -> Result {
    let Some(preview) = &preview.0 else {
        return Ok(());
    };
    render(contexts.ctx_mut()?, preview);
    Ok(())
}

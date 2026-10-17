//! 🖥️ ui.rs — Наэкранный интерфейс (HUD), подсказки управления и счётчик врагов.
//!
//! Экспортирует:
//! - `UiPlugin`: Bevy-плагин спавна и обновления UI элементов.

use bevy::prelude::*;

use crate::enemy::{Enemy, WaveStats};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(Update, update_hud);
    }
}

#[derive(Component)]
struct HudText;

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(
            "oxide-core\nWASD — move | SPACE — dash | LMB — shoot\nEliminated: 0 | Hostiles: 6",
        ),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 1.0, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
}

fn update_hud(
    stats: Res<WaveStats>,
    enemies_q: Query<&Enemy>,
    mut text_q: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = text_q.get_single_mut() else {
        return;
    };
    let hostiles = enemies_q.iter().count();
    **text = format!(
        "oxide-core\nWASD: move | SPACE: dash | LMB: shoot\nEliminated: {} | Hostiles: {}",
        stats.kills, hostiles
    );
}

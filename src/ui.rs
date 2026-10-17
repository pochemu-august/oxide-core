//! 🖥️ ui.rs — Наэкранный интерфейс (HUD) и подсказки управления.
//!
//! Экспортирует:
//! - `UiPlugin`: Bevy-плагин спавна UI элементов.

use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("oxide-core\nWASD — move | SPACE — dash | LMB — shoot"),
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
    ));
}

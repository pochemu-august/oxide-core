//! 📷 camera.rs — Настройка камеры (HDR, Bloom) и плавное следование за игроком.
//!
//! Экспортирует:
//! - `CameraPlugin`: Bevy-плагин камеры.
//! - `MainCamera`: маркерный компонент основной игровой камеры.

use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::GameSet;
use crate::config::{CAMERA_OFFSET, CAMERA_SMOOTHNESS, PLAYER_START};
use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, camera_follow.in_set(GameSet::Camera));
    }
}

/// Маркерный компонент основной 3D-камеры.
#[derive(Component)]
pub struct MainCamera;

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_translation(PLAYER_START + CAMERA_OFFSET).looking_at(PLAYER_START, Vec3::Y),
        MainCamera,
    ));
}

/// Сглаженное следование камеры за игроком (LERP, независимый от частоты кадров).
fn camera_follow(
    time: Res<Time>,
    player_q: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let (Ok(player), Ok(mut camera)) = (player_q.get_single(), camera_q.get_single_mut()) else {
        return;
    };
    let target = player.translation + CAMERA_OFFSET;
    let t = 1.0 - (-CAMERA_SMOOTHNESS * time.delta_secs()).exp();
    camera.translation = camera.translation.lerp(target, t);
}

//! 🎯 aim.rs — Прицеливание курсором мыши через Raycast, поворот игрока и отрисовка лазера/прицела.
//!
//! Экспортирует:
//! - `AimPlugin`: Bevy-плагин системы прицеливания.
//! - `AimPoint`: ресурс с текущей 3D-точкой прицеливания на плоскости.

use bevy::math::Isometry3d;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::FRAC_PI_2;

use crate::GameSet;
use crate::camera::MainCamera;
use crate::config::{AIM_PLANE_Y, NOSE_TIP_LOCAL};
use crate::player::Player;

pub struct AimPlugin;

impl Plugin for AimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AimPoint>()
            .add_systems(
                Update,
                (update_aim, rotate_player_to_aim)
                    .chain()
                    .in_set(GameSet::Aim),
            )
            .add_systems(Update, draw_aim_gizmos.in_set(GameSet::Vfx));
    }
}

/// Мировая позиция точки прицеливания (пересечение луча из курсора с горизонтальной плоскостью).
#[derive(Resource, Default)]
pub struct AimPoint(pub Option<Vec3>);

/// Рассчитывает 3D-точку пересечения луча курсора мыши с плоскостью y = `AIM_PLANE_Y`.
fn update_aim(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &Transform), With<MainCamera>>,
    mut aim: ResMut<AimPoint>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_tf)) = camera_q.get_single() else {
        return;
    };

    // Строим GlobalTransform напрямую из локального Transform камеры,
    // чтобы исключить 1-кадровое отставание и устранить микро-дрожание прицела при движении.
    let camera_global = GlobalTransform::from(*camera_tf);

    // В Bevy 0.15 viewport_to_world возвращает Result<Ray3d, ...>
    let Ok(ray) = camera.viewport_to_world(&camera_global, cursor) else {
        return;
    };

    // Пересечение луча с бесконечной плоскостью на высоте выстрела
    let plane_origin = Vec3::new(0.0, AIM_PLANE_Y, 0.0);
    let Some(distance) = ray.intersect_plane(plane_origin, InfinitePlane3d::new(Vec3::Y)) else {
        return;
    };

    aim.0 = Some(ray.get_point(distance));
}

/// Поворачивает игрока лицом к точке прицеливания по оси Y.
fn rotate_player_to_aim(aim: Res<AimPoint>, mut player_q: Query<&mut Transform, With<Player>>) {
    let (Some(point), Ok(mut transform)) = (aim.0, player_q.get_single_mut()) else {
        return;
    };
    let target = Vec3::new(point.x, transform.translation.y, point.z);
    if target.distance_squared(transform.translation) > 0.01 {
        transform.look_at(target, Vec3::Y);
    }
}

/// Визуализация лазерного луча прицеливания и перекрестия на земле с помощью Bevy Gizmos.
fn draw_aim_gizmos(
    mut gizmos: Gizmos,
    aim: Res<AimPoint>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Ok(transform) = player_q.get_single() else {
        return;
    };
    let Some(point) = aim.0 else {
        return;
    };

    let flat = Quat::from_rotation_x(FRAC_PI_2);
    let muzzle = transform.transform_point(NOSE_TIP_LOCAL);

    // Сверхъяркий цвет лазера (HDR > 1.0) даёт свечение Bloom
    let laser = Color::LinearRgba(LinearRgba::rgb(4.0, 0.25, 0.15));
    gizmos.line(muzzle, point, laser);

    // Перекрестие в точке прицела
    gizmos.circle(Isometry3d::new(point, flat), 0.35, laser);
    gizmos.line(point - Vec3::X * 0.5, point + Vec3::X * 0.5, laser);
    gizmos.line(point - Vec3::Z * 0.5, point + Vec3::Z * 0.5, laser);

    // Проекция на землю для лучшей оценки глубины
    let ground = Vec3::new(point.x, 0.03, point.z);
    gizmos.circle(
        Isometry3d::new(ground, flat),
        0.2,
        Color::srgba(1.0, 0.3, 0.2, 0.5),
    );
}

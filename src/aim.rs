//! 🎯 aim.rs — Прицеливание курсором мыши через Raycast, поворот игрока и отрисовка лазера/прицела.
//!
//! Экспортирует:
//! - `AimPlugin`: Bevy-плагин системы прицеливания.
//! - `AimPoint`: ресурс с текущей 3D-точкой прицеливания на плоскости.
//! - `LaserSight`: маркер сущности лазерного луча.
//! - `AimReticle`: маркер сущности перекрестия прицела.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::GameSet;
use crate::camera::MainCamera;
use crate::config::{AIM_PLANE_Y, NOSE_TIP_LOCAL};
use crate::player::Player;

pub struct AimPlugin;

impl Plugin for AimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AimPoint>()
            .add_systems(Startup, setup_aim_visuals)
            .add_systems(
                Update,
                (update_aim, rotate_player_to_aim)
                    .chain()
                    .in_set(GameSet::Aim),
            )
            .add_systems(Update, update_aim_visuals.in_set(GameSet::Vfx));
    }
}

/// Мировая позиция точки прицеливания (пересечение луча из курсора с горизонтальной плоскостью).
#[derive(Resource, Default)]
pub struct AimPoint(pub Option<Vec3>);

/// Маркер для сущности лазерного луча прицела.
#[derive(Component)]
pub struct LaserSight;

/// Маркер для сущности перекрестия прицела на земле.
#[derive(Component)]
pub struct AimReticle;

/// Создаёт сущности лазерного луча и кольца прицела один раз при старте игры.
/// В цикле кадра обновляются исключительно их `Transform` (поворот, позиция и scale).
fn setup_aim_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let laser_mat = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(4.0, 0.25, 0.15)),
        emissive: LinearRgba::rgb(8.0, 0.5, 0.3),
        unlit: true,
        ..default()
    });

    // Единичный кубоид: толщина 0.03x0.03, базовая длина 1.0 вдоль Z.
    // Длина динамически регулируется через transform.scale.z без реаллокации меша.
    let laser_mesh = meshes.add(Cuboid::new(0.03, 0.03, 1.0));

    commands.spawn((
        Mesh3d(laser_mesh),
        MeshMaterial3d(laser_mat.clone()),
        Transform::default(),
        Visibility::Hidden,
        LaserSight,
        NotShadowCaster,
    ));

    // Кольцо / перекрестие прицела на земле
    let reticle_mesh = meshes.add(Torus::new(0.02, 0.25));

    commands.spawn((
        Mesh3d(reticle_mesh),
        MeshMaterial3d(laser_mat),
        Transform::default(),
        Visibility::Hidden,
        AimReticle,
        NotShadowCaster,
    ));
}

/// Рассчитывает 3D-точку пересечения луча курсора мыши с плоскостью y = `AIM_PLANE_Y`.
/// Опрашивает позицию курсора ровно 1 раз за кадр из окна, исключая повторные Unproject при 1000+ Гц мышах.
fn update_aim(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &Transform), With<MainCamera>>,
    mut aim: ResMut<AimPoint>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        if aim.0.is_some() {
            aim.0 = None;
        }
        return;
    };
    let Ok((camera, camera_tf)) = camera_q.get_single() else {
        return;
    };

    // Строим GlobalTransform напрямую из локального Transform камеры,
    // чтобы исключить 1-кадровое отставание и устранить микро-дрожание прицела при движении.
    let camera_global = GlobalTransform::from(*camera_tf);

    // В Bevy 0.15 viewport_to_world возвращает Result<Ray3d, ...>
    let Ok(ray) = camera.viewport_to_world(&camera_global, cursor_pos) else {
        return;
    };

    // Пересечение луча с бесконечной плоскостью на высоте выстрела
    let plane_origin = Vec3::new(0.0, AIM_PLANE_Y, 0.0);
    let Some(distance) = ray.intersect_plane(plane_origin, InfinitePlane3d::new(Vec3::Y)) else {
        return;
    };

    let new_point = ray.get_point(distance);
    if aim.0 != Some(new_point) {
        aim.0 = Some(new_point);
    }
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

/// Оптимизированное обновление лазерного луча и прицела:
/// Меняются исключительно `Transform` и `Visibility` предсозданных сущностей.
/// Никаких аллокаций мешей и материалов в Update!
fn update_aim_visuals(
    aim: Res<AimPoint>,
    player_q: Query<&Transform, With<Player>>,
    mut laser_q: Query<
        (&mut Transform, &mut Visibility),
        (With<LaserSight>, Without<AimReticle>, Without<Player>),
    >,
    mut reticle_q: Query<
        (&mut Transform, &mut Visibility),
        (With<AimReticle>, Without<LaserSight>, Without<Player>),
    >,
) {
    let Ok((mut laser_tf, mut laser_vis)) = laser_q.get_single_mut() else {
        return;
    };
    let Ok((mut reticle_tf, mut reticle_vis)) = reticle_q.get_single_mut() else {
        return;
    };

    let (Some(point), Ok(player_tf)) = (aim.0, player_q.get_single()) else {
        if *laser_vis != Visibility::Hidden {
            *laser_vis = Visibility::Hidden;
        }
        if *reticle_vis != Visibility::Hidden {
            *reticle_vis = Visibility::Hidden;
        }
        return;
    };

    let muzzle = player_tf.transform_point(NOSE_TIP_LOCAL);
    let diff = point - muzzle;
    let distance = diff.length();

    if distance > 0.05 {
        laser_tf.translation = (muzzle + point) * 0.5;
        laser_tf.look_at(point, Vec3::Y);
        laser_tf.scale = Vec3::new(1.0, 1.0, distance);
        if *laser_vis != Visibility::Visible {
            *laser_vis = Visibility::Visible;
        }
    } else if *laser_vis != Visibility::Hidden {
        *laser_vis = Visibility::Hidden;
    }

    reticle_tf.translation = Vec3::new(point.x, 0.03, point.z);
    if *reticle_vis != Visibility::Visible {
        *reticle_vis = Visibility::Visible;
    }
}

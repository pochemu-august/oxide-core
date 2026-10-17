//! 💥 combat.rs — Логика стрельбы, полёт снарядов и регистрация попаданий в препятствия.
//!
//! Экспортирует:
//! - `CombatPlugin`: Bevy-плагин боевой системы.
//! - `Projectile`: компонент снаряда.
//! - `CombatAssets`: меш и светящийся материал снаряда.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::GameSet;
use crate::aim::AimPoint;
use crate::arena::Obstacle;
use crate::config::{
    ENEMY_RADIUS, ENEMY_SIZE, FIRE_COOLDOWN, NOSE_TIP_LOCAL, PROJECTILE_DAMAGE,
    PROJECTILE_LIFETIME, PROJECTILE_RADIUS, PROJECTILE_SPEED,
};
use crate::enemy::{Enemy, Health, WaveStats};
use crate::player::Player;
use crate::vfx::{SparkKind, VfxAssets, spawn_death_chunks, spawn_sparks};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_combat_assets).add_systems(
            Update,
            (player_shoot, update_projectiles)
                .chain()
                .in_set(GameSet::Combat),
        );
    }
}

/// Компонент снаряда в полёте.
#[derive(Component)]
pub struct Projectile {
    pub velocity: Vec3,
    pub lifetime_left: f32,
}

#[derive(Resource)]
pub struct CombatAssets {
    pub mesh: Handle<Mesh>,
    pub mat: Handle<StandardMaterial>,
}

fn setup_combat_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = CombatAssets {
        mesh: meshes.add(
            Sphere::new(PROJECTILE_RADIUS)
                .mesh()
                .ico(1)
                .expect("Failed to create ico sphere"),
        ),
        mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.2),
            emissive: LinearRgba::rgb(18.0, 12.0, 1.5),
            ..default()
        }),
    };
    commands.insert_resource(assets);
}

/// Проверка пересечения отрезка [p0, p1] с AABB препятствия (с учетом радиуса снаряда).
/// Реализует Continuous Collision Detection (CCD), исключающий прохождение сквозь стены на высокой скорости.
#[must_use]
fn segment_intersects_aabb(
    p0: Vec3,
    p1: Vec3,
    box_center: Vec3,
    box_half: Vec3,
    radius: f32,
) -> Option<Vec3> {
    let r = Vec3::splat(radius);
    let min = box_center - box_half - r;
    let max = box_center + box_half + r;

    // Быстрый отсев: если охватывающий объём отрезка не пересекается с AABB
    let seg_min = p0.min(p1);
    let seg_max = p0.max(p1);
    if seg_min.x > max.x
        || seg_max.x < min.x
        || seg_min.y > max.y
        || seg_max.y < min.y
        || seg_min.z > max.z
        || seg_max.z < min.z
    {
        return None;
    }

    // Если начальная точка уже находится внутри AABB
    if p0.x >= min.x
        && p0.x <= max.x
        && p0.y >= min.y
        && p0.y <= max.y
        && p0.z >= min.z
        && p0.z <= max.z
    {
        return Some(p0);
    }

    let d = p1 - p0;
    let mut tmin = 0.0_f32;
    let mut tmax = 1.0_f32;

    for i in 0..3 {
        let (p0_axis, d_axis, min_axis, max_axis) = match i {
            0 => (p0.x, d.x, min.x, max.x),
            1 => (p0.y, d.y, min.y, max.y),
            _ => (p0.z, d.z, min.z, max.z),
        };

        if d_axis.abs() < 1e-7 {
            if p0_axis < min_axis || p0_axis > max_axis {
                return None;
            }
        } else {
            let inv_d = 1.0 / d_axis;
            let mut t1 = (min_axis - p0_axis) * inv_d;
            let mut t2 = (max_axis - p0_axis) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }

    if tmin <= 1.0 && tmax >= 0.0 {
        Some(p0 + d * tmin.max(0.0))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn player_shoot(
    mut commands: Commands,
    aim: Res<AimPoint>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    combat_assets: Res<CombatAssets>,
    vfx_assets: Res<VfxAssets>,
    mut player_q: Query<(&mut Player, &Transform)>,
    obstacles: Query<(&Transform, &Obstacle), Without<Player>>,
) {
    let Ok((mut player, transform)) = player_q.get_single_mut() else {
        return;
    };
    player.fire_cooldown_left = (player.fire_cooldown_left - time.delta_secs()).max(0.0);

    // 1. Блокируем стрельбу при выходе курсора мыши за пределы игрового окна
    if aim.0.is_none() {
        return;
    }

    if !mouse.pressed(MouseButton::Left) || player.fire_cooldown_left > 0.0 {
        return;
    }
    player.fire_cooldown_left = FIRE_COOLDOWN;

    let dir = transform.forward().as_vec3();
    let player_center = transform.translation;
    let muzzle = transform.transform_point(NOSE_TIP_LOCAL);

    // 2. Защита от стрельбы в упор: если дуло касается или вошло в стену,
    // снаряд детонирует на поверхности, не проникая насквозь
    let mut hit_point_blank = None;
    for (obs_tf, obs) in &obstacles {
        if let Some(hit) = segment_intersects_aabb(
            player_center,
            muzzle,
            obs_tf.translation,
            obs.half_extents,
            PROJECTILE_RADIUS,
        ) {
            hit_point_blank = Some(hit);
            break;
        }
    }

    let seed = (time.elapsed_secs() * 7919.0) as u32;

    if let Some(hit) = hit_point_blank {
        spawn_sparks(
            &mut commands,
            &vfx_assets,
            SparkKind::Hit,
            hit,
            4,
            5.0,
            seed,
        );
        return;
    }

    commands.spawn((
        Mesh3d(combat_assets.mesh.clone()),
        MeshMaterial3d(combat_assets.mat.clone()),
        Transform::from_translation(muzzle),
        Projectile {
            velocity: dir * PROJECTILE_SPEED,
            lifetime_left: PROJECTILE_LIFETIME,
        },
        NotShadowCaster,
    ));

    // Короткая дульная вспышка
    spawn_sparks(
        &mut commands,
        &vfx_assets,
        SparkKind::Hit,
        muzzle,
        3,
        3.0,
        seed,
    );
}

fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: Res<VfxAssets>,
    mut stats: ResMut<WaveStats>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    obstacles: Query<(&Transform, &Obstacle), Without<Projectile>>,
    mut enemies: Query<(Entity, &Transform, &mut Health), (With<Enemy>, Without<Projectile>)>,
) {
    let dt = time.delta_secs();
    for (proj_entity, mut transform, mut projectile) in &mut projectiles {
        let p0 = transform.translation;
        let p1 = p0 + projectile.velocity * dt;
        projectile.lifetime_left -= dt;

        if projectile.lifetime_left <= 0.0 {
            commands.entity(proj_entity).despawn();
            continue;
        }

        // 1. Непрерывная проверка попадания в препятствия (CCD через отрезок [p0, p1])
        let mut hit_obstacle = None;
        for (obs_tf, obs) in &obstacles {
            if let Some(hit_pos) = segment_intersects_aabb(
                p0,
                p1,
                obs_tf.translation,
                obs.half_extents,
                PROJECTILE_RADIUS,
            ) {
                hit_obstacle = Some(hit_pos);
                break;
            }
        }

        if let Some(hit_pos) = hit_obstacle {
            commands.entity(proj_entity).despawn();
            let seed = proj_entity.index().wrapping_mul(2_654_435_761);
            spawn_sparks(
                &mut commands,
                &vfx_assets,
                SparkKind::Hit,
                hit_pos,
                4,
                6.0,
                seed,
            );
            continue;
        }

        // 2. Проверка попадания по врагам
        let mut hit_enemy = false;
        for (enemy_entity, enemy_tf, mut health) in &mut enemies {
            let e_pos = enemy_tf.translation;
            let d_xz = Vec2::new(p1.x - e_pos.x, p1.z - e_pos.z).length();
            let d_y = (p1.y - e_pos.y).abs();

            if d_xz <= (ENEMY_RADIUS + PROJECTILE_RADIUS)
                && d_y <= (ENEMY_SIZE.y / 2.0 + PROJECTILE_RADIUS)
            {
                hit_enemy = true;
                commands.entity(proj_entity).despawn();

                // Искры попадания
                let seed = proj_entity.index().wrapping_mul(2_654_435_761);
                spawn_sparks(&mut commands, &vfx_assets, SparkKind::Hit, p1, 4, 5.0, seed);

                // Нанесение урона
                health.current -= PROJECTILE_DAMAGE;
                if health.current <= 0.0 {
                    commands.entity(enemy_entity).despawn_recursive();
                    stats.kills += 1;

                    // Эффект смерти: разлёт кубических осколков во все стороны
                    let death_seed = enemy_entity.index().wrapping_mul(3_141_592_653);
                    spawn_death_chunks(&mut commands, &vfx_assets, e_pos, 8, death_seed);
                }
                break;
            }
        }

        if hit_enemy {
            continue;
        }

        transform.translation = p1;
    }
}

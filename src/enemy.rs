//! 👾 enemy.rs — Враги, ИИ преследования игрока, здоровье и спавн.
//!
//! Экспортирует:
//! - `EnemyPlugin`: Bevy-плагин врагов.
//! - `Enemy`: маркерный компонент врага.
//! - `Health`: компонент текущего и максимального здоровья.
//! - `WaveStats`: счетчик поверженных и активных врагов.

use bevy::prelude::*;

use crate::GameSet;
use crate::arena::Obstacle;
use crate::config::{
    ARENA_HALF, COLOR_ENEMY, COLOR_ENEMY_EYE, ENEMY_HEALTH, ENEMY_RADIUS, ENEMY_SIZE,
    ENEMY_SPAWN_INTERVAL, ENEMY_SPEED, MAX_ENEMIES,
};
use crate::physics::Velocity;
use crate::player::Player;
use crate::vfx::hash01;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaveStats::default())
            .insert_resource(SpawnerTimer(Timer::from_seconds(
                ENEMY_SPAWN_INTERVAL,
                TimerMode::Repeating,
            )))
            .add_systems(Startup, (setup_enemy_assets, spawn_initial_enemies).chain())
            .add_systems(
                Update,
                (enemy_ai, periodic_spawner)
                    .chain()
                    .in_set(GameSet::Movement),
            )
            .add_systems(Update, resolve_enemy_collisions.in_set(GameSet::Physics));
    }
}

/// Маркерный компонент врага.
#[derive(Component)]
pub struct Enemy;

/// Компонент здоровья сущности.
#[derive(Component, Clone, Copy, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    #[must_use]
    pub const fn new(amount: f32) -> Self {
        Self {
            current: amount,
            max: amount,
        }
    }
}

/// Глобальная статистика уничтоженных и появившихся врагов.
#[derive(Resource, Default)]
pub struct WaveStats {
    pub kills: u32,
    pub spawned_count: u32,
}

#[derive(Resource)]
struct SpawnerTimer(Timer);

#[derive(Resource)]
pub struct EnemyAssets {
    body_mesh: Handle<Mesh>,
    body_mat: Handle<StandardMaterial>,
    eye_mesh: Handle<Mesh>,
    eye_mat: Handle<StandardMaterial>,
}

fn setup_enemy_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = EnemyAssets {
        body_mesh: meshes.add(Cuboid::from_size(ENEMY_SIZE)),
        body_mat: materials.add(StandardMaterial {
            base_color: COLOR_ENEMY,
            perceptual_roughness: 0.85,
            ..default()
        }),
        eye_mesh: meshes.add(Cuboid::new(0.5, 0.2, 0.2)),
        eye_mat: materials.add(StandardMaterial {
            base_color: COLOR_ENEMY_EYE,
            emissive: LinearRgba::rgb(14.0, 10.0, 1.0),
            ..default()
        }),
    };
    commands.insert_resource(assets);
}

/// Спавнит одного врага с телом, светящимся визором и коллайдером.
pub fn spawn_single_enemy(commands: &mut Commands, assets: &EnemyAssets, position: Vec3) -> Entity {
    commands
        .spawn((
            Mesh3d(assets.body_mesh.clone()),
            MeshMaterial3d(assets.body_mat.clone()),
            Transform::from_translation(position),
            Enemy,
            Health::new(ENEMY_HEALTH),
            Velocity::default(),
        ))
        .with_children(|parent| {
            // Глаз / визор направления взгляда врага (вперед в Bevy -Z)
            parent.spawn((
                Mesh3d(assets.eye_mesh.clone()),
                MeshMaterial3d(assets.eye_mat.clone()),
                Transform::from_xyz(0.0, 0.25, -0.45),
            ));
        })
        .id()
}

fn spawn_initial_enemies(
    mut commands: Commands,
    assets: Res<EnemyAssets>,
    mut stats: ResMut<WaveStats>,
) {
    // Стартовая группа врагов в разных квадрантах арены
    let spawn_points = [
        Vec3::new(14.0, ENEMY_SIZE.y / 2.0, -18.0),
        Vec3::new(-16.0, ENEMY_SIZE.y / 2.0, 16.0),
        Vec3::new(-22.0, ENEMY_SIZE.y / 2.0, -14.0),
        Vec3::new(20.0, ENEMY_SIZE.y / 2.0, 20.0),
        Vec3::new(0.0, ENEMY_SIZE.y / 2.0, 32.0),
        Vec3::new(32.0, ENEMY_SIZE.y / 2.0, 0.0),
    ];

    for pos in spawn_points {
        spawn_single_enemy(&mut commands, &assets, pos);
        stats.spawned_count += 1;
    }
}

/// Простой ИИ: враги непрерывно поворачиваются к игроку и преследуют его.
fn enemy_ai(
    player_q: Query<&Transform, With<Player>>,
    mut enemies_q: Query<(&mut Transform, &mut Velocity), (With<Enemy>, Without<Player>)>,
) {
    let Ok(player_tf) = player_q.get_single() else {
        return;
    };
    let player_pos = player_tf.translation;

    for (mut enemy_tf, mut vel) in &mut enemies_q {
        let diff = player_pos - enemy_tf.translation;
        let dir = Vec3::new(diff.x, 0.0, diff.z).normalize_or_zero();

        vel.0 = dir * ENEMY_SPEED;

        // Поворот врага лицом к игроку
        let target = Vec3::new(player_pos.x, enemy_tf.translation.y, player_pos.z);
        if target.distance_squared(enemy_tf.translation) > 0.01 {
            enemy_tf.look_at(target, Vec3::Y);
        }
    }
}

/// Периодическое пополнение рядов врагов до `MAX_ENEMIES`.
fn periodic_spawner(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    mut timer: ResMut<SpawnerTimer>,
    mut stats: ResMut<WaveStats>,
    enemies_q: Query<&Enemy>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let current_count = enemies_q.iter().count();
    if current_count >= MAX_ENEMIES {
        return;
    }

    let seed = stats.spawned_count.wrapping_mul(1_337_761);
    let angle = hash01(seed) * std::f32::consts::TAU;
    let dist = 28.0 + hash01(seed ^ 0x99AA) * 12.0;

    let x = (angle.cos() * dist).clamp(-ARENA_HALF + 4.0, ARENA_HALF - 4.0);
    let z = (angle.sin() * dist).clamp(-ARENA_HALF + 4.0, ARENA_HALF - 4.0);
    let pos = Vec3::new(x, ENEMY_SIZE.y / 2.0, z);

    spawn_single_enemy(&mut commands, &assets, pos);
    stats.spawned_count += 1;
}

/// Коллизии врагов с препятствиями (AABB) арены.
fn resolve_enemy_collisions(
    mut enemies_q: Query<(&mut Transform, &mut Velocity), With<Enemy>>,
    obstacles: Query<(&Transform, &Obstacle), (Without<Enemy>, Without<Player>)>,
) {
    for (mut transform, mut velocity) in &mut enemies_q {
        for (obs_tf, obs) in &obstacles {
            let c = obs_tf.translation;
            let h = obs.half_extents;
            let p = transform.translation;

            let closest = Vec2::new(
                p.x.clamp(c.x - h.x, c.x + h.x),
                p.z.clamp(c.z - h.z, c.z + h.z),
            );
            let delta = Vec2::new(p.x, p.z) - closest;
            let dist_sq = delta.length_squared();
            if dist_sq >= ENEMY_RADIUS * ENEMY_RADIUS {
                continue;
            }

            let (normal, push) = if dist_sq > 1e-8 {
                let dist = dist_sq.sqrt();
                (delta / dist, ENEMY_RADIUS - dist)
            } else {
                let dx = h.x - (p.x - c.x).abs();
                let dz = h.z - (p.z - c.z).abs();
                if dx < dz {
                    (Vec2::new((p.x - c.x).signum(), 0.0), dx + ENEMY_RADIUS)
                } else {
                    (Vec2::new(0.0, (p.z - c.z).signum()), dz + ENEMY_RADIUS)
                }
            };

            transform.translation.x += normal.x * push;
            transform.translation.z += normal.y * push;

            let mut v = Vec2::new(velocity.0.x, velocity.0.z);
            let into = v.dot(normal);
            if into < 0.0 {
                v -= normal * into;
                velocity.0.x = v.x;
                velocity.0.z = v.y;
            }
        }

        let limit = ARENA_HALF - ENEMY_RADIUS;
        transform.translation.x = transform.translation.x.clamp(-limit, limit);
        transform.translation.z = transform.translation.z.clamp(-limit, limit);
    }
}

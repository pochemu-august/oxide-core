//! 🎮 player.rs — Спавн игрока, ввод движения WASD, механика рывка и индикатор кулдауна.
//!
//! Экспортирует:
//! - `PlayerPlugin`: Bevy-плагин игрока.
//! - `Player`: компонент состояния игрока (таймеры рывка, направление, кулдаун выстрела, i-frames).
//! - `PlayerAssets`: меш и материалы игрока.

use bevy::math::Isometry3d;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::config::{
    ACCELERATION, COLOR_PLAYER, COLOR_PLAYER_HURT, DASH_COOLDOWN, DASH_DURATION, DASH_EXIT_SPEED,
    DASH_SPEED, ENEMY_ATTACK_DAMAGE, ENEMY_RADIUS, ENEMY_SIZE, FRICTION, MOVE_SPEED, NOSE_LOCAL,
    NOSE_SIZE, PLAYER_INVULN_AFTER_HIT, PLAYER_MAX_HEALTH, PLAYER_RADIUS, PLAYER_SIZE,
    PLAYER_START,
};
use crate::enemy::{Enemy, Health};
use crate::physics::{Velocity, move_towards};
use crate::vfx::{SparkKind, VfxAssets, spawn_dash_ghost, spawn_sparks};
use crate::{AppState, GameSet};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player)
            .add_systems(Update, player_movement.in_set(GameSet::Movement))
            .add_systems(Update, player_enemy_damage.in_set(GameSet::Combat))
            .add_systems(Update, draw_dash_ring.in_set(GameSet::Vfx));
    }
}

/// Состояние игрока и кулдауны способностей.
#[derive(Component, Default)]
pub struct Player {
    pub dash_time_left: f32,
    pub dash_cooldown_left: f32,
    pub dash_dir: Vec3,
    pub fire_cooldown_left: f32,
    pub invuln_timer: f32,
    pub ghost_timer: f32,
}

#[derive(Resource)]
pub struct PlayerAssets {
    pub mesh: Handle<Mesh>,
    pub mat_normal: Handle<StandardMaterial>,
    pub mat_dash: Handle<StandardMaterial>,
    pub mat_hurt: Handle<StandardMaterial>,
}

fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = PlayerAssets {
        mesh: meshes.add(Cuboid::from_size(PLAYER_SIZE)),
        mat_normal: materials.add(StandardMaterial {
            base_color: COLOR_PLAYER,
            perceptual_roughness: 0.8,
            ..default()
        }),
        mat_dash: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.9, 1.0),
            emissive: LinearRgba::rgb(2.0, 6.0, 12.0),
            ..default()
        }),
        mat_hurt: materials.add(StandardMaterial {
            base_color: COLOR_PLAYER_HURT,
            emissive: LinearRgba::rgb(8.0, 1.0, 1.0),
            ..default()
        }),
    };

    commands
        .spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.mat_normal.clone()),
            Transform::from_translation(PLAYER_START),
            Player::default(),
            Health::new(PLAYER_MAX_HEALTH),
            Velocity::default(),
        ))
        .with_children(|parent| {
            // "Нос" — визуальный маркер ориентации взгляда
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(NOSE_SIZE))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.9, 0.95, 1.0),
                    emissive: LinearRgba::rgb(0.6, 0.8, 1.5),
                    ..default()
                })),
                Transform::from_translation(NOSE_LOCAL),
            ));
        });

    commands.insert_resource(assets);
}

fn player_movement(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    player_assets: Res<PlayerAssets>,
    vfx_assets: Res<VfxAssets>,
    mut query: Query<(
        &mut Player,
        &mut Velocity,
        &Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Ok((mut player, mut velocity, transform, mut material)) = query.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs();

    // WASD: камера направлена вдоль -Z, поэтому "вверх" по экрану соответствует -Z в мировых координатах
    let mut wish = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish.x += 1.0;
    }
    let wish = wish.normalize_or_zero();

    player.dash_cooldown_left = (player.dash_cooldown_left - dt).max(0.0);

    // Старт рывка
    if keys.just_pressed(KeyCode::Space)
        && player.dash_cooldown_left <= 0.0
        && player.dash_time_left <= 0.0
    {
        let dir = if wish == Vec3::ZERO {
            let f = transform.forward().as_vec3();
            Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
        } else {
            wish
        };

        player.dash_dir = dir;
        player.dash_time_left = DASH_DURATION;
        player.dash_cooldown_left = DASH_COOLDOWN;
        player.ghost_timer = 0.0;
        material.0 = player_assets.mat_dash.clone();

        let seed = (time.elapsed_secs() * 1000.0) as u32;
        spawn_sparks(
            &mut commands,
            &vfx_assets,
            SparkKind::Dash,
            transform.translation,
            6,
            6.0,
            seed,
        );
    }

    if player.dash_time_left > 0.0 {
        player.dash_time_left -= dt;
        velocity.0 = player.dash_dir * DASH_SPEED;

        // Спавним призрачные следы с разумным интервалом (3-4 на рывок вместо спавна каждый кадр)
        player.ghost_timer += dt;
        if player.ghost_timer >= 0.035 {
            player.ghost_timer = 0.0;
            spawn_dash_ghost(
                &mut commands,
                &vfx_assets,
                player_assets.mesh.clone(),
                transform,
            );
        }

        if player.dash_time_left <= 0.0 {
            // Возвращаем нормальный материал, если игрок не получил урон
            if player.invuln_timer <= 0.0 {
                material.0 = player_assets.mat_normal.clone();
            } else {
                material.0 = player_assets.mat_hurt.clone();
            }
            velocity.0 = player.dash_dir * DASH_EXIT_SPEED;
        }
    } else {
        let target = wish * MOVE_SPEED;
        let rate = if wish == Vec3::ZERO {
            FRICTION
        } else {
            ACCELERATION
        };
        velocity.0 = move_towards(velocity.0, target, rate * dt);
    }
}

/// Проверка контакта врагов с игроком: нанесение урона, неуязвимость при рывке (ULTRAKILL Dash I-Frames) и переход в Game Over.
fn player_enemy_damage(
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: Res<VfxAssets>,
    player_assets: Res<PlayerAssets>,
    mut next_state: ResMut<NextState<AppState>>,
    mut player_q: Query<(
        &Transform,
        &mut Player,
        &mut Health,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    enemies_q: Query<&Transform, With<Enemy>>,
) {
    let Ok((transform, mut player, mut health, mut material)) = player_q.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    if player.invuln_timer > 0.0 {
        player.invuln_timer = (player.invuln_timer - dt).max(0.0);
        if player.invuln_timer <= 0.0 && player.dash_time_left <= 0.0 {
            material.0 = player_assets.mat_normal.clone();
        }
    }

    // Во время рывка (ULTRAKILL I-Frames) либо во время кулдауна неуязвимости урон не проходит
    if player.dash_time_left > 0.0 || player.invuln_timer > 0.0 {
        return;
    }

    let p = transform.translation;
    let hit = enemies_q.iter().any(|enemy_tf| {
        let e = enemy_tf.translation;
        let d_xz = Vec2::new(p.x - e.x, p.z - e.z).length();
        let d_y = (p.y - e.y).abs();
        d_xz <= (PLAYER_RADIUS + ENEMY_RADIUS) && d_y <= (PLAYER_SIZE.y / 2.0 + ENEMY_SIZE.y / 2.0)
    });

    if hit {
        health.current = (health.current - ENEMY_ATTACK_DAMAGE).max(0.0);
        player.invuln_timer = PLAYER_INVULN_AFTER_HIT;
        material.0 = player_assets.mat_hurt.clone();

        let seed = (time.elapsed_secs() * 3333.0) as u32;
        spawn_sparks(&mut commands, &vfx_assets, SparkKind::Hit, p, 8, 6.0, seed);

        if health.current <= 0.0 {
            spawn_sparks(
                &mut commands,
                &vfx_assets,
                SparkKind::Dash,
                p,
                24,
                10.0,
                seed ^ 0xDEAD,
            );
            next_state.set(AppState::GameOver);
        }
    }
}

/// Рисует под ногами игрока кольцо готовности рывка: неоново-голубое — готов, серое — кулдаун.
fn draw_dash_ring(mut gizmos: Gizmos, player_q: Query<(&Transform, &Player)>) {
    let Ok((transform, player)) = player_q.get_single() else {
        return;
    };
    let flat = Quat::from_rotation_x(FRAC_PI_2);
    let feet = Vec3::new(transform.translation.x, 0.03, transform.translation.z);
    let dash_color = if player.dash_cooldown_left <= 0.0 {
        Color::LinearRgba(LinearRgba::rgb(0.3, 1.5, 3.0))
    } else {
        Color::srgba(0.4, 0.4, 0.45, 0.6)
    };
    gizmos.circle(Isometry3d::new(feet, flat), 0.75, dash_color);
}

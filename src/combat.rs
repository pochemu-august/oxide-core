//! 💥 combat.rs — Логика стрельбы, полёт снарядов и регистрация попаданий в препятствия.
//!
//! Экспортирует:
//! - `CombatPlugin`: Bevy-плагин боевой системы.
//! - `Projectile`: компонент снаряда.
//! - `CombatAssets`: меш и светящийся материал снаряда.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::GameSet;
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

fn player_shoot(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    combat_assets: Res<CombatAssets>,
    vfx_assets: Res<VfxAssets>,
    mut player_q: Query<(&mut Player, &Transform)>,
) {
    let Ok((mut player, transform)) = player_q.get_single_mut() else {
        return;
    };
    player.fire_cooldown_left = (player.fire_cooldown_left - time.delta_secs()).max(0.0);
    if !mouse.pressed(MouseButton::Left) || player.fire_cooldown_left > 0.0 {
        return;
    }
    player.fire_cooldown_left = FIRE_COOLDOWN;

    let dir = transform.forward().as_vec3();
    let muzzle = transform.transform_point(NOSE_TIP_LOCAL);

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
    let seed = (time.elapsed_secs() * 7919.0) as u32;
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
        transform.translation += projectile.velocity * dt;
        projectile.lifetime_left -= dt;

        if projectile.lifetime_left <= 0.0 {
            commands.entity(proj_entity).despawn();
            continue;
        }

        let p = transform.translation;

        // 1. Проверка попадания по врагам
        let mut hit_enemy = false;
        for (enemy_entity, enemy_tf, mut health) in &mut enemies {
            let e_pos = enemy_tf.translation;
            let d_xz = Vec2::new(p.x - e_pos.x, p.z - e_pos.z).length();
            let d_y = (p.y - e_pos.y).abs();

            if d_xz <= (ENEMY_RADIUS + PROJECTILE_RADIUS)
                && d_y <= (ENEMY_SIZE.y / 2.0 + PROJECTILE_RADIUS)
            {
                hit_enemy = true;
                commands.entity(proj_entity).despawn();

                // Искры попадания
                let seed = proj_entity.index().wrapping_mul(2_654_435_761);
                spawn_sparks(&mut commands, &vfx_assets, SparkKind::Hit, p, 6, 5.0, seed);

                // Нанесение урона
                health.current -= PROJECTILE_DAMAGE;
                if health.current <= 0.0 {
                    commands.entity(enemy_entity).despawn_recursive();
                    stats.kills += 1;

                    // Эффект смерти: разлёт кубических осколков во все стороны
                    let death_seed = enemy_entity.index().wrapping_mul(3_141_592_653);
                    spawn_death_chunks(&mut commands, &vfx_assets, e_pos, 16, death_seed);
                }
                break;
            }
        }

        if hit_enemy {
            continue;
        }

        // 2. Попадание в препятствие
        let hit_obstacle = obstacles.iter().any(|(obs_tf, obs)| {
            let d = (p - obs_tf.translation).abs();
            let h = obs.half_extents + Vec3::splat(PROJECTILE_RADIUS);
            d.x <= h.x && d.y <= h.y && d.z <= h.z
        });

        if hit_obstacle {
            commands.entity(proj_entity).despawn();
            let seed = proj_entity.index().wrapping_mul(2_654_435_761);
            spawn_sparks(&mut commands, &vfx_assets, SparkKind::Hit, p, 8, 6.0, seed);
        }
    }
}

//! 💥 vfx.rs — Процедурные визуальные эффекты и система частиц.
//!
//! Экспортирует:
//! - `VfxPlugin`: Bevy-плагин обновления частиц.
//! - `VfxAssets`: ресурс с готовыми материалами и мешами для искр и призраков.
//! - `SparkKind`: тип эффекта искр (попадание снаряда / вспышка рывка).
//! - `spawn_sparks(...)`: процедурный спавн искр без внешних крейтов рандома.
//! - `spawn_dash_ghost(...)`: спавн полупрозрачного следа игрока.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::GameSet;
use crate::config::{DASH_GHOST_LIFETIME, SPARK_LIFETIME};

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_vfx_assets)
            .add_systems(Update, update_particles.in_set(GameSet::Vfx));
    }
}

/// Меши и материалы для процедурных визуальных эффектов.
#[derive(Resource)]
pub struct VfxAssets {
    pub spark_mesh: Handle<Mesh>,
    pub spark_mat: Handle<StandardMaterial>,
    pub dash_spark_mat: Handle<StandardMaterial>,
    pub ghost_mat: Handle<StandardMaterial>,
}

/// Тип искр при спавне.
#[derive(Clone, Copy)]
pub enum SparkKind {
    /// Оранжево-жёлтые искры попадания снаряда и дульной вспышки.
    Hit,
    /// Неоново-голубые искры рывка (Dash).
    Dash,
}

/// Компонент простой частицы (летит, подвергается гравитации и угасает по масштабу).
#[derive(Component)]
pub struct Particle {
    pub velocity: Vec3,
    pub gravity: f32,
    pub lifetime_left: f32,
    pub max_lifetime: f32,
    pub start_scale: f32,
}

fn setup_vfx_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = VfxAssets {
        spark_mesh: meshes.add(Cuboid::from_length(0.12)),
        spark_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.7, 0.2),
            emissive: LinearRgba::rgb(12.0, 5.0, 0.5),
            ..default()
        }),
        dash_spark_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.9, 1.0),
            emissive: LinearRgba::rgb(2.0, 7.0, 14.0),
            ..default()
        }),
        ghost_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.7, 1.0, 0.35),
            emissive: LinearRgba::rgb(0.5, 2.0, 5.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };
    commands.insert_resource(assets);
}

/// Быстрый детерминированный целочисленный хэш → [0.0, 1.0].
/// Позволяет генерировать случайные вариации без подключения сторонней зависимости `rand`.
pub fn hash01(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}

/// Спавнит веер искр в указанной точке.
pub fn spawn_sparks(
    commands: &mut Commands,
    vfx: &VfxAssets,
    kind: SparkKind,
    origin: Vec3,
    count: u32,
    speed: f32,
    rng_seed: u32,
) {
    let mat = match kind {
        SparkKind::Hit => vfx.spark_mat.clone(),
        SparkKind::Dash => vfx.dash_spark_mat.clone(),
    };

    for i in 0..count {
        let s = rng_seed.wrapping_add(i.wrapping_mul(97));
        let angle = (i as f32 / count as f32) * TAU + hash01(s) * 0.8;
        let up = 0.5 + hash01(s ^ 0xA5A5) * 1.5;
        let spd = speed * (0.5 + hash01(s ^ 0x5A5A));
        let velocity = Vec3::new(angle.cos() * spd, up * spd * 0.4, angle.sin() * spd);
        let scale = 0.6 + hash01(s ^ 0x1234) * 0.8;

        commands.spawn((
            Mesh3d(vfx.spark_mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(origin).with_scale(Vec3::splat(scale)),
            Particle {
                velocity,
                gravity: 18.0,
                lifetime_left: SPARK_LIFETIME,
                max_lifetime: SPARK_LIFETIME,
                start_scale: scale,
            },
            NotShadowCaster,
        ));
    }
}

/// Спавнит полупрозрачный след ("призрак") игрока при рывке.
pub fn spawn_dash_ghost(
    commands: &mut Commands,
    vfx: &VfxAssets,
    mesh: Handle<Mesh>,
    transform: &Transform,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(vfx.ghost_mat.clone()),
        Transform::from_translation(transform.translation).with_rotation(transform.rotation),
        Particle {
            velocity: Vec3::ZERO,
            gravity: 0.0,
            lifetime_left: DASH_GHOST_LIFETIME,
            max_lifetime: DASH_GHOST_LIFETIME,
            start_scale: 1.0,
        },
        NotShadowCaster,
    ));
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Transform, &mut Particle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut particles {
        particle.lifetime_left -= dt;
        if particle.lifetime_left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        particle.velocity.y -= particle.gravity * dt;
        transform.translation += particle.velocity * dt;
        transform.translation.y = transform.translation.y.max(0.05);

        let k = particle.lifetime_left / particle.max_lifetime;
        transform.scale = Vec3::splat(particle.start_scale * k);
    }
}

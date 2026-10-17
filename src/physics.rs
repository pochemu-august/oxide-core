//! 🏃 physics.rs — Интеграция скорости и обработка коллизий с препятствиями.
//!
//! Экспортирует:
//! - `PhysicsPlugin`: Bevy-плагин физики перемещения.
//! - `Velocity`: компонент 3D-скорости сущности.
//! - `move_towards(...)`: функция линейного разгона/торможения.

use bevy::prelude::*;

use crate::GameSet;
use crate::arena::Obstacle;
use crate::config::{ARENA_HALF, PLAYER_RADIUS};
use crate::player::Player;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (apply_velocity, resolve_player_collisions)
                .chain()
                .in_set(GameSet::Physics),
        );
    }
}

/// Собственный компонент физической скорости без необходимости в тяжелом стороннем движке.
#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

/// Линейно приближает вектор `current` к `target` не более чем на `max_delta`.
/// Даёт максимальную отзывчивость без вялого хвоста классического экспоненциального lerp.
pub fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let diff = target - current;
    let dist = diff.length();
    if dist <= max_delta || dist < f32::EPSILON {
        target
    } else {
        current + diff / dist * max_delta
    }
}

fn apply_velocity(time: Res<Time>, mut query: Query<(&mut Transform, &Velocity)>) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut query {
        transform.translation += velocity.0 * dt;
    }
}

/// Проверка коллизий: круг (игрок) против AABB (препятствие) в плоскости XZ.
/// Обеспечивает плавное скольжение вдоль стен и ящиков без застреваний.
fn resolve_player_collisions(
    mut player_q: Query<(&mut Transform, &mut Velocity), With<Player>>,
    obstacles: Query<(&Transform, &Obstacle), Without<Player>>,
) {
    let Ok((mut transform, mut velocity)) = player_q.get_single_mut() else {
        return;
    };

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
        if dist_sq >= PLAYER_RADIUS * PLAYER_RADIUS {
            continue;
        }

        let (normal, push) = if dist_sq > 1e-8 {
            let dist = dist_sq.sqrt();
            (delta / dist, PLAYER_RADIUS - dist)
        } else {
            // Центр внутри препятствия — выталкиваем по наикратчайшей оси
            let dx = h.x - (p.x - c.x).abs();
            let dz = h.z - (p.z - c.z).abs();
            if dx < dz {
                (Vec2::new((p.x - c.x).signum(), 0.0), dx + PLAYER_RADIUS)
            } else {
                (Vec2::new(0.0, (p.z - c.z).signum()), dz + PLAYER_RADIUS)
            }
        };

        transform.translation.x += normal.x * push;
        transform.translation.z += normal.y * push;

        // Гасим вектор скорости, направленный внутрь стены, для скольжения
        let mut v = Vec2::new(velocity.0.x, velocity.0.z);
        let into = v.dot(normal);
        if into < 0.0 {
            v -= normal * into;
            velocity.0.x = v.x;
            velocity.0.z = v.y;
        }
    }

    // Ограничение границами арены
    let limit = ARENA_HALF - PLAYER_RADIUS;
    transform.translation.x = transform.translation.x.clamp(-limit, limit);
    transform.translation.z = transform.translation.z.clamp(-limit, limit);
}

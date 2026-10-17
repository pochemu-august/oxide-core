//! oxide-core — прототип top-down шутера на Bevy 0.15.
//!
//! Управление:
//!   WASD  — движение
//!   Space — рывок (dash)
//!   LMB   — стрельба (можно зажать)
//!   Мышь  — прицеливание

// Педантичные проверки, ложноположительные для Bevy ECS и геймдев-математики.
// Атрибуты в коде имеют приоритет над флагом `-W clippy::pedantic` из командной строки.
#![allow(
    // Системы Bevy обязаны принимать `Res<T>` / `Query<…>` по значению (иначе ломается `IntoSystem`),
    // а `Handle<T>` — дешёвый clone-able хэндл.
    clippy::needless_pass_by_value,
    // Намеренные касты в расчётах времени и хэшах частиц.
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::math::Isometry3d;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::{FRAC_PI_2, TAU};

// ============================================================================
// КОНСТАНТЫ ДЛЯ ТЮНИНГА
// ============================================================================

// --- Арена ---
const ARENA_SIZE: f32 = 100.0;
const ARENA_HALF: f32 = ARENA_SIZE / 2.0;
const WALL_THICKNESS: f32 = 1.0;
const WALL_HEIGHT: f32 = 1.2;

// --- Игрок ---
const PLAYER_SIZE: Vec3 = Vec3::new(0.8, 1.6, 0.8);
/// Радиус игрока для коллизий (круг в плоскости XZ).
const PLAYER_RADIUS: f32 = 0.5;
/// Локальная позиция "носа" (вперёд в Bevy — это -Z).
const NOSE_LOCAL: Vec3 = Vec3::new(0.0, 0.3, -0.55);
const NOSE_SIZE: Vec3 = Vec3::new(0.25, 0.25, 0.5);
/// Кончик носа — точка, откуда вылетают снаряды.
const NOSE_TIP_LOCAL: Vec3 = Vec3::new(0.0, NOSE_LOCAL.y, NOSE_LOCAL.z - NOSE_SIZE.z / 2.0);

// --- Движение (Ultrakill-like: резко и отзывчиво) ---
const MOVE_SPEED: f32 = 12.0;
/// Ускорение при наличии ввода (ед/с²). Большое значение = почти мгновенный разгон.
const ACCELERATION: f32 = 120.0;
/// Торможение без ввода (ед/с²). Ещё больше — чтобы не было "льда".
const FRICTION: f32 = 160.0;

// --- Рывок ---
const DASH_SPEED: f32 = 42.0;
const DASH_DURATION: f32 = 0.14;
const DASH_COOLDOWN: f32 = 0.5;
/// Скорость, с которой игрок выходит из рывка (сохраняем немного импульса).
const DASH_EXIT_SPEED: f32 = MOVE_SPEED * 1.2;

// --- Стрельба ---
const PROJECTILE_SPEED: f32 = 45.0;
const PROJECTILE_LIFETIME: f32 = 1.5;
const PROJECTILE_RADIUS: f32 = 0.15;
/// Пауза между выстрелами при зажатой ЛКМ.
const FIRE_COOLDOWN: f32 = 0.1;

// --- Прицеливание ---
/// Высота горизонтальной плоскости, на которую проецируется курсор.
/// Берём высоту носа игрока: тогда снаряд летит ровно "под курсором" на экране.
const AIM_PLANE_Y: f32 = PLAYER_SIZE.y / 2.0 + NOSE_LOCAL.y;

// --- Камера ---
/// Постоянный сдвиг камеры относительно игрока.
/// atan(15/10) ≈ 56° — это и есть наклон камеры вниз по оси X.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 15.0, 10.0);
/// Жёсткость следования камеры (больше = быстрее догоняет).
const CAMERA_SMOOTHNESS: f32 = 8.0;

// --- Частицы ---
const DASH_GHOST_LIFETIME: f32 = 0.22;
const SPARK_LIFETIME: f32 = 0.35;

// --- Палитра (low-poly ретро) ---
const COLOR_GROUND: Color = Color::srgb(0.16, 0.16, 0.18);
const COLOR_WALL: Color = Color::srgb(0.09, 0.09, 0.11);
const COLOR_RED: Color = Color::srgb(0.75, 0.12, 0.10);
const COLOR_ORANGE: Color = Color::srgb(0.95, 0.45, 0.08);
const COLOR_PLAYER: Color = Color::srgb(0.15, 0.35, 0.95);
const COLOR_SKY: Color = Color::srgb(0.04, 0.04, 0.06);

/// Описание препятствия на арене.
struct ObstacleDef {
    /// Центр в плоскости XZ.
    pos: Vec2,
    /// Размеры (ширина, высота, глубина).
    size: Vec3,
    orange: bool,
    /// Шестигранная колонна вместо ящика.
    column: bool,
}

const OBSTACLES: &[ObstacleDef] = &[
    ObstacleDef {
        pos: Vec2::new(6.0, -5.0),
        size: Vec3::new(2.0, 1.5, 2.0),
        orange: false,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-8.0, -3.0),
        size: Vec3::new(3.0, 1.0, 1.5),
        orange: true,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-4.0, 9.0),
        size: Vec3::new(1.6, 4.0, 1.6),
        orange: false,
        column: true,
    },
    ObstacleDef {
        pos: Vec2::new(10.0, 8.0),
        size: Vec3::new(1.4, 3.0, 1.4),
        orange: true,
        column: true,
    },
    ObstacleDef {
        pos: Vec2::new(0.0, -14.0),
        size: Vec3::new(8.0, 1.2, 1.0),
        orange: false,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-15.0, -12.0),
        size: Vec3::new(2.5, 2.5, 2.5),
        orange: true,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(16.0, -10.0),
        size: Vec3::new(1.8, 5.0, 1.8),
        orange: false,
        column: true,
    },
    ObstacleDef {
        pos: Vec2::new(18.0, 14.0),
        size: Vec3::new(4.0, 1.0, 4.0),
        orange: true,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-18.0, 12.0),
        size: Vec3::new(1.2, 1.8, 6.0),
        orange: false,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-25.0, 0.0),
        size: Vec3::new(2.0, 6.0, 2.0),
        orange: true,
        column: true,
    },
    ObstacleDef {
        pos: Vec2::new(26.0, 0.0),
        size: Vec3::new(2.0, 6.0, 2.0),
        orange: false,
        column: true,
    },
    ObstacleDef {
        pos: Vec2::new(0.0, 22.0),
        size: Vec3::new(10.0, 0.8, 1.2),
        orange: true,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(30.0, -28.0),
        size: Vec3::new(3.0, 3.0, 3.0),
        orange: false,
        column: false,
    },
    ObstacleDef {
        pos: Vec2::new(-30.0, 30.0),
        size: Vec3::new(3.0, 2.0, 3.0),
        orange: true,
        column: false,
    },
];

// ============================================================================
// КОМПОНЕНТЫ
// ============================================================================

/// Состояние игрока. Таймеры храним как "оставшееся время" в секундах.
#[derive(Component, Default)]
struct Player {
    dash_time_left: f32,
    dash_cooldown_left: f32,
    dash_dir: Vec3,
    fire_cooldown_left: f32,
}

/// Линейная скорость (собственная, без сторонних физических движков).
#[derive(Component, Default, Deref, DerefMut)]
struct Velocity(Vec3);

#[derive(Component)]
struct MainCamera;

/// Статичное препятствие с AABB-коллизией.
#[derive(Component)]
struct Obstacle {
    half_extents: Vec3,
}

#[derive(Component)]
struct Projectile {
    velocity: Vec3,
    lifetime_left: f32,
}

/// Простая частица: летит, (опционально) падает и уменьшается до нуля.
#[derive(Component)]
struct Particle {
    velocity: Vec3,
    gravity: f32,
    lifetime_left: f32,
    max_lifetime: f32,
    start_scale: f32,
}

// ============================================================================
// РЕСУРСЫ
// ============================================================================

/// Точка прицеливания в мире (пересечение луча из курсора с плоскостью).
#[derive(Resource, Default)]
struct AimPoint(Option<Vec3>);

/// Заранее созданные меши и материалы, чтобы не плодить ассеты каждый кадр.
#[derive(Resource)]
struct GameAssets {
    player_mesh: Handle<Mesh>,
    player_mat: Handle<StandardMaterial>,
    player_dash_mat: Handle<StandardMaterial>,
    ghost_mat: Handle<StandardMaterial>,
    projectile_mesh: Handle<Mesh>,
    projectile_mat: Handle<StandardMaterial>,
    spark_mesh: Handle<Mesh>,
    spark_mat: Handle<StandardMaterial>,
    dash_spark_mat: Handle<StandardMaterial>,
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "oxide-core".into(),
                resolution: (1280.0_f32, 720.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(COLOR_SKY))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.6, 0.65, 0.8),
            brightness: 250.0,
        })
        .init_resource::<AimPoint>()
        .add_systems(
            Startup,
            (setup_assets, setup_arena, setup_player_and_camera, setup_ui).chain(),
        )
        .add_systems(
            Update,
            (
                player_movement,
                apply_velocity,
                resolve_player_collisions,
                camera_follow,
                update_aim,
                rotate_player_to_aim,
                player_shoot,
                update_projectiles,
                update_particles,
                draw_aim_gizmos,
            )
                .chain(),
        )
        .run();
}

// ============================================================================
// STARTUP
// ============================================================================

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = GameAssets {
        player_mesh: meshes.add(Cuboid::from_size(PLAYER_SIZE)),
        player_mat: materials.add(StandardMaterial {
            base_color: COLOR_PLAYER,
            perceptual_roughness: 0.8,
            ..default()
        }),
        // Во время рывка игрок "вспыхивает" голубым.
        player_dash_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.9, 1.0),
            emissive: LinearRgba::rgb(2.0, 6.0, 12.0),
            ..default()
        }),
        // Полупрозрачные "призраки" — след рывка.
        ghost_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.7, 1.0, 0.35),
            emissive: LinearRgba::rgb(0.5, 2.0, 5.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        // Низкополигональная сфера — под ретро-стиль.
        projectile_mesh: meshes.add(Sphere::new(PROJECTILE_RADIUS).mesh().ico(1).unwrap()),
        projectile_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.2),
            emissive: LinearRgba::rgb(18.0, 12.0, 1.5),
            ..default()
        }),
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
    };
    commands.insert_resource(assets);
}

fn setup_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // --- Земля 100x100 ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ARENA_SIZE, ARENA_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: COLOR_GROUND,
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::default(),
    ));

    let mat_red = materials.add(StandardMaterial {
        base_color: COLOR_RED,
        perceptual_roughness: 0.9,
        ..default()
    });
    let mat_orange = materials.add(StandardMaterial {
        base_color: COLOR_ORANGE,
        perceptual_roughness: 0.9,
        ..default()
    });
    let mat_wall = materials.add(StandardMaterial {
        base_color: COLOR_WALL,
        perceptual_roughness: 1.0,
        ..default()
    });

    // --- Препятствия: ящики и шестигранные колонны ---
    for def in OBSTACLES {
        let mesh = if def.column {
            // resolution(6) — шестигранник, классический low-poly.
            meshes.add(
                Cylinder::new(def.size.x / 2.0, def.size.y)
                    .mesh()
                    .resolution(6),
            )
        } else {
            meshes.add(Cuboid::from_size(def.size))
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(if def.orange {
                mat_orange.clone()
            } else {
                mat_red.clone()
            }),
            Transform::from_xyz(def.pos.x, def.size.y / 2.0, def.pos.y),
            // Колонну для коллизий аппроксимируем квадратом — для прототипа достаточно.
            Obstacle {
                half_extents: def.size / 2.0,
            },
        ));
    }

    // --- Стены по периметру (тоже препятствия: блокируют игрока и снаряды) ---
    let long = ARENA_SIZE + WALL_THICKNESS * 2.0;
    let walls = [
        (
            Vec3::new(0.0, 0.0, -ARENA_HALF - WALL_THICKNESS / 2.0),
            Vec3::new(long, WALL_HEIGHT, WALL_THICKNESS),
        ),
        (
            Vec3::new(0.0, 0.0, ARENA_HALF + WALL_THICKNESS / 2.0),
            Vec3::new(long, WALL_HEIGHT, WALL_THICKNESS),
        ),
        (
            Vec3::new(-ARENA_HALF - WALL_THICKNESS / 2.0, 0.0, 0.0),
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, long),
        ),
        (
            Vec3::new(ARENA_HALF + WALL_THICKNESS / 2.0, 0.0, 0.0),
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, long),
        ),
    ];
    for (pos, size) in walls {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(mat_wall.clone()),
            Transform::from_translation(pos + Vec3::Y * size.y / 2.0),
            Obstacle {
                half_extents: size / 2.0,
            },
        ));
    }

    // --- Свет: жёсткое направленное "солнце" с тенями ---
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(20.0, 40.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn setup_player_and_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<GameAssets>,
) {
    let start = Vec3::new(0.0, PLAYER_SIZE.y / 2.0, 0.0);

    // --- Игрок: синий вытянутый куб + "нос"-указатель направления ---
    commands
        .spawn((
            Mesh3d(assets.player_mesh.clone()),
            MeshMaterial3d(assets.player_mat.clone()),
            Transform::from_translation(start),
            Player::default(),
            Velocity::default(),
        ))
        .with_children(|parent| {
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

    // --- Камера: HDR + Bloom, чтобы снаряды и рывок "светились" ---
    // looking_at из точки (player + offset) даёт наклон ≈ -56° по X.
    // Поворот камеры дальше не меняется — двигаем только позицию.
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_translation(start + CAMERA_OFFSET).looking_at(start, Vec3::Y),
        MainCamera,
    ));
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("oxide-core\nWASD: move | SPACE: dash | LMB: shoot"),
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

// ============================================================================
// ДВИЖЕНИЕ И РЫВОК
// ============================================================================

/// Двигает `current` к `target` не более чем на `max_delta`.
/// Это даёт линейное (а не экспоненциальное) ускорение/торможение —
/// предсказуемое и "резкое", без вязкого хвоста как у lerp.
fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let diff = target - current;
    let dist = diff.length();
    if dist <= max_delta || dist < f32::EPSILON {
        target
    } else {
        current + diff / dist * max_delta
    }
}

fn player_movement(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    assets: Res<GameAssets>,
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

    // Ввод WASD в мировых осях: камера смотрит вдоль -Z, поэтому "вверх по экрану" = -Z.
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

    // --- Старт рывка ---
    if keys.just_pressed(KeyCode::Space)
        && player.dash_cooldown_left <= 0.0
        && player.dash_time_left <= 0.0
    {
        // Рывок в сторону движения; если стоим — туда, куда смотрим.
        let dir = if wish == Vec3::ZERO {
            let f = transform.forward().as_vec3();
            Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
        } else {
            wish
        };
        player.dash_dir = dir;
        player.dash_time_left = DASH_DURATION;
        player.dash_cooldown_left = DASH_COOLDOWN;
        material.0 = assets.player_dash_mat.clone();

        // Вспышка искр в момент рывка.
        let seed = (time.elapsed_secs() * 1000.0) as u32;
        spawn_sparks(
            &mut commands,
            &assets,
            assets.dash_spark_mat.clone(),
            transform.translation,
            14,
            7.0,
            seed,
        );
    }

    if player.dash_time_left > 0.0 {
        // --- Во время рывка: фиксированная высокая скорость ---
        player.dash_time_left -= dt;
        velocity.0 = player.dash_dir * DASH_SPEED;

        // След: полупрозрачный "призрак" игрока каждый кадр.
        commands.spawn((
            Mesh3d(assets.player_mesh.clone()),
            MeshMaterial3d(assets.ghost_mat.clone()),
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

        if player.dash_time_left <= 0.0 {
            // Конец рывка: возвращаем цвет и гасим скорость до "выходной".
            material.0 = assets.player_mat.clone();
            velocity.0 = player.dash_dir * DASH_EXIT_SPEED;
        }
    } else {
        // --- Обычное движение: быстрый разгон и ещё более быстрое торможение ---
        let target = wish * MOVE_SPEED;
        let rate = if wish == Vec3::ZERO {
            FRICTION
        } else {
            ACCELERATION
        };
        velocity.0 = move_towards(velocity.0, target, rate * dt);
    }
}

fn apply_velocity(time: Res<Time>, mut query: Query<(&mut Transform, &Velocity)>) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut query {
        transform.translation += velocity.0 * dt;
    }
}

/// Коллизия "круг (игрок) против AABB (препятствие)" в плоскости XZ.
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

        // Ближайшая к центру игрока точка прямоугольника.
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
            // Центр игрока внутри коробки (например, после рывка) — выталкиваем по кратчайшей оси.
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

        // Убираем компоненту скорости "в стену" — игрок скользит вдоль неё.
        let mut v = Vec2::new(velocity.0.x, velocity.0.z);
        let into = v.dot(normal);
        if into < 0.0 {
            v -= normal * into;
            velocity.0.x = v.x;
            velocity.0.z = v.y;
        }
    }

    // Страховка: не даём выйти за пределы арены.
    let limit = ARENA_HALF - PLAYER_RADIUS;
    transform.translation.x = transform.translation.x.clamp(-limit, limit);
    transform.translation.z = transform.translation.z.clamp(-limit, limit);
}

// ============================================================================
// КАМЕРА
// ============================================================================

fn camera_follow(
    time: Res<Time>,
    player_q: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let (Ok(player), Ok(mut camera)) = (player_q.get_single(), camera_q.get_single_mut()) else {
        return;
    };
    let target = player.translation + CAMERA_OFFSET;
    // Frame-rate-independent LERP: t = 1 - e^(-k*dt).
    // Обычный lerp(…, k*dt) вёл бы себя по-разному на 30 и 240 FPS.
    let t = 1.0 - (-CAMERA_SMOOTHNESS * time.delta_secs()).exp();
    camera.translation = camera.translation.lerp(target, t);
}

// ============================================================================
// ПРИЦЕЛИВАНИЕ (RAYCAST КУРСОРА НА ПЛОСКОСТЬ)
// ============================================================================

fn update_aim(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &Transform), With<MainCamera>>,
    mut aim: ResMut<AimPoint>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    // 1. Позиция курсора в пикселях окна (None, если курсор вне окна —
    //    тогда сохраняем прошлую точку прицела).
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_tf)) = camera_q.get_single() else {
        return;
    };

    // 2. GlobalTransform камеры обновляется только в PostUpdate, т.е. он отстаёт
    //    на кадр от camera_follow. У камеры нет родителя, поэтому её локальный
    //    Transform == глобальный — строим GlobalTransform из него, чтобы
    //    прицел не "дрожал" при движении камеры.
    let camera_global = GlobalTransform::from(*camera_tf);

    // 3. viewport_to_world превращает 2D-точку экрана в 3D-луч:
    //    origin — точка на ближней плоскости отсечения под курсором,
    //    direction — направление "вглубь" сцены через этот пиксель.
    //    (В Bevy 0.15 возвращает Result; в 0.14 — Option.)
    let Ok(ray) = camera.viewport_to_world(&camera_global, cursor) else {
        return;
    };

    // 4. Пересекаем луч с бесконечной горизонтальной плоскостью y = AIM_PLANE_Y
    //    (нормаль — вверх). Математика: t = dot(plane_origin - ray.origin, n) / dot(ray.dir, n).
    //    intersect_plane возвращает расстояние t вдоль луча или None,
    //    если луч параллелен плоскости или смотрит от неё.
    let plane_origin = Vec3::new(0.0, AIM_PLANE_Y, 0.0);
    let Some(distance) = ray.intersect_plane(plane_origin, InfinitePlane3d::new(Vec3::Y)) else {
        return;
    };

    // 5. Точка в мире: origin + direction * t.
    aim.0 = Some(ray.get_point(distance));
}

fn rotate_player_to_aim(aim: Res<AimPoint>, mut player_q: Query<&mut Transform, With<Player>>) {
    let (Some(point), Ok(mut transform)) = (aim.0, player_q.get_single_mut()) else {
        return;
    };
    // Смотрим на точку прицела, но на высоте самого игрока — вращение только по оси Y.
    let target = Vec3::new(point.x, transform.translation.y, point.z);
    if target.distance_squared(transform.translation) > 0.01 {
        transform.look_at(target, Vec3::Y);
    }
}

// ============================================================================
// СТРЕЛЬБА
// ============================================================================

fn player_shoot(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    assets: Res<GameAssets>,
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

    // Игрок уже повёрнут к прицелу, поэтому направление = его forward.
    let dir = transform.forward().as_vec3();
    let muzzle = transform.transform_point(NOSE_TIP_LOCAL);

    commands.spawn((
        Mesh3d(assets.projectile_mesh.clone()),
        MeshMaterial3d(assets.projectile_mat.clone()),
        Transform::from_translation(muzzle),
        Projectile {
            velocity: dir * PROJECTILE_SPEED,
            lifetime_left: PROJECTILE_LIFETIME,
        },
        NotShadowCaster,
    ));

    // Маленькая дульная вспышка.
    let seed = (time.elapsed_secs() * 7919.0) as u32;
    spawn_sparks(
        &mut commands,
        &assets,
        assets.spark_mat.clone(),
        muzzle,
        3,
        3.0,
        seed,
    );
}

fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    obstacles: Query<(&Transform, &Obstacle), Without<Projectile>>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut projectile) in &mut projectiles {
        transform.translation += projectile.velocity * dt;
        projectile.lifetime_left -= dt;

        // Время жизни истекло — тихо удаляем.
        if projectile.lifetime_left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Попадание в препятствие: точка внутри AABB, расширенного на радиус снаряда.
        let p = transform.translation;
        let hit = obstacles.iter().any(|(obs_tf, obs)| {
            let d = (p - obs_tf.translation).abs();
            let h = obs.half_extents + Vec3::splat(PROJECTILE_RADIUS);
            d.x <= h.x && d.y <= h.y && d.z <= h.z
        });
        if hit {
            commands.entity(entity).despawn();
            let seed = entity.index().wrapping_mul(2_654_435_761);
            spawn_sparks(
                &mut commands,
                &assets,
                assets.spark_mat.clone(),
                p,
                8,
                6.0,
                seed,
            );
        }
    }
}

// ============================================================================
// ЧАСТИЦЫ
// ============================================================================

/// Дешёвый детерминированный хеш → [0, 1]. Позволяет обойтись без крейта `rand`.
fn hash01(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}

fn spawn_sparks(
    commands: &mut Commands,
    assets: &GameAssets,
    material: Handle<StandardMaterial>,
    origin: Vec3,
    count: u32,
    speed: f32,
    rng_seed: u32,
) {
    for i in 0..count {
        let s = rng_seed.wrapping_add(i.wrapping_mul(97));
        // Равномерно по кругу + немного случайности по углу, высоте и скорости.
        let angle = (i as f32 / count as f32) * TAU + hash01(s) * 0.8;
        let up = 0.5 + hash01(s ^ 0xA5A5) * 1.5;
        let spd = speed * (0.5 + hash01(s ^ 0x5A5A));
        let velocity = Vec3::new(angle.cos() * spd, up * spd * 0.4, angle.sin() * spd);
        let scale = 0.6 + hash01(s ^ 0x1234) * 0.8;

        commands.spawn((
            Mesh3d(assets.spark_mesh.clone()),
            MeshMaterial3d(material.clone()),
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
        // Не проваливаемся под пол.
        transform.translation.y = transform.translation.y.max(0.05);
        // Плавно уменьшаемся до нуля к концу жизни.
        let k = particle.lifetime_left / particle.max_lifetime;
        transform.scale = Vec3::splat(particle.start_scale * k);
    }
}

// ============================================================================
// ВИЗУАЛИЗАЦИЯ ПРИЦЕЛА (GIZMOS)
// ============================================================================

fn draw_aim_gizmos(mut gizmos: Gizmos, aim: Res<AimPoint>, player_q: Query<(&Transform, &Player)>) {
    let Ok((transform, player)) = player_q.get_single() else {
        return;
    };
    // Gizmo-круги по умолчанию лежат в плоскости XY — кладём их на землю (XZ).
    let flat = Quat::from_rotation_x(FRAC_PI_2);

    // Индикатор готовности рывка под ногами игрока.
    let feet = Vec3::new(transform.translation.x, 0.03, transform.translation.z);
    let dash_color = if player.dash_cooldown_left <= 0.0 {
        Color::LinearRgba(LinearRgba::rgb(0.3, 1.5, 3.0))
    } else {
        Color::srgba(0.4, 0.4, 0.45, 0.6)
    };
    gizmos.circle(Isometry3d::new(feet, flat), 0.75, dash_color);

    let Some(point) = aim.0 else {
        return;
    };

    // Лазерный указатель: от кончика носа до точки прицела.
    // Значения > 1.0 в HDR дают свечение через Bloom.
    let muzzle = transform.transform_point(NOSE_TIP_LOCAL);
    let laser = Color::LinearRgba(LinearRgba::rgb(4.0, 0.25, 0.15));
    gizmos.line(muzzle, point, laser);

    // Перекрестие в точке прицела.
    gizmos.circle(Isometry3d::new(point, flat), 0.35, laser);
    gizmos.line(point - Vec3::X * 0.5, point + Vec3::X * 0.5, laser);
    gizmos.line(point - Vec3::Z * 0.5, point + Vec3::Z * 0.5, laser);

    // Проекция прицела на землю — помогает читать глубину сцены.
    let ground = Vec3::new(point.x, 0.03, point.z);
    gizmos.circle(
        Isometry3d::new(ground, flat),
        0.2,
        Color::srgba(1.0, 0.3, 0.2, 0.5),
    );
}

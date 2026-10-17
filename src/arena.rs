//! 🏛️ arena.rs — Генерация арены, препятствий, стен и освещения.
//!
//! Экспортирует:
//! - `ArenaPlugin`: Bevy-плагин инициализации уровня.
//! - `Obstacle`: компонент препятствия с полуразмерами AABB-коллайдера.
//! - `OBSTACLES`: декларативная таблица расстановки ящиков и колонн на арене.

use bevy::prelude::*;

use crate::config::{
    ARENA_HALF, ARENA_SIZE, COLOR_GROUND, COLOR_ORANGE, COLOR_RED, COLOR_SKY, COLOR_WALL,
    WALL_HEIGHT, WALL_THICKNESS,
};

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(COLOR_SKY))
            .insert_resource(AmbientLight {
                color: Color::srgb(0.6, 0.65, 0.8),
                brightness: 250.0,
            })
            .add_systems(Startup, setup_arena);
    }
}

/// Статичное препятствие с прямоугольной AABB-коллизией в плоскости XZ.
#[derive(Component)]
pub struct Obstacle {
    pub half_extents: Vec3,
}

/// Описание отдельного препятствия в таблице уровня.
pub struct ObstacleDef {
    /// Позиция центра в горизонтальной плоскости (X, Z).
    pub pos: Vec2,
    /// Габариты препятствия: (ширина X, высота Y, глубина Z).
    pub size: Vec3,
    pub orange: bool,
    /// true — цилиндрическая шестигранная колонна, false — прямоугольный ящик.
    pub column: bool,
}

/// Декларативная карта препятствий арены.
/// Чтобы добавить, удалить или подвинуть объект — просто отредактируйте этот массив.
pub const OBSTACLES: &[ObstacleDef] = &[
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

fn setup_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Плоскость пола 100x100
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

    // Расстановка препятствий
    for def in OBSTACLES {
        let mesh = if def.column {
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
            Obstacle {
                half_extents: def.size / 2.0,
            },
        ));
    }

    // Стены по периметру арены
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

    // Направленный источник света с тенями
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(20.0, 40.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

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
    clippy::cast_sign_loss,
    // Сложные типы в Bevy Query<(...), (...)> идиоматичны для ECS
    clippy::type_complexity
)]

pub mod aim;
pub mod arena;
pub mod camera;
pub mod combat;
pub mod config;
pub mod enemy;
pub mod physics;
pub mod player;
pub mod ui;
pub mod vfx;

use bevy::prelude::*;

/// Состояния игры: активный геймплей либо экран окончания игры (Game Over).
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    InGame,
    GameOver,
}

/// Единый порядок выполнения систем в каждом игровом кадре (`Update`).
/// Любая новая система подключается в подходящий набор: `.in_set(GameSet::Combat)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// 1. Ввод пользователя, WASD, инициация рывка
    Movement,
    /// 2. Физическая интеграция скорости и реакция на столкновения
    Physics,
    /// 3. Сглаженное следование камеры за игроком
    Camera,
    /// 4. Рейкаст курсора мыши и поворот игрока в сторону прицела
    Aim,
    /// 5. Логика стрельбы и симуляция снарядов
    Combat,
    /// 6. Процедурные эффекты (частицы, Gizmo-линии лазера и прицела)
    Vfx,
}

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
        .init_state::<AppState>()
        // Задаём строгий порядок фаз каждого кадра во время активного боя
        .configure_sets(
            Update,
            (
                GameSet::Movement,
                GameSet::Physics,
                GameSet::Camera,
                GameSet::Aim,
                GameSet::Combat,
                GameSet::Vfx,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        // Подключаем модули игры
        .add_plugins((
            arena::ArenaPlugin,
            vfx::VfxPlugin,
            physics::PhysicsPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            aim::AimPlugin,
            combat::CombatPlugin,
            enemy::EnemyPlugin,
            ui::UiPlugin,
        ))
        .run();
}

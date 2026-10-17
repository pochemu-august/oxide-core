//! 🖥️ ui.rs — Наэкранный интерфейс (HUD), экран Game Over и логика перезапуска на клавишу R (включая русскую раскладку К).
//!
//! Экспортирует:
//! - `UiPlugin`: Bevy-плагин UI и перезапуска.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::AppState;
use crate::camera::MainCamera;
use crate::combat::Projectile;
use crate::config::{CAMERA_OFFSET, PLAYER_MAX_HEALTH, PLAYER_START};
use crate::enemy::{Enemy, EnemyAssets, Health, WaveStats, spawn_squad};
use crate::physics::Velocity;
use crate::player::{Player, PlayerAssets};
use crate::vfx::Particle;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(Update, update_hud)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over_screen)
            .add_systems(OnExit(AppState::GameOver), despawn_game_over_screen)
            .add_systems(
                Update,
                handle_game_restart.run_if(in_state(AppState::GameOver)),
            );
    }
}

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct GameOverOverlay;

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(
            "oxide-core\nWASD: move | SPACE: dash | LMB: shoot\nHP: 100/100 | Eliminated: 0 | Hostiles: 6 | FPS: --",
        ),
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
        HudText,
    ));
}

fn update_hud(
    app_state: Res<State<AppState>>,
    stats: Res<WaveStats>,
    diagnostics: Res<DiagnosticsStore>,
    player_q: Query<&Health, With<Player>>,
    enemies_q: Query<&Enemy>,
    mut text_q: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = text_q.get_single_mut() else {
        return;
    };

    // При Game Over гарантированно отображаем 0 HP
    let hp = if *app_state.get() == AppState::GameOver {
        0.0
    } else {
        player_q.get_single().map_or(0.0, |h| h.current.max(0.0))
    };
    let hostiles = enemies_q.iter().count();
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(bevy::diagnostic::Diagnostic::smoothed)
        .unwrap_or(0.0);

    // Используем только ASCII символы, чтобы не отображались квадраты (▯)
    **text = format!(
        "oxide-core\nWASD: move | SPACE: dash | LMB: shoot\nHP: {:.0}/{:.0} | Eliminated: {} | Hostiles: {} | FPS: {:.0}",
        hp, PLAYER_MAX_HEALTH, stats.kills, hostiles, fps
    );
}

/// Спавнит полноэкранный ретро-оверлей при гибели игрока (Game Over).
fn spawn_game_over_screen(mut commands: Commands, stats: Res<WaveStats>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.01, 0.02, 0.88)),
            GameOverOverlay,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("SIGNAL LOST"),
                TextFont {
                    font_size: 58.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.15, 0.15)),
            ));

            // Замена длинного тире на стандартный ASCII дефис '-'
            parent.spawn((
                Text::new("SYSTEM CRITICAL - CORE TERMINATED"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.4, 0.4)),
            ));

            parent.spawn((
                Text::new(format!(
                    "Hostiles Eliminated: {}\nSimulation Time Terminated",
                    stats.kills
                )),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.85, 0.85)),
            ));

            // Только ASCII-символы во встроенном шрифте Bevy (без кириллицы, чтобы не было квадрата ▯)
            parent.spawn((
                Text::new("Press [ R ] or [ Space / LMB ] to Reboot Core"),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.3, 0.85, 1.0)),
            ));
        });
}

fn despawn_game_over_screen(mut commands: Commands, query: Query<Entity, With<GameOverOverlay>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

/// Перезапуск игры по нажатию клавиши R / К (поддерживает русскую и английскую раскладки, а также Space/LMB).
#[allow(clippy::too_many_arguments)]
fn handle_game_restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut stats: ResMut<WaveStats>,
    enemy_assets: Res<EnemyAssets>,
    player_assets: Res<PlayerAssets>,
    mut player_q: Query<(
        &mut Transform,
        &mut Velocity,
        &mut Player,
        &mut Health,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut camera_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
    enemies_q: Query<Entity, With<Enemy>>,
    projectiles_q: Query<Entity, With<Projectile>>,
    particles_q: Query<Entity, With<Particle>>,
) {
    // Проверка физической клавиши R, пробела, Enter или клика ЛКМ
    let mut restart_requested = keys.just_pressed(KeyCode::KeyR)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left);

    // Поддержка русской раскладки ввода (клавиша 'К' на месте 'R')
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if ev.key_code == KeyCode::KeyR {
            restart_requested = true;
            break;
        }
        if let Key::Character(ref s) = ev.logical_key {
            let s_str = s.as_str();
            if s_str.eq_ignore_ascii_case("r") || s_str == "к" || s_str == "К" {
                restart_requested = true;
                break;
            }
        }
    }

    if !restart_requested {
        return;
    }

    // 1. Очистка старых врагов, снарядов и частиц
    for e in &enemies_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &projectiles_q {
        commands.entity(e).despawn();
    }
    for e in &particles_q {
        commands.entity(e).despawn();
    }

    // 2. Сброс позиции, скорости, таймеров и здоровья игрока
    if let Ok((mut tf, mut vel, mut player, mut health, mut mat)) = player_q.get_single_mut() {
        tf.translation = PLAYER_START;
        tf.rotation = Quat::IDENTITY;
        vel.0 = Vec3::ZERO;
        health.current = PLAYER_MAX_HEALTH;
        player.dash_time_left = 0.0;
        player.dash_cooldown_left = 0.0;
        player.fire_cooldown_left = 0.0;
        player.invuln_timer = 0.0;
        mat.0 = player_assets.mat_normal.clone();
    }

    // 3. Сброс позиции камеры
    if let Ok(mut cam_tf) = camera_q.get_single_mut() {
        cam_tf.translation = PLAYER_START + CAMERA_OFFSET;
    }

    // 4. Сброс статистики убийств и спавна
    stats.kills = 0;
    stats.spawned_count = 0;

    // 5. Спавн нового начального отряда врагов
    spawn_squad(&mut commands, &enemy_assets, &mut stats);

    // 6. Возврат в игровое состояние
    next_state.set(AppState::InGame);
}

//! 📊 profiling.rs — Расширенная диагностика дисплея и неблокирующее логирование производительности.
//!
//! Экспортирует:
//! - `ProfilingPlugin`: Bevy-плагин сбора метрик монитора, времени кадров и отчётов.

#![allow(
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::format_push_string,
    clippy::similar_names,
    clippy::missing_panics_doc
)]

use std::fmt::Write as _;
use std::fs::{OpenOptions, create_dir_all};
use std::io::{BufWriter, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use bevy::app::AppExit;
use bevy::diagnostic::EntityCountDiagnosticsPlugin;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorMoved, Monitor, PrimaryWindow};

use crate::combat::Projectile;
use crate::enemy::Enemy;

pub struct ProfilingPlugin;

impl Plugin for ProfilingPlugin {
    fn build(&self, app: &mut App) {
        let profiler = PerfProfiler::new();

        app.add_plugins(EntityCountDiagnosticsPlugin)
            .insert_resource(profiler)
            .add_systems(Startup, log_display_specs)
            .add_systems(
                Update,
                (check_late_monitors, profile_frame, handle_app_exit),
            );
    }
}

/// Сообщение фоновому потоку записи логов.
enum LogMessage {
    Write(String),
}

/// Информация об экстремальном спайке кадра (> 10 мс).
#[derive(Clone, Debug)]
pub struct FrameSpike {
    pub duration_ms: f64,
    pub session_time_secs: f32,
    pub entities: usize,
    pub enemies: usize,
    pub projectiles: usize,
}

/// Общее состояние профилировщика, разделяемое с логикой записи.
struct ProfilerState {
    monitors_logged: bool,
    total_frames: u64,
    spikes_over_10ms: u64,
    all_frame_durations_ms: Vec<f64>,
    all_mouse_moving_durations: Vec<f64>,
    all_mouse_still_durations: Vec<f64>,
    all_mouse_events: u64,
    interval_frame_durations_ms: Vec<f64>,
    interval_mouse_moving_durations: Vec<f64>,
    interval_mouse_still_durations: Vec<f64>,
    interval_mouse_events: u64,
    top_10_spikes: Vec<FrameSpike>,
    interval_timer: f32,
}

/// Основной ресурс профилирования.
#[derive(Resource)]
pub struct PerfProfiler {
    state: Arc<Mutex<ProfilerState>>,
    log_sender: Option<Sender<LogMessage>>,
    logger_handle: Option<thread::JoinHandle<()>>,
    final_summary_written: Arc<AtomicBool>,
}

impl PerfProfiler {
    #[must_use]
    pub fn new() -> Self {
        let _ = create_dir_all("logs");
        let (tx, rx) = channel::<LogMessage>();

        // Фоновый рабочий поток с буферизованной записью на диск
        let logger_handle = thread::Builder::new()
            .name("perf_logger".into())
            .spawn(move || {
                let file = match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open("logs/perf_profile.log")
                {
                    Ok(f) => f,
                    Err(e) => {
                        error!("Не удалось открыть logs/perf_profile.log: {e}");
                        return;
                    }
                };
                let mut writer = BufWriter::new(file);
                while let Ok(msg) = rx.recv() {
                    match msg {
                        LogMessage::Write(text) => {
                            let _ = writer.write_all(text.as_bytes());
                            let _ = writer.flush();
                        }
                    }
                }
                let _ = writer.flush();
            })
            .expect("Не удалось запустить поток perf_logger");

        let state = Arc::new(Mutex::new(ProfilerState {
            monitors_logged: false,
            total_frames: 0,
            spikes_over_10ms: 0,
            all_frame_durations_ms: Vec::with_capacity(36_000),
            all_mouse_moving_durations: Vec::with_capacity(18_000),
            all_mouse_still_durations: Vec::with_capacity(18_000),
            all_mouse_events: 0,
            interval_frame_durations_ms: Vec::with_capacity(300),
            interval_mouse_moving_durations: Vec::with_capacity(300),
            interval_mouse_still_durations: Vec::with_capacity(300),
            interval_mouse_events: 0,
            top_10_spikes: Vec::with_capacity(10),
            interval_timer: 0.0,
        }));

        Self {
            state,
            log_sender: Some(tx),
            logger_handle: Some(logger_handle),
            final_summary_written: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Отправляет форматированную строку в фоновый поток для неблокирующей записи.
    pub fn log_text(&self, text: String) {
        if let Some(ref sender) = self.log_sender {
            let _ = sender.send(LogMessage::Write(text));
        }
    }

    /// Формирует итоговый аналитический отчёт при завершении сессии.
    pub fn flush_final_summary(&self) {
        if self.final_summary_written.swap(true, Ordering::SeqCst) {
            return;
        }

        let Ok(state) = self.state.lock() else {
            return;
        };

        let total = state.total_frames;
        if total == 0 {
            return;
        }

        let total_time_ms: f64 = state.all_frame_durations_ms.iter().sum();
        let avg_duration_ms = total_time_ms / (total as f64);
        let avg_fps = if avg_duration_ms > 0.0 {
            1000.0 / avg_duration_ms
        } else {
            0.0
        };

        let (fps_1pct_low, fps_01pct_low) = calculate_lows(&state.all_frame_durations_ms);
        let spike_pct = (state.spikes_over_10ms as f64) / (total as f64) * 100.0;
        let session_secs = total_time_ms / 1000.0;
        let spikes_over_10ms = state.spikes_over_10ms;

        let mut report = String::new();
        report.push_str(
            "\n===============================================================================\n",
        );
        report.push_str(
            "                       ИТОГОВЫЙ ОТЧЁТ ПРОИЗВОДИТЕЛЬНОСТИ                      \n",
        );
        report.push_str(
            "===============================================================================\n",
        );
        let _ = writeln!(report, "  Всего отрендерено кадров        : {total}");
        let _ = writeln!(
            report,
            "  Общее время сессии              : {session_secs:.2} сек"
        );
        let _ = writeln!(report, "  Средний FPS (Average FPS)       : {avg_fps:.1}");
        let _ = writeln!(
            report,
            "  Показатель 1% Low FPS           : {fps_1pct_low:.1}"
        );
        let _ = writeln!(
            report,
            "  Показатель 0.1% Low FPS         : {fps_01pct_low:.1}"
        );
        let _ = writeln!(
            report,
            "  Кадры > 10.0 мс (< 100 FPS)     : {spikes_over_10ms} ({spike_pct:.2}% от всех кадров)"
        );

        // Анализ влияния движения мыши на всю сессию
        let moving_frames = state.all_mouse_moving_durations.len();
        let still_frames = state.all_mouse_still_durations.len();

        report.push_str(
            "-------------------------------------------------------------------------------\n",
        );
        report.push_str("  АНАЛИЗ ВЛИЯНИЯ ДВИЖЕНИЯ МЫШИ (Mouse Motion vs Idle):\n");
        report.push_str("  Режим            Кадры     Ср. FPS   1% Low   Спайки >10мс\n");

        if moving_frames > 0 {
            let moving_sum: f64 = state.all_mouse_moving_durations.iter().sum();
            let moving_avg_fps = 1000.0 / (moving_sum / moving_frames as f64);
            let (moving_1pct, _) = calculate_lows(&state.all_mouse_moving_durations);
            let moving_spikes = state
                .all_mouse_moving_durations
                .iter()
                .filter(|&&d| d > 10.0)
                .count();
            let moving_spikes_pct = (moving_spikes as f64) / (moving_frames as f64) * 100.0;
            let _ = writeln!(
                report,
                "  Мышь двигалась  : {moving_frames:5}     {moving_avg_fps:5.1}     {moving_1pct:5.1}    {moving_spikes:5} ({moving_spikes_pct:.1}%)"
            );
        } else {
            report.push_str("  Мышь двигалась  :     0       N/A       N/A        0 (N/A)\n");
        }

        if still_frames > 0 {
            let still_sum: f64 = state.all_mouse_still_durations.iter().sum();
            let still_avg_fps = 1000.0 / (still_sum / still_frames as f64);
            let (still_1pct, _) = calculate_lows(&state.all_mouse_still_durations);
            let still_spikes = state
                .all_mouse_still_durations
                .iter()
                .filter(|&&d| d > 10.0)
                .count();
            let still_spikes_pct = (still_spikes as f64) / (still_frames as f64) * 100.0;
            let _ = writeln!(
                report,
                "  Мышь неподвижна : {still_frames:5}     {still_avg_fps:5.1}     {still_1pct:5.1}    {still_spikes:5} ({still_spikes_pct:.1}%)"
            );
        } else {
            report.push_str("  Мышь неподвижна :     0       N/A       N/A        0 (N/A)\n");
        }

        let total_events = state.all_mouse_events;
        let _ = writeln!(report, "  Всего событий мыши за сессию    : {total_events}");

        report.push_str(
            "-------------------------------------------------------------------------------\n",
        );
        report.push_str("  ТОП-10 САМЫХ ДЛИННЫХ КАДРОВ (СПАЙКОВ):\n");
        report.push_str("  #   Время кадра    FPS     Секунда    Сущности   Враги   Снаряды\n");

        for (i, spike) in state.top_10_spikes.iter().enumerate() {
            let inst_fps = if spike.duration_ms > 0.0 {
                1000.0 / spike.duration_ms
            } else {
                0.0
            };
            let idx = i + 1;
            let dur = spike.duration_ms;
            let time_sec = spike.session_time_secs;
            let ent = spike.entities;
            let en = spike.enemies;
            let pr = spike.projectiles;
            let _ = writeln!(
                report,
                "  {idx:02}  {dur:6.2} мс   {inst_fps:6.1}   {time_sec:6.2} с   {ent:8}   {en:5}   {pr:7}"
            );
        }

        report.push_str(
            "===============================================================================\n",
        );

        info!("{report}");
        self.log_text(report);
    }
}

impl Default for PerfProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PerfProfiler {
    fn drop(&mut self) {
        self.flush_final_summary();
        drop(self.log_sender.take());
        if let Some(handle) = self.logger_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Рассчитывает 1% Low и 0.1% Low по выборке длительностей кадров.
fn calculate_lows(durations_ms: &[f64]) -> (f64, f64) {
    if durations_ms.is_empty() {
        return (0.0, 0.0);
    }
    let mut sorted = durations_ms.to_vec();
    // Сортировка от самых долгих кадров к самым быстрым
    sorted.sort_by(|a, b| b.total_cmp(a));

    // 1% Low: среднее время кадра в верхнем 1% самых долгих
    let count_1pct = ((sorted.len() as f64) * 0.01).ceil() as usize;
    let count_1pct = count_1pct.clamp(1, sorted.len());
    let avg_1pct = sorted[..count_1pct].iter().sum::<f64>() / (count_1pct as f64);
    let fps_1pct = if avg_1pct > 0.0 {
        1000.0 / avg_1pct
    } else {
        0.0
    };

    // 0.1% Low: среднее время кадра в верхнем 0.1% самых долгих
    let count_01pct = ((sorted.len() as f64) * 0.001).ceil() as usize;
    let count_01pct = count_01pct.clamp(1, sorted.len());
    let avg_01pct = sorted[..count_01pct].iter().sum::<f64>() / (count_01pct as f64);
    let fps_01pct = if avg_01pct > 0.0 {
        1000.0 / avg_01pct
    } else {
        0.0
    };

    (fps_1pct, fps_01pct)
}

/// Логирует параметры окна и подключенных дисплеев на этапе запуска.
fn log_display_specs(
    windows: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    profiler: Res<PerfProfiler>,
) {
    try_log_monitors(&windows, &monitors, &profiler);
}

/// Проверяет появление мониторов, если winit инициализировал их после `Startup`.
fn check_late_monitors(
    windows: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    profiler: Res<PerfProfiler>,
) {
    let already_logged = profiler.state.lock().is_ok_and(|s| s.monitors_logged);
    if already_logged {
        return;
    }
    try_log_monitors(&windows, &monitors, &profiler);
}

fn try_log_monitors(
    windows: &Query<&Window, With<PrimaryWindow>>,
    monitors: &Query<&Monitor>,
    profiler: &PerfProfiler,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    let mut monitor_lines = Vec::new();
    for (i, m) in monitors.iter().enumerate() {
        let name = m.name.as_deref().unwrap_or("Неизвестный дисплей");
        let size = m.physical_size();
        let hz_str = m.refresh_rate_millihertz.map_or("N/A".into(), |mhz| {
            format!("{:.2} Гц", f64::from(mhz) / 1000.0)
        });

        monitor_lines.push(format!(
            "    - Дисплей #{i} [{name}]: Физич. размер: {}x{}, Scale: {:.2}, Частота: {hz_str}",
            size.x, size.y, m.scale_factor
        ));
    }

    if monitor_lines.is_empty() {
        return;
    }

    if let Ok(mut state) = profiler.state.lock() {
        if state.monitors_logged {
            return;
        }
        state.monitors_logged = true;
    }

    let mut header = String::new();
    header.push_str(
        "===============================================================================\n",
    );
    header.push_str(
        "                       ДИАГНОСТИКА ДИСПЛЕЯ И РЕНДЕРА                           \n",
    );
    header.push_str(
        "===============================================================================\n",
    );
    let _ = writeln!(
        header,
        "  Окно oxide-core: Физич. разрешение: {}x{}, Логич. размер: {:.0}x{:.0}",
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        window.width(),
        window.height()
    );
    let _ = writeln!(
        header,
        "  Режим синхронизации (PresentMode): {:?}",
        window.present_mode
    );
    header.push_str("  Обнаруженные мониторы:\n");
    for line in &monitor_lines {
        header.push_str(line);
        header.push('\n');
    }
    header.push_str(
        "===============================================================================\n\n",
    );

    info!("{header}");
    profiler.log_text(header);
}

/// Фиксирует метрики каждого кадра и периодически пишет срез в файл.
fn profile_frame(
    time: Res<Time<Real>>,
    profiler: Res<PerfProfiler>,
    mut cursor_moved: EventReader<CursorMoved>,
    mut mouse_motion: EventReader<MouseMotion>,
    entities_q: Query<Entity>,
    enemies_q: Query<&Enemy>,
    projectiles_q: Query<&Projectile>,
) {
    let dt_ms = time.delta_secs_f64() * 1000.0;
    if dt_ms <= 0.001 {
        return;
    }

    let mouse_events = (cursor_moved.read().count() + mouse_motion.read().count()) as u64;

    let Ok(mut state) = profiler.state.lock() else {
        return;
    };

    state.total_frames += 1;
    state.all_frame_durations_ms.push(dt_ms);
    state.interval_frame_durations_ms.push(dt_ms);

    state.all_mouse_events += mouse_events;
    state.interval_mouse_events += mouse_events;

    if mouse_events > 0 {
        state.all_mouse_moving_durations.push(dt_ms);
        state.interval_mouse_moving_durations.push(dt_ms);
    } else {
        state.all_mouse_still_durations.push(dt_ms);
        state.interval_mouse_still_durations.push(dt_ms);
    }

    let is_spike = dt_ms > 10.0;
    let is_interval = state.interval_timer + time.delta_secs() >= 2.0;

    // Считаем сущности только когда они действительно нужны (при спайке или для 2-сек среза),
    // чтобы не тратить процессорное время на полный перебор таблиц ECS в каждом обычном кадре.
    // Для Enemy и Projectile используем O(1) ExactSizeIterator::len().
    let (entities, enemies, projectiles) = if is_spike || is_interval {
        (
            entities_q.iter().count(),
            enemies_q.iter().len(),
            projectiles_q.iter().len(),
        )
    } else {
        (0, 0, 0)
    };

    if is_spike {
        state.spikes_over_10ms += 1;

        let spike = FrameSpike {
            duration_ms: dt_ms,
            session_time_secs: time.elapsed_secs(),
            entities,
            enemies,
            projectiles,
        };

        // Сохраняем в Топ-10 спайков
        if state.top_10_spikes.len() < 10 {
            state.top_10_spikes.push(spike);
            state
                .top_10_spikes
                .sort_by(|a, b| b.duration_ms.total_cmp(&a.duration_ms));
        } else if state
            .top_10_spikes
            .last()
            .is_some_and(|last| dt_ms > last.duration_ms)
        {
            state.top_10_spikes.pop();
            state.top_10_spikes.push(spike);
            state
                .top_10_spikes
                .sort_by(|a, b| b.duration_ms.total_cmp(&a.duration_ms));
        }
    }

    // Периодический сброс каждые 2 секунды
    state.interval_timer += time.delta_secs();
    if state.interval_timer >= 2.0 {
        state.interval_timer = 0.0;

        let interval_count = state.interval_frame_durations_ms.len();
        if interval_count > 0 {
            let interval_sum: f64 = state.interval_frame_durations_ms.iter().sum();
            let avg_duration = interval_sum / (interval_count as f64);
            let avg_fps = if avg_duration > 0.0 {
                1000.0 / avg_duration
            } else {
                0.0
            };
            let (fps_1pct, _) = calculate_lows(&state.interval_frame_durations_ms);
            let interval_spikes = state
                .interval_frame_durations_ms
                .iter()
                .filter(|&&d| d > 10.0)
                .count();

            // Расчёт FPS для режимов «мышь двигалась» / «мышь стояла» за интервал
            let moving_count = state.interval_mouse_moving_durations.len();
            let moving_fps = if moving_count > 0 {
                let moving_sum: f64 = state.interval_mouse_moving_durations.iter().sum();
                1000.0 / (moving_sum / moving_count as f64)
            } else {
                0.0
            };

            let still_count = state.interval_mouse_still_durations.len();
            let still_fps = if still_count > 0 {
                let still_sum: f64 = state.interval_mouse_still_durations.iter().sum();
                1000.0 / (still_sum / still_count as f64)
            } else {
                0.0
            };

            let interval_events = state.interval_mouse_events;

            state.interval_frame_durations_ms.clear();
            state.interval_mouse_moving_durations.clear();
            state.interval_mouse_still_durations.clear();
            state.interval_mouse_events = 0;

            let elapsed_sec = time.elapsed_secs();

            let mouse_str = if interval_events == 0 {
                format!("Мышь: 0 соб. (покой: {still_fps:5.1} FPS)")
            } else if still_count == 0 {
                format!("Мышь: {interval_events:4} соб. (движ: {moving_fps:5.1} FPS)")
            } else {
                format!(
                    "Мышь: {interval_events:4} соб. (движ: {moving_fps:5.1} | покой: {still_fps:5.1} FPS)"
                )
            };

            let log_line = format!(
                "[{elapsed_sec:6.1}с] FPS: {avg_fps:5.1} | 1% Low: {fps_1pct:5.1} | Спайки (>10мс): {interval_spikes:2}/{interval_count} кадров | {mouse_str} | Сущности: {entities} (Враги: {enemies}, Пули: {projectiles})\n"
            );

            profiler.log_text(log_line);
        }
    }
}

/// Перехватывает сигнал выхода из приложения (или нажатие Escape) для генерации итогового отчета.
fn handle_app_exit(
    mut app_exit: EventReader<AppExit>,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit_writer: EventWriter<AppExit>,
    profiler: Res<PerfProfiler>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit_writer.send(AppExit::Success);
    }
    if app_exit.read().next().is_some() {
        profiler.flush_final_summary();
    }
}

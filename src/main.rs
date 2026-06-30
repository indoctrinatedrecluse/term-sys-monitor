mod engine;

use engine::{LuaEngine, WidgetInfo, DiskStats, ProcessStats, SpanInfo};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::io;
use std::path::PathBuf;

use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{block::Title, Block, Borders, Paragraph},
    Terminal,
};
use sysinfo::System;

fn parse_color(color_str: &str) -> Color {
    match color_str.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" => Color::Gray,
        "white" => Color::White,
        "light_red" => Color::LightRed,
        "light_green" => Color::LightGreen,
        "light_yellow" => Color::LightYellow,
        "light_blue" => Color::LightBlue,
        "light_magenta" => Color::LightMagenta,
        "light_cyan" => Color::LightCyan,
        _ => Color::White,
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    );
}

// Determines the configuration path based on standard platform guidelines
fn get_config_path() -> PathBuf {
    // 1. Check local directory first (development mode)
    let local_path = PathBuf::from("config.lua");
    if local_path.exists() {
        return local_path;
    }

    // 2. Resolve to user configuration folder
    let mut config_dir = if cfg!(target_os = "windows") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
        } else {
            PathBuf::from(".")
        }
    } else {
        // Linux/macOS
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut p = PathBuf::from(home);
        p.push(".config");
        p
    };

    config_dir.push("sysmon");
    // Ensure the folder exists
    let _ = std::fs::create_dir_all(&config_dir);
    config_dir.push("config.lua");
    config_dir
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal raw mode and alternate screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Handle panics to restore terminal state
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    // Setup shared stats variables
    let cpu_usage = Arc::new(AtomicU32::new(0));
    let total_mem = Arc::new(AtomicU64::new(0));
    let used_mem = Arc::new(AtomicU64::new(0));
    let mem_percent = Arc::new(AtomicU32::new(0));
    let cpu_frequency = Arc::new(AtomicU64::new(0));
    let uptime = Arc::new(AtomicU64::new(0));
    let disks = Arc::new(std::sync::Mutex::new(Vec::<DiskStats>::new()));
    let top_cpu_procs = Arc::new(std::sync::Mutex::new(Vec::<ProcessStats>::new()));
    let top_mem_procs = Arc::new(std::sync::Mutex::new(Vec::<ProcessStats>::new()));

    // Query static host details at startup using a temp system instance
    let mut temp_sys = System::new_all();
    temp_sys.refresh_cpu();
    let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let cpu_brand = if let Some(cpu) = temp_sys.cpus().first() {
        cpu.brand().to_string()
    } else {
        "Unknown".to_string()
    };
    
    // Spawn background thread to poll system stats using sysinfo
    let cpu_usage_clone = cpu_usage.clone();
    let total_mem_clone = total_mem.clone();
    let used_mem_clone = used_mem.clone();
    let mem_percent_clone = mem_percent.clone();
    let cpu_frequency_clone = cpu_frequency.clone();
    let uptime_clone = uptime.clone();
    let disks_clone = disks.clone();
    let top_cpu_procs_clone = top_cpu_procs.clone();
    let top_mem_procs_clone = top_mem_procs.clone();

    std::thread::spawn(move || {
        let mut sys = System::new_all();
        let mut disk_loader = sysinfo::Disks::new_with_refreshed_list();
        
        // Populate disks list immediately at start
        {
            if let Ok(mut list) = disks_clone.lock() {
                for d in disk_loader.list() {
                    list.push(DiskStats {
                        name: d.name().to_string_lossy().into_owned(),
                        mount_point: d.mount_point().to_string_lossy().into_owned(),
                        total_space: d.total_space(),
                        available_space: d.available_space(),
                        is_removable: d.is_removable(),
                    });
                }
            }
        }

        // Warm up sysinfo to calculate first CPU usage interval correctly
        sys.refresh_cpu();
        sys.refresh_memory();
        sys.refresh_processes();
        std::thread::sleep(Duration::from_millis(100));
        sys.refresh_cpu();
        sys.refresh_memory();
        sys.refresh_processes();

        let mut loop_counter: u64 = 0;

        loop {
            sys.refresh_cpu();
            sys.refresh_memory();

            // Store CPU usage (* 10)
            let global_cpu = sys.global_cpu_info().cpu_usage();
            cpu_usage_clone.store((global_cpu * 10.0) as u32, Ordering::Relaxed);

            // Store RAM usage
            let total = sys.total_memory();
            let used = sys.used_memory();
            let pct = if total > 0 {
                ((used as f64 / total as f64) * 1000.0) as u32
            } else {
                0
            };

            total_mem_clone.store(total, Ordering::Relaxed);
            used_mem_clone.store(used, Ordering::Relaxed);
            mem_percent_clone.store(pct, Ordering::Relaxed);

            // Store CPU Frequency (first core or global CPU)
            let freq = if let Some(cpu) = sys.cpus().first() {
                cpu.frequency()
            } else {
                0
            };
            cpu_frequency_clone.store(freq, Ordering::Relaxed);

            // Store System Uptime
            let upt = System::uptime();
            uptime_clone.store(upt, Ordering::Relaxed);

            // Refresh processes every 1 second (every 2 loops)
            if loop_counter % 2 == 0 {
                sys.refresh_processes();
                
                let mut all_procs = Vec::new();
                for (pid, process) in sys.processes() {
                    all_procs.push(ProcessStats {
                        pid: pid.as_u32() as usize,
                        name: process.name().to_string(),
                        cpu_usage: process.cpu_usage(),
                        memory: process.memory(),
                    });
                }

                // Get top CPU processes
                let mut cpu_procs = all_procs.clone();
                cpu_procs.sort_by(|a, b| {
                    b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal)
                });
                cpu_procs.truncate(10);
                if let Ok(mut lock) = top_cpu_procs_clone.lock() {
                    *lock = cpu_procs;
                }

                // Get top Memory processes
                let mut mem_procs = all_procs;
                mem_procs.sort_by(|a, b| b.memory.cmp(&a.memory));
                mem_procs.truncate(10);
                if let Ok(mut lock) = top_mem_procs_clone.lock() {
                    *lock = mem_procs;
                }
            }

            // Refresh Disk list every 5 seconds (500ms * 10)
            if loop_counter % 10 == 0 {
                disk_loader.refresh_list();
                let mut current_disks = Vec::new();
                for d in disk_loader.list() {
                    current_disks.push(DiskStats {
                        name: d.name().to_string_lossy().into_owned(),
                        mount_point: d.mount_point().to_string_lossy().into_owned(),
                        total_space: d.total_space(),
                        available_space: d.available_space(),
                        is_removable: d.is_removable(),
                    });
                }
                if let Ok(mut list) = disks_clone.lock() {
                    *list = current_disks;
                }
            }

            loop_counter = loop_counter.wrapping_add(1);
            std::thread::sleep(Duration::from_millis(500));
        }
    });

    // Initialize NVML for GPU metrics if available
    let nvml = nvml_wrapper::Nvml::init().ok();

    // Initialize Lua Engine and read config.lua
    let lua_engine = match LuaEngine::new(
        cpu_usage,
        total_mem,
        used_mem,
        mem_percent,
        cpu_frequency,
        uptime,
        disks,
        top_cpu_procs,
        top_mem_procs,
        nvml,
        hostname,
        os_name,
        kernel_version,
        cpu_brand,
    ) {
        Ok(engine) => engine,
        Err(e) => {
            restore_terminal();
            eprintln!("Failed to initialize Lua Engine: {}", e);
            std::process::exit(1);
        }
    };

    // Determine config path and write embedded default config if none is present
    let config_path = get_config_path();
    if !config_path.exists() {
        let default_config = include_str!("../config.lua");
        if let Err(e) = std::fs::write(&config_path, default_config) {
            eprintln!("Warning: Failed to write default config.lua: {}", e);
        }
    }

    if let Err(e) = lua_engine.load_config(&config_path) {
        restore_terminal();
        eprintln!("Failed to load config.lua ({:?}): {}", config_path, e);
        std::process::exit(1);
    }

    // Main TUI render loop
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    let mut reload_banner_time: Option<Instant> = None;

    loop {
        // Poll keyboard event with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                } else if key.code == KeyCode::Char('r') {
                    // Trigger configuration reloading
                    let active_config = get_config_path();
                    if let Ok(()) = lua_engine.reload_config(&active_config) {
                        reload_banner_time = Some(Instant::now());
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            let time_str = Local::now().format("%H:%M:%S").to_string();

            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(0),
                        Constraint::Length(3),
                    ])
                    .split(f.size());

                // Header with live clock
                let header = Paragraph::new("   Status: Active | Refresh: 250ms")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Title::from(" 🖥️  TERMINAL SYSTEM MONITOR  ").alignment(Alignment::Left))
                            .title(Title::from(format!(" 🕒 {} ", time_str)).alignment(Alignment::Right))
                            .border_style(Style::default().fg(Color::Cyan))
                    )
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(header, chunks[0]);

                // Body containing Lua widgets
                let widgets = match lua_engine.get_widgets() {
                    Ok(w) => w,
                    Err(e) => {
                        let mut spans = Vec::new();
                        spans.push(SpanInfo {
                            text: format!("Engine Error: {}", e),
                            color: "red".to_string(),
                        });
                        vec![WidgetInfo {
                            name: "Error".to_string(),
                            spans,
                        }]
                    }
                };

                let mut widget_spans = Vec::new();
                for w in widgets {
                    let mut line_spans = vec![
                        // Pad widget name to 18 characters and add a nice column separator
                        Span::styled(format!(" {: <18} │ ", w.name), Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
                    ];
                    
                    for span in w.spans {
                        line_spans.push(Span::styled(
                            span.text,
                            Style::default().fg(parse_color(&span.color)),
                        ));
                    }
                    widget_spans.push(Line::from(line_spans));
                }

                // Fill rest of the body with some instructions if empty
                if widget_spans.is_empty() {
                    widget_spans.push(Line::from(" No widgets registered in config.lua"));
                }

                let body = Paragraph::new(widget_spans)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Title::from(" Lua Configured Widgets ").alignment(Alignment::Left))
                            .border_style(Style::default().fg(Color::DarkGray))
                    )
                    .style(Style::default().fg(Color::White));
                f.render_widget(body, chunks[1]);

                // Footer with hot-reload status overlay
                let reload_status = if let Some(t) = reload_banner_time {
                    if t.elapsed() < Duration::from_secs(2) {
                        "  [ CONFIG RELOADED! ]"
                    } else {
                        ""
                    }
                } else {
                    ""
                };
                
                let footer_text = format!(" Press [q] to Quit | Press [r] to Reload{}", reload_status);
                let footer = Paragraph::new(footer_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray))
                    )
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(footer, chunks[2]);
            })?;

            last_tick = Instant::now();
        }
    }

    // Reset terminal raw mode and alternate screen
    restore_terminal();

    Ok(())
}

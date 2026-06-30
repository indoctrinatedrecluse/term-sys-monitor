mod engine;

use engine::{LuaEngine, WidgetInfo, DiskStats};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::io;

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
        std::thread::sleep(Duration::from_millis(100));
        sys.refresh_cpu();
        sys.refresh_memory();

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

    if let Err(e) = lua_engine.load_config("config.lua") {
        restore_terminal();
        eprintln!("Failed to load config.lua: {}", e);
        std::process::exit(1);
    }

    // Main TUI render loop
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        // Poll keyboard event with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
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
                    Err(e) => vec![WidgetInfo {
                        name: "Error".to_string(),
                        text: format!("Engine Error: {}", e),
                        color: "red".to_string(),
                    }],
                };

                let mut widget_spans = Vec::new();
                for w in widgets {
                    widget_spans.push(Line::from(vec![
                        // Pad widget name to 18 characters and add a nice column separator
                        Span::styled(format!(" {: <18} │ ", w.name), Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
                        Span::styled(w.text, Style::default().fg(parse_color(&w.color))),
                    ]));
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

                // Footer
                let footer = Paragraph::new(" Press [q] to Quit | Powered by Rust & Lua")
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

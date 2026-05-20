use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Chart, Dataset, Gauge, LineGauge, List, ListItem,
        Paragraph, Row, Table, Tabs,
    },
    Frame, Terminal,
};
use std::{io, time::Duration};

pub async fn run() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res?;
    Ok(())
}

#[derive(Clone)]
enum Tab {
    Dashboard,
    Skills,
    Health,
    Users,
    Settings,
}

struct App {
    tab: Tab,
    cases: Vec<CaseStatus>,
    skills: Vec<SkillStatus>,
    health: Vec<HealthItem>,
    users: Vec<UserItem>,
    settings: Vec<SettingItem>,
    workspace_root: String,
    scroll_offset: u16,
    should_quit: bool,
}

#[derive(Clone)]
struct CaseStatus {
    name: String,
    stage: &'static str,
    stage_progress: f64,
    details: Vec<String>,
}

#[derive(Clone)]
struct SkillStatus {
    name: String,
    version: u32,
    confidence: f64,
    trials: u32,
    score: f64,
    reward_history: Vec<f64>,
}

#[derive(Clone)]
struct HealthItem {
    category: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Clone)]
struct UserItem {
    name: String,
    email: String,
    role: String,
    active: bool,
}

#[derive(Clone)]
struct SettingItem {
    section: &'static str,
    key: &'static str,
    value: String,
}

fn initial_app() -> App {
    let workspace = std::env::var("AEROFLOW_WORKSPACE_DIR").unwrap_or_else(|_| "/data".to_string());
    App {
        tab: Tab::Dashboard,
        cases: vec![
            CaseStatus {
                name: "wing-v2".into(),
                stage: "SOLVING",
                stage_progress: 0.68,
                details: vec![
                    "itr: 1342/2000".into(),
                    "p: 1.2e-5".into(),
                    "U: 3.7e-6".into(),
                    "Cd: 0.032".into(),
                    "Cl: 0.521".into(),
                ],
            },
            CaseStatus {
                name: "fuse-v1".into(),
                stage: "MESHING",
                stage_progress: 0.42,
                details: vec![
                    "cells: 2.1M".into(),
                    "qual: ⚠".into(),
                    "nonOrth: 63°".into(),
                ],
            },
            CaseStatus {
                name: "duct-v3".into(),
                stage: "QUEUED",
                stage_progress: 0.0,
                details: vec!["waiting...".into()],
            },
            CaseStatus {
                name: "nozzle".into(),
                stage: "QUEUED",
                stage_progress: 0.0,
                details: vec!["waiting...".into()],
            },
        ],
        skills: vec![
            SkillStatus {
                name: "External_Aero_Subsonic".into(),
                version: 3,
                confidence: 0.82,
                trials: 12,
                score: 0.92,
                reward_history: vec![0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.85, 0.88, 0.90, 0.91, 0.91, 0.92],
            },
            SkillStatus {
                name: "Internal_Pipe_HighRe".into(),
                version: 1,
                confidence: 0.21,
                trials: 3,
                score: 0.45,
                reward_history: vec![0.3, 0.38, 0.45],
            },
        ],
        health: vec![
            HealthItem { category: "Docker", status: "✓", detail: "5/5 checks passed".into() },
            HealthItem { category: "Database", status: "✓", detail: "Connected, schema v003".into() },
            HealthItem { category: "OpenFOAM", status: "✗", detail: "Env not sourced (fix available)".into() },
            HealthItem { category: "Filesystem", status: "⚠", detail: "Disk 3.2 GB free".into() },
            HealthItem { category: "System", status: "✓", detail: "16 CPUs, 31 GB mem".into() },
            HealthItem { category: "Skills", status: "✓", detail: "14 skills, 37 trials".into() },
            HealthItem { category: "VTK/Post", status: "✓", detail: "vtkio ready".into() },
        ],
        users: vec![
            UserItem { name: "Jettanakorn Pengsiri".into(), email: "jetta@jfoxaircraft.com".into(), role: "admin".into(), active: true },
            UserItem { name: "Engineer Bot".into(), email: "bot@aeroflow.local".into(), role: "engineer".into(), active: true },
        ],
        settings: vec![
            SettingItem { section: "Workspace", key: "workspace_dir", value: workspace.clone() },
            SettingItem { section: "Workspace", key: "write_format", value: "binary".into() },
            SettingItem { section: "Resources", key: "max_concurrent", value: "4".into() },
            SettingItem { section: "Resources", key: "default_cores", value: "4".into() },
            SettingItem { section: "Resources", key: "default_memory_gb", value: "8".into() },
            SettingItem { section: "Solver", key: "max_iterations", value: "3000".into() },
            SettingItem { section: "Solver", key: "convergence_residual", value: "1e-6".into() },
        ],
        workspace_root: workspace,
        scroll_offset: 0,
        should_quit: false,
    }
}

async fn run_app(terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = initial_app();

    while !app.should_quit {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Tab => {
                        app.tab = match app.tab {
                            Tab::Dashboard => Tab::Skills,
                            Tab::Skills => Tab::Health,
                            Tab::Health => Tab::Users,
                            Tab::Users => Tab::Settings,
                            Tab::Settings => Tab::Dashboard,
                        };
                    }
                    KeyCode::BackTab => {
                        app.tab = match app.tab {
                            Tab::Dashboard => Tab::Settings,
                            Tab::Skills => Tab::Dashboard,
                            Tab::Health => Tab::Skills,
                            Tab::Users => Tab::Health,
                            Tab::Settings => Tab::Users,
                        };
                    }
                    KeyCode::Up => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        app.scroll_offset = app.scroll_offset.saturating_add(1);
                    }
                    _ => {}
                }
            }
        }

        for case in &mut app.cases {
            if case.stage == "SOLVING" {
                case.stage_progress = (case.stage_progress + 0.005).min(1.0);
            } else if case.stage == "MESHING" {
                case.stage_progress = (case.stage_progress + 0.003).min(1.0);
            }
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let header = Paragraph::new(vec![
        Line::from("AeroFlow Agent v0.1.0").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("Developer: Jettanakorn Pengsiri by JFOX Aircraft Co., Ltd.").style(
            Style::default().fg(Color::DarkGray),
        ),
    ])
    .block(Block::default().borders(Borders::BOTTOM));

    let header_area = Rect {
        height: 3,
        ..size
    };
    let rest = Rect {
        y: 3,
        height: size.height - 3,
        ..size
    };
    f.render_widget(header, header_area);

    let tab_titles = vec![" Dashboard ", " Skills ", " Health ", " Users ", " Settings "];
    let tabs = Tabs::new(tab_titles)
        .select(match app.tab {
            Tab::Dashboard => 0,
            Tab::Skills => 1,
            Tab::Health => 2,
            Tab::Users => 3,
            Tab::Settings => 4,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let tab_area = Rect {
        height: 3,
        ..rest
    };
    let content_area = Rect {
        y: rest.y + 3,
        height: rest.height - 3,
        ..rest
    };
    f.render_widget(tabs, tab_area);

    match app.tab {
        Tab::Dashboard => render_dashboard(f, content_area, app),
        Tab::Skills => render_skills(f, content_area, app),
        Tab::Health => render_health(f, content_area, app),
        Tab::Users => render_users(f, content_area, app),
        Tab::Settings => render_settings(f, content_area, app),
    }

    let footer = Paragraph::new(
        "Tab/Shift+Tab: Switch panel  |  ↑↓: Scroll  |  q/Esc: Quit",
    )
    .style(Style::default().fg(Color::DarkGray))
    .block(Block::default().borders(Borders::TOP));
    let footer_area = Rect {
        y: size.height - 1,
        height: 1,
        ..size
    };
    f.render_widget(footer, footer_area);
}

fn render_dashboard(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let case_block = Block::default()
        .title("Active Cases")
        .borders(Borders::ALL);
    let inner = case_block.inner(chunks[0]);
    f.render_widget(case_block, chunks[0]);

    let case_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, 4); 4])
        .margin(1)
        .split(inner);

    for (i, case) in app.cases.iter().enumerate() {
        if i >= case_layout.len() {
            break;
        }
        render_case_card(f, case_layout[i], case);
    }

    let resource_block = Block::default()
        .title("System Resources")
        .borders(Borders::ALL);
    let resource_inner = resource_block.inner(chunks[1]);
    f.render_widget(&resource_block, chunks[1]);

    let resource_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .margin(1)
        .split(resource_inner);

    render_resource_gauge(f, resource_layout[0], "CPU", 0.76, Color::Green);
    render_resource_gauge(f, resource_layout[1], "Memory", 0.58, Color::Yellow);
    render_resource_gauge(f, resource_layout[2], "Disk", 0.35, Color::Blue);
    render_resource_gauge(f, resource_layout[3], "Network", 0.22, Color::Magenta);

    let log_items: Vec<ListItem> = vec![
        ListItem::new("14:32:15 wing-v2 Convergence OK (residuals < 1e-6)"),
        ListItem::new("14:32:14 fuse-v1 Mesh quality WARNING: maxNonOrtho = 63°"),
        ListItem::new("14:32:12 fuse-v1 Auto-adjusting snappyHexMesh, re-meshing..."),
    ];
    let log_block = Block::default()
        .title("Event Log")
        .borders(Borders::ALL);
    let log_list = List::new(log_items).block(log_block);
    let log_area = resource_layout[4];
    f.render_widget(log_list, log_area);
}

fn render_case_card(f: &mut Frame, area: Rect, case: &CaseStatus) {
    let color = match case.stage {
        "SOLVING" => Color::Cyan,
        "MESHING" => Color::Yellow,
        "QUEUED" => Color::DarkGray,
        "COMPLETE" => Color::Green,
        "FAILED" => Color::Red,
        _ => Color::White,
    };

    let block = Block::default()
        .title(format!(" {} ", case.name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    let stage_label = Line::from(vec![
        Span::styled(case.stage, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ]);
    let stage_widget = Paragraph::new(stage_label);
    f.render_widget(stage_widget, chunks[0]);

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(case.stage_progress as f64);
    let gauge_area = Rect {
        y: chunks[0].y,
        height: 1,
        ..chunks[0]
    };
    f.render_widget(gauge, gauge_area);

    let details: Vec<Line> = case.details.iter().map(|d| Line::from(Span::raw(d))).collect();
    let detail_widget = Paragraph::new(details);
    f.render_widget(detail_widget, chunks[1]);
}

fn render_resource_gauge(f: &mut Frame, area: Rect, label: &str, value: f64, color: Color) {
    let gauge = LineGauge::default()
        .filled_style(Style::default().fg(color))
        .ratio(value)
        .label(label);
    f.render_widget(gauge, area);
}

fn render_skills(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let skill_items: Vec<ListItem> = app
        .skills
        .iter()
        .map(|s| {
            let content = format!(
                "{} v{}  ⭐ {:.2}  conf={:.0}%  n={}",
                s.name, s.version, s.score, s.confidence * 100.0, s.trials
            );
            ListItem::new(content)
        })
        .collect();

    let skill_block = Block::default()
        .title("Skills Database")
        .borders(Borders::ALL);
    let skill_list = List::new(skill_items).block(skill_block);
    f.render_widget(skill_list, chunks[0]);

    if let Some(skill) = app.skills.first() {
        let data: Vec<(f64, f64)> = skill
            .reward_history
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f64, *v))
            .collect();

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&data);

        let chart_block = Block::default()
            .title(format!("{} Reward History", skill.name))
            .borders(Borders::ALL);

        let chart = Chart::new(vec![dataset])
            .block(chart_block)
            .x_axis(
                ratatui::widgets::Axis::default()
                    .title("Trial")
                    .bounds([0.0, skill.reward_history.len() as f64 - 1.0]),
            )
            .y_axis(
                ratatui::widgets::Axis::default()
                    .title("Reward")
                    .bounds([0.0, 1.0]),
            );
        f.render_widget(chart, chunks[1]);
    }
}

fn render_health(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("System Health")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows: Vec<Row> = app
        .health
        .iter()
        .map(|h| {
            let color = match h.status {
                "✓" => Color::Green,
                "⚠" => Color::Yellow,
                "✗" => Color::Red,
                "ℹ" => Color::Cyan,
                _ => Color::White,
            };
            Row::new(vec![
                Cell::from(h.category),
                Cell::from(h.status).style(Style::default().fg(color)),
                Cell::from(h.detail.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(15),
            Constraint::Length(5),
            Constraint::Min(40),
        ],
    )
    .header(
        Row::new(vec!["Category", "Status", "Detail"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    );

    let table_area = Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(2),
    };
    f.render_widget(table, table_area);
}

fn render_users(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Users & Authentication")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows: Vec<Row> = app
        .users
        .iter()
        .map(|u| {
            let active = if u.active { "✓" } else { "✗" };
            Row::new(vec![
                Cell::from(u.name.clone()),
                Cell::from(u.email.clone()),
                Cell::from(u.role.clone()),
                Cell::from(active),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(25),
            Constraint::Length(30),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Name", "Email", "Role", "Active"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    );

    let table_area = Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(10),
    };
    f.render_widget(table, table_area);

    let info_area = Rect {
        x: inner.x + 1,
        y: inner.y + inner.height - 8,
        width: inner.width.saturating_sub(2),
        height: 7,
    };
    let info = Paragraph::new(vec![
        Line::from("User Management Commands:"),
        Line::from("  aeroflow user create     — Register a new user"),
        Line::from("  aeroflow user login      — Authenticate and get session token"),
        Line::from("  aeroflow user list       — List all registered users"),
        Line::from("  aeroflow user show       — Show user details"),
        Line::from("  aeroflow user update     — Modify user profile"),
        Line::from("  aeroflow user delete     — Remove a user"),
    ])
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(info, info_area);
}

fn render_settings(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let setting_rows: Vec<Row> = app
        .settings
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(s.section),
                Cell::from(s.key),
                Cell::from(s.value.clone()),
            ])
        })
        .collect();

    let setting_block = Block::default()
        .title("Current Settings")
        .borders(Borders::ALL);
    let setting_inner = setting_block.inner(chunks[0]);
    f.render_widget(setting_block, chunks[0]);

    let setting_table = Table::new(
        setting_rows,
        vec![
            Constraint::Length(14),
            Constraint::Length(22),
            Constraint::Min(10),
        ],
    )
    .header(
        Row::new(vec!["Section", "Key", "Value"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    );

    let table_area = Rect {
        x: setting_inner.x + 1,
        y: setting_inner.y + 1,
        width: setting_inner.width.saturating_sub(2),
        height: setting_inner.height.saturating_sub(2),
    };
    f.render_widget(setting_table, table_area);

    let info_block = Block::default()
        .title("Settings Management")
        .borders(Borders::ALL);
    let info_inner = info_block.inner(chunks[1]);
    f.render_widget(info_block, chunks[1]);

    let info = Paragraph::new(vec![
        Line::from(format!("Workspace: {}", app.workspace_root)),
        Line::from(""),
        Line::from("aeroflow settings show     — View all settings"),
        Line::from("aeroflow settings set      — Modify a setting"),
        Line::from("aeroflow settings init     — Initialize workspace"),
        Line::from("aeroflow settings reset    — Reset to defaults"),
        Line::from("aeroflow settings path     — Show config file path"),
        Line::from(""),
        Line::from("Env vars prefix: AEROFLOW_*"),
        Line::from("Config file: aeroflow-settings.toml"),
    ])
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(info, info_inner);
}

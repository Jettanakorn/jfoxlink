mod commands;
mod tui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "aeroflow")]
#[command(about = "AeroFlow Agent — Automated CFD Analysis by JFOX Aircraft Co., Ltd.")]
#[command(long_about = "AeroFlow Agent v0.1.0
Developer: Jettanakorn Pengsiri by JFOX Aircraft Co., Ltd.

Autonomous CFD analysis using OpenFOAM + ParaView.
Self-improving skills database with Gaussian Process optimization.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(global = true, short = 'v', long = "verbose")]
    verbose: bool,

    #[arg(global = true, long = "json-logs")]
    json_logs: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new case with guided Q&A
    Init {
        /// Case name
        name: Option<String>,
    },
    /// Execute full pipeline for a case
    Run {
        /// Case directory
        case: PathBuf,
        /// Number of optimization trials (autonomous mode)
        #[arg(long, default_value = "0")]
        trials: u32,
    },
    /// Show all active cases
    Status,
    /// Generate report for a case
    Report {
        /// Case ID or name
        case: String,
    },
    /// Watch a directory for new STL files
    Watch {
        /// Directory to watch
        #[arg(default_value = "/data/import")]
        path: PathBuf,
    },
    /// Start optional web dashboard
    Serve {
        #[arg(default_value = "8080")]
        port: u16,
    },
    /// System health check and fix
    Doctor {
        /// Category to check (docker, database, openfoam, filesystem, system, skills, postproc)
        category: Option<String>,
        /// Attempt auto-fix
        #[arg(long)]
        fix: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Continuous watch mode
        #[arg(long)]
        watch: bool,
    },
    /// Manage skills database
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
    /// User management
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Manage AeroFlow settings
    Settings {
        #[command(subcommand)]
        action: SettingsAction,
    },
    /// Launch interactive TUI dashboard
    Tui,
}

#[derive(Subcommand)]
enum SkillsAction {
    /// List all skills
    List,
    /// Show skill details
    Show { name: String },
    /// Trigger optimization for a skill
    Optimize { name: String, #[arg(long, default_value = "10")] trials: u32 },
    /// Export a skill
    Export { name: String, #[arg(long)] format: Option<String> },
    /// Import a skill
    Import { path: PathBuf },
    /// Reset/unlearn a skill
    Reset { name: String },
}

#[derive(Subcommand)]
enum UserAction {
    /// Create a new user
    Create,
    /// List all users
    List,
    /// Show user details
    Show { email: String },
    /// Update a user
    Update { email: String },
    /// Delete a user
    Delete { email: String },
    /// Authenticate a user
    Login { email: String },
}

#[derive(Subcommand)]
enum SettingsAction {
    /// Show current settings
    Show,
    /// Set a setting value (key=value)
    Set { key: String, value: String },
    /// Initialize workspace and settings
    Init { path: Option<String> },
    /// Reset settings to defaults
    Reset,
    /// Show settings file path
    Path,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let env_filter = if cli.verbose { "aeroflow=debug" } else { "aeroflow=info" };

    if cli.json_logs {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    match &cli.command {
        Commands::Init { name } => commands::init::execute(name.clone()).await?,
        Commands::Run { case, trials } => commands::run::execute(case, *trials).await?,
        Commands::Status => commands::status::execute().await?,
        Commands::Report { case } => commands::report::execute(case).await?,
        Commands::Watch { path } => commands::watch::execute(path).await?,
        Commands::Serve { port } => commands::serve::execute(*port).await?,
        Commands::Doctor { category, fix, json, watch } => {
            commands::doctor::execute(category.as_deref(), *fix, *json, *watch).await?
        }
        Commands::Skills { action } => commands::skills::execute(action).await?,
        Commands::User { action } => commands::user::execute(action).await?,
        Commands::Settings { action } => commands::settings::execute(action).await?,
        Commands::Tui => tui::dashboard::run().await?,
    }

    Ok(())
}

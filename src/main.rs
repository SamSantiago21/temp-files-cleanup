use std::path::PathBuf;
use temp_files_cleanup_rust::actions::ActionExecutor;
use temp_files_cleanup_rust::domain::Action;
use temp_files_cleanup_rust::{
    actions::SystemActionExecutor,
    history::JsonlHistory,
    persistence::{self, Config},
    runtime::{RuntimeHandle, RuntimeStatus},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    if std::env::args().skip(1).eq(["--internal-elevated-clean"]) {
        let result =
            SystemActionExecutor.execute(&Action::CleanTemporaryFiles { directories: None });
        let message = result?;
        println!("{message}");
        return Ok(());
    }
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--engine") {
        let config_path = args
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("automations.json"));
        let app = temp_files_cleanup_rust::app::DesktopApp::open(config_path)
            .map_err(|e| format!("Could not start desktop application: {e}"))?;
        let options = eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_title("Automation Desk")
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([900.0, 600.0]),
            ..Default::default()
        };
        eframe::run_native("Automation Desk", options, Box::new(|_| Ok(Box::new(app))))?;
        return Ok(());
    }
    let config_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("automations.json"));
    let config = if config_path.exists() {
        persistence::load(&config_path)?
    } else {
        let c = Config::default();
        persistence::save(&config_path, &c)?;
        c
    };
    let history_path = directories::ProjectDirs::from("com", "temp-files-cleanup", "engine")
        .ok_or("cannot resolve application data directory")?
        .data_dir()
        .join("execution.jsonl");
    let history = std::sync::Arc::new(JsonlHistory::open(history_path)?);
    let runtime = RuntimeHandle::start(config.automations, history);
    tracing::info!(path=%config_path.display(), "headless runtime started; waiting for events");
    loop {
        match runtime.status() {
            RuntimeStatus::Running | RuntimeStatus::Starting => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            RuntimeStatus::Stopped => return Ok(()),
            RuntimeStatus::Error(error) => return Err(error.into()),
        }
    }
}

use directories::ProjectDirs;
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};
use temp_files_cleanup_rust::{
    actions::SystemActionExecutor,
    conditions::SystemConditionEvaluator,
    engine::Engine,
    history::JsonlHistory,
    persistence::{self, Config},
    triggers,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("automations.json"));
    let config = if config_path.exists() {
        persistence::load(&config_path)?
    } else {
        let c = Config::default();
        persistence::save(&config_path, &c)?;
        c
    };
    let dirs = ProjectDirs::from("com", "temp-files-cleanup", "engine")
        .ok_or("cannot resolve application data directory")?;
    let history = Arc::new(JsonlHistory::open(dirs.data_dir().join("execution.jsonl"))?);
    let (tx, rx) = mpsc::channel();
    triggers::spawn_interval_triggers(config.automations.clone(), tx);
    let engine = Engine::new(
        config.automations,
        Arc::new(SystemActionExecutor),
        history,
        SystemConditionEvaluator,
    );
    tracing::info!(path=%config_path.display(), "engine started; waiting for events");
    engine.run(rx).map_err(Into::into)
}

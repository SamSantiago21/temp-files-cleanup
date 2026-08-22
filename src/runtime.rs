use crate::{
    actions::SystemActionExecutor,
    conditions::SystemConditionEvaluator,
    domain::{Automation, TriggerFired},
    engine::Engine,
    history::JsonlHistory,
    triggers,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

type EngineInstance = Engine<SystemActionExecutor, JsonlHistory, SystemConditionEvaluator>;

struct SourceHandles {
    stop: Arc<AtomicBool>,
    scheduler: JoinHandle<()>,
    #[cfg(windows)]
    hotkeys: JoinHandle<()>,
}

impl SourceHandles {
    fn shutdown(self) -> Result<(), String> {
        self.stop.store(true, Ordering::Relaxed);
        self.scheduler
            .join()
            .map_err(|_| "scheduler source thread panicked".to_string())?;
        #[cfg(windows)]
        self.hotkeys
            .join()
            .map_err(|_| "hotkey source thread panicked".to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

enum RuntimeCommand {
    Trigger(TriggerFired, Option<Sender<Result<(), String>>>),
    Refresh(Vec<Automation>, Sender<()>),
    Shutdown,
}

pub struct RuntimeHandle {
    commands: Sender<RuntimeCommand>,
    status: Arc<Mutex<RuntimeStatus>>,
    join: Option<JoinHandle<()>>,
}

impl RuntimeHandle {
    pub fn start(automations: Vec<Automation>, history: Arc<JsonlHistory>) -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (events, event_rx) = mpsc::channel();
        let (started, started_rx) = mpsc::sync_channel(1);
        let status = Arc::new(Mutex::new(RuntimeStatus::Starting));
        let thread_status = status.clone();
        let join = thread::spawn(move || {
            runtime_thread(
                automations,
                history,
                command_rx,
                event_rx,
                events,
                thread_status,
                started,
            )
        });
        let _ = started_rx.recv();
        Self {
            commands,
            status,
            join: Some(join),
        }
    }

    pub fn status(&self) -> RuntimeStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| RuntimeStatus::Error("Runtime status unavailable".into()))
    }

    pub fn trigger(&self, automation_id: impl Into<String>) -> Result<(), String> {
        self.commands
            .send(RuntimeCommand::Trigger(
                TriggerFired {
                    automation_id: automation_id.into(),
                    source: "manual".into(),
                },
                None,
            ))
            .map_err(|_| "The automation runtime is stopped".into())
    }

    pub fn refresh(&self, automations: Vec<Automation>) -> Result<(), String> {
        let (complete, done) = mpsc::channel();
        self.commands
            .send(RuntimeCommand::Refresh(automations, complete))
            .map_err(|_| "The automation runtime is stopped".to_string())?;
        done.recv()
            .map_err(|_| "The automation runtime stopped during refresh".into())
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(RuntimeCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn runtime_thread(
    mut automations: Vec<Automation>,
    history: Arc<JsonlHistory>,
    command_rx: Receiver<RuntimeCommand>,
    event_rx: Receiver<TriggerFired>,
    event_tx: Sender<TriggerFired>,
    status: Arc<Mutex<RuntimeStatus>>,
    started: mpsc::SyncSender<()>,
) {
    let mut engine = build_engine(&automations, history.clone());
    let mut sources = start_sources(&automations, event_tx.clone());
    set_status(&status, RuntimeStatus::Running);
    let _ = started.send(());

    loop {
        match command_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(RuntimeCommand::Trigger(event, complete)) => {
                let result = engine.dispatch(event).map_err(|error| error.to_string());
                if let Some(complete) = complete {
                    let _ = complete.send(result.clone());
                }
                if let Err(error) = result {
                    let _ = sources.shutdown();
                    set_status(&status, RuntimeStatus::Error(error.to_string()));
                    return;
                }
            }
            Ok(RuntimeCommand::Refresh(updated, complete)) => {
                if let Err(error) = sources.shutdown() {
                    set_status(&status, RuntimeStatus::Error(error));
                    let _ = complete.send(());
                    return;
                }
                automations = updated;
                engine = build_engine(&automations, history.clone());
                sources = start_sources(&automations, event_tx.clone());
                let _ = complete.send(());
            }
            Ok(RuntimeCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(error) = sources.shutdown() {
                    set_status(&status, RuntimeStatus::Error(error));
                    return;
                }
                set_status(&status, RuntimeStatus::Stopped);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for event in event_rx.try_iter() {
            if let Err(error) = engine.dispatch(event) {
                let _ = sources.shutdown();
                set_status(&status, RuntimeStatus::Error(error.to_string()));
                return;
            }
        }
    }
}

fn build_engine(automations: &[Automation], history: Arc<JsonlHistory>) -> EngineInstance {
    Engine::new(
        automations.to_vec(),
        Arc::new(SystemActionExecutor),
        history,
        SystemConditionEvaluator,
    )
}

fn start_sources(automations: &[Automation], events: Sender<TriggerFired>) -> SourceHandles {
    let stop = Arc::new(AtomicBool::new(false));
    let scheduler = triggers::spawn_scheduled_triggers_with_stop(
        automations.to_vec(),
        events.clone(),
        stop.clone(),
    );
    #[cfg(windows)]
    let hotkeys = crate::windows::hotkey::spawn_all_with_stop(automations, events, stop.clone());
    SourceHandles {
        stop,
        scheduler,
        #[cfg(windows)]
        hotkeys,
    }
}

fn set_status(status: &Arc<Mutex<RuntimeStatus>>, value: RuntimeStatus) {
    if let Ok(mut current) = status.lock() {
        *current = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{Action, ConditionNode, Trigger},
        history::{HistoryFilter, HistoryProvider},
    };

    fn test_automation(id: &str) -> Automation {
        Automation {
            id: id.into(),
            name: id.into(),
            enabled: true,
            trigger: Trigger::Manual,
            conditions: ConditionNode::Empty,
            actions: vec![Action::CleanTemporaryFiles {
                directories: Some(vec!["Z:\\runtime-stabilization-missing".into()]),
            }],
            settings: Default::default(),
        }
    }

    #[test]
    fn status_is_running_after_start() {
        let path = std::env::temp_dir().join("automation-desk-runtime-test.jsonl");
        let history = Arc::new(JsonlHistory::open(path).expect("history"));
        let runtime = RuntimeHandle::start(vec![], history);
        assert_eq!(runtime.status(), RuntimeStatus::Running);
    }

    #[test]
    fn refresh_replaces_sources_and_uses_new_automation() {
        let path = std::env::temp_dir().join("automation-desk-runtime-refresh-test.jsonl");
        let history = Arc::new(JsonlHistory::open(path).expect("history"));
        let runtime = RuntimeHandle::start(vec![test_automation("old")], history.clone());
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        runtime
            .refresh(vec![test_automation("new")])
            .expect("refresh");
        let (complete, done) = mpsc::channel();
        runtime
            .commands
            .send(RuntimeCommand::Trigger(
                TriggerFired {
                    automation_id: "new".into(),
                    source: "manual".into(),
                },
                Some(complete),
            ))
            .expect("trigger refreshed automation");
        done.recv().expect("dispatch response").expect("dispatch");

        let records = history
            .get_history(HistoryFilter::default())
            .expect("read history");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].result.automation_id, "new");
    }

    #[test]
    fn repeated_refreshes_do_not_prevent_clean_shutdown() {
        let path = std::env::temp_dir().join("automation-desk-runtime-repeated-refresh-test.jsonl");
        let history = Arc::new(JsonlHistory::open(path).expect("history"));
        let runtime = RuntimeHandle::start(vec![], history);
        assert_eq!(runtime.status(), RuntimeStatus::Running);
        for index in 0..4 {
            runtime
                .refresh(vec![test_automation(&format!("refresh-{index}"))])
                .expect("refresh");
        }
        drop(runtime);
    }
}

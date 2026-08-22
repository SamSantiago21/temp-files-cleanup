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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

enum RuntimeCommand {
    Trigger(TriggerFired),
    Refresh(Vec<Automation>),
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
            )
        });
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
            .send(RuntimeCommand::Trigger(TriggerFired {
                automation_id: automation_id.into(),
                source: "manual".into(),
            }))
            .map_err(|_| "The automation runtime is stopped".into())
    }

    pub fn refresh(&self, automations: Vec<Automation>) -> Result<(), String> {
        self.commands
            .send(RuntimeCommand::Refresh(automations))
            .map_err(|_| "The automation runtime is stopped".into())
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
) {
    let mut stop = Arc::new(AtomicBool::new(false));
    let mut engine = build_engine(&automations, history.clone());
    start_sources(&automations, event_tx.clone(), stop.clone());
    set_status(&status, RuntimeStatus::Running);

    loop {
        match command_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(RuntimeCommand::Trigger(event))
                if event.source == crate::domain::SHUTDOWN_SOURCE =>
            {
                stop.store(true, Ordering::Relaxed);
                set_status(&status, RuntimeStatus::Stopped);
                return;
            }
            Ok(RuntimeCommand::Trigger(event)) => {
                if let Err(error) = engine.dispatch(event) {
                    set_status(&status, RuntimeStatus::Error(error.to_string()));
                    return;
                }
            }
            Ok(RuntimeCommand::Refresh(updated)) => {
                stop.store(true, Ordering::Relaxed);
                automations = updated;
                engine = build_engine(&automations, history.clone());
                let next_stop = Arc::new(AtomicBool::new(false));
                start_sources(&automations, event_tx.clone(), next_stop.clone());
                // The old source threads observe their stop flag; the new flag is now authoritative.
                // Keeping this local assignment makes shutdown and subsequent refreshes deterministic.
                stop = next_stop;
            }
            Ok(RuntimeCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                stop.store(true, Ordering::Relaxed);
                set_status(&status, RuntimeStatus::Stopped);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for event in event_rx.try_iter() {
            if event.source == crate::domain::SHUTDOWN_SOURCE {
                stop.store(true, Ordering::Relaxed);
                set_status(&status, RuntimeStatus::Stopped);
                return;
            }
            if let Err(error) = engine.dispatch(event) {
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

fn start_sources(automations: &[Automation], events: Sender<TriggerFired>, stop: Arc<AtomicBool>) {
    triggers::spawn_scheduled_triggers_with_stop(
        automations.to_vec(),
        events.clone(),
        stop.clone(),
    );
    #[cfg(windows)]
    crate::windows::hotkey::spawn_all_with_stop(automations, events, stop);
}

fn set_status(status: &Arc<Mutex<RuntimeStatus>>, value: RuntimeStatus) {
    if let Ok(mut current) = status.lock() {
        *current = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_is_running_after_start() {
        let path = std::env::temp_dir().join("automation-desk-runtime-test.jsonl");
        let history = Arc::new(JsonlHistory::open(path).expect("history"));
        let runtime = RuntimeHandle::start(vec![], history);
        for _ in 0..20 {
            if runtime.status() == RuntimeStatus::Running {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("runtime did not start");
    }
}

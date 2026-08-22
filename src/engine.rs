use crate::{
    actions::ActionExecutor,
    conditions::ConditionEvaluator,
    domain::{Automation, Trigger, TriggerFired},
    errors::EngineError,
    history::{ExecutionResult, HistoryProvider},
};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
};
pub struct Engine<E, H, C> {
    automations: HashMap<String, Automation>,
    executor: Arc<E>,
    history: Arc<H>,
    conditions: C,
    running: HashMap<String, bool>,
}
impl<E: ActionExecutor + 'static, H: HistoryProvider + 'static, C: ConditionEvaluator>
    Engine<E, H, C>
{
    pub fn new(a: Vec<Automation>, executor: Arc<E>, history: Arc<H>, conditions: C) -> Self {
        Self {
            automations: a.into_iter().map(|x| (x.id.clone(), x)).collect(),
            executor,
            history,
            conditions,
            running: HashMap::new(),
        }
    }
    pub fn run(mut self, rx: mpsc::Receiver<TriggerFired>) -> Result<(), EngineError> {
        while let Ok(e) = rx.recv() {
            self.dispatch(e)?
        }
        Ok(())
    }
    pub fn dispatch(&mut self, e: TriggerFired) -> Result<(), EngineError> {
        let Some(a) = self.automations.get(&e.automation_id).cloned() else {
            return Ok(());
        };
        if !a.enabled
            || !trigger_matches(&a.trigger, &e.source)
            || !self.conditions.evaluate(&a.conditions)?
        {
            return Ok(());
        }
        if matches!(
            a.settings.concurrency_policy,
            crate::domain::ConcurrencyPolicy::SkipIfRunning
        ) && self.running.get(&a.id) == Some(&true)
        {
            return Ok(());
        }
        self.running.insert(a.id.clone(), true);
        for action in &a.actions {
            let (record, failure) = match self.executor.execute(action) {
                Ok(m) => (
                    ExecutionResult {
                        automation_id: a.id.clone(),
                        action: format!("{action:?}"),
                        success: true,
                        message: m,
                    },
                    None,
                ),
                Err(err) => {
                    let r = ExecutionResult {
                        automation_id: a.id.clone(),
                        action: format!("{action:?}"),
                        success: false,
                        message: err.to_string(),
                    };
                    (r, Some(err))
                }
            };
            if let Err(err) = self.history.record_execution(&record) {
                self.running.insert(a.id.clone(), false);
                return Err(err);
            }
            if !record.success
                && matches!(
                    a.settings.failure_policy,
                    crate::domain::FailurePolicy::Stop
                )
            {
                self.running.insert(a.id.clone(), false);
                return Err(failure.expect("failed action must carry its error"));
            }
        }
        self.running.insert(a.id, false);
        tracing::info!(automation=%a.name,source=%e.source,"automation completed");
        Ok(())
    }
}
pub fn trigger_matches(t: &Trigger, source: &str) -> bool {
    match t {
        Trigger::Manual => source == "manual",
        Trigger::Hotkey { combination } => source == combination,
        Trigger::Interval { .. } => source == "interval",
        Trigger::Daily { .. } => source == "daily",
    }
}

use crate::domain::{Automation, Trigger, TriggerFired};
use std::{sync::mpsc::Sender, thread, time::Duration};
pub fn spawn_interval_triggers(automations: Vec<Automation>, tx: Sender<TriggerFired>) {
    for a in automations {
        if let Trigger::Interval { seconds } = a.trigger {
            let tx = tx.clone();
            let _worker = thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(seconds.max(1)));
                    if tx
                        .send(TriggerFired {
                            automation_id: a.id.clone(),
                            source: "interval".into(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
    }
}
pub fn spawn_daily_triggers(_automations: Vec<Automation>, _tx: Sender<TriggerFired>) { /* isolated scheduling boundary for the next clock implementation */
}

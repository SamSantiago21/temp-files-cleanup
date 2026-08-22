use crate::domain::{Automation, Trigger, TriggerFired};
use std::{
    sync::mpsc::Sender,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub fn spawn_interval_triggers(automations: Vec<Automation>, tx: Sender<TriggerFired>) {
    for a in automations {
        if a.enabled {
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
}
pub fn spawn_daily_triggers(automations: Vec<Automation>, tx: Sender<TriggerFired>) {
    for a in automations {
        if !a.enabled {
            continue;
        }
        let Trigger::Daily { time_hh_mm } = a.trigger else {
            continue;
        };
        let Some((hour, minute)) = time_hh_mm
            .split_once(':')
            .and_then(|(h, m)| Some((h.parse::<u64>().ok()?, m.parse::<u64>().ok()?)))
        else {
            continue;
        };
        if hour > 23 || minute > 59 {
            continue;
        }
        let tx = tx.clone();
        thread::spawn(move || {
            loop {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let day = now / 86_400;
                let target = day * 86_400 + hour * 3_600 + minute * 60;
                let next = if target > now {
                    target
                } else {
                    target + 86_400
                };
                thread::sleep(Duration::from_secs((next - now).max(1)));
                if tx
                    .send(TriggerFired {
                        automation_id: a.id.clone(),
                        source: "daily".into(),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
}

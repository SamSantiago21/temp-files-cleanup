use crate::domain::{Automation, Trigger, TriggerFired};
use std::{
    sync::mpsc::Sender,
    thread,
    time::{Duration, Instant},
};

enum Schedule {
    Interval {
        seconds: u64,
        next: Instant,
    },
    Daily {
        hour: u64,
        minute: u64,
        next: Instant,
    },
}

pub fn spawn_scheduled_triggers(automations: Vec<Automation>, tx: Sender<TriggerFired>) {
    let mut schedules = Vec::new();
    for a in automations.into_iter().filter(|a| a.enabled) {
        match a.trigger {
            Trigger::Interval { seconds } => schedules.push((
                a.id,
                Schedule::Interval {
                    seconds: seconds.max(1),
                    next: Instant::now() + Duration::from_secs(seconds.max(1)),
                },
            )),
            Trigger::Daily { time_hh_mm } => match parse_daily(&time_hh_mm) {
                Ok((hour, minute)) => schedules.push((
                    a.id,
                    Schedule::Daily {
                        hour,
                        minute,
                        next: next_daily(hour, minute),
                    },
                )),
                Err(error) => {
                    tracing::warn!(automation=%a.id, %error, "invalid daily schedule; not registered")
                }
            },
            _ => {}
        }
    }
    thread::spawn(move || {
        loop {
            let Some((index, wait)) = schedules
                .iter()
                .enumerate()
                .map(|(i, (_, s))| (i, until(s)))
                .min_by_key(|(_, d)| *d)
            else {
                return;
            };
            thread::sleep(wait.max(Duration::from_millis(1)));
            let (id, schedule) = &mut schedules[index];
            let source = match schedule {
                Schedule::Interval { seconds, next } => {
                    *next += Duration::from_secs(*seconds);
                    "interval"
                }
                Schedule::Daily { hour, minute, next } => {
                    *next = next_daily(*hour, *minute);
                    "daily"
                }
            };
            if tx
                .send(TriggerFired {
                    automation_id: id.clone(),
                    source: source.into(),
                })
                .is_err()
            {
                return;
            }
        }
    });
}
fn until(s: &Schedule) -> Duration {
    match s {
        Schedule::Interval { next, .. } | Schedule::Daily { next, .. } => {
            next.saturating_duration_since(Instant::now())
        }
    }
}
fn parse_daily(value: &str) -> Result<(u64, u64), String> {
    let (h, m) = value
        .trim()
        .split_once(':')
        .ok_or_else(|| format!("invalid time {value:?}"))?;
    let hour = h
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("invalid time {value:?}"))?;
    let minute = m
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("invalid time {value:?}"))?;
    if hour > 23 || minute > 59 {
        return Err(format!("invalid time {value:?}"));
    }
    Ok((hour, minute))
}
fn next_daily(hour: u64, minute: u64) -> Instant {
    let now = local_clock();
    let current = now.0 * 60 + now.1;
    let target = hour * 60 + minute;
    let minutes = if target > current {
        target - current
    } else {
        1_440 - current + target
    };
    Instant::now() + Duration::from_secs(minutes * 60)
}
#[cfg(windows)]
fn local_clock() -> (u64, u64) {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let t = unsafe { GetLocalTime() };
    (t.wHour as u64, t.wMinute as u64)
}
#[cfg(not(windows))]
fn local_clock() -> (u64, u64) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mins = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 60
        % 1_440;
    (mins / 60, mins % 60)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_daily_times() {
        assert_eq!(parse_daily(" 18:00 ").unwrap(), (18, 0));
        assert!(parse_daily("24:00").is_err());
        assert!(parse_daily("noon").is_err());
    }
}

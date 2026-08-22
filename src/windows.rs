#[cfg(windows)]
pub mod hotkey {
    use crate::{
        domain::{Automation, Trigger, TriggerFired},
        errors::EngineError,
    };
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::Sender,
        },
        thread,
        time::Duration,
    };
    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{
                HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey,
                UnregisterHotKey, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY},
        },
    };

    pub fn spawn_all(
        automations: &[Automation],
        tx: Sender<TriggerFired>,
    ) -> thread::JoinHandle<()> {
        spawn_all_with_stop(automations, tx, Arc::new(AtomicBool::new(false)))
    }

    pub fn spawn_all_with_stop(
        automations: &[Automation],
        tx: Sender<TriggerFired>,
        stop: Arc<AtomicBool>,
    ) -> thread::JoinHandle<()> {
        let bindings: Vec<_> = automations
            .iter()
            .filter(|a| a.enabled)
            .filter_map(|a| match &a.trigger {
                Trigger::Hotkey { combination } => Some((combination.clone(), a.id.clone())),
                _ => None,
            })
            .collect();
        thread::spawn(move || {
            let mut registered = Vec::new();
            for (id, (combination, automation_id)) in bindings.into_iter().enumerate() {
                match parse(&combination) {
                    Ok((modifiers, key)) => match unsafe {
                        RegisterHotKey(
                            Some(HWND::default()),
                            id as i32 + 1,
                            modifiers,
                            key.0 as u32,
                        )
                    } {
                        Ok(()) => registered.push((id as i32 + 1, combination, automation_id)),
                        Err(error) => {
                            tracing::warn!(%error, combination, "global hotkey registration failed")
                        }
                    },
                    Err(error) => tracing::warn!(%error, combination, "invalid global hotkey"),
                }
            }
            while !stop.load(Ordering::Relaxed) {
                let mut message = MSG::default();
                let hotkey = if unsafe {
                    PeekMessageW(&mut message, Some(HWND::default()), 0, 0, PM_REMOVE)
                }
                .as_bool()
                    && message.message == WM_HOTKEY
                {
                    registered
                        .iter()
                        .find(|(id, _, _)| *id == message.wParam.0 as i32)
                } else {
                    None
                };
                if let Some((_, combination, automation_id)) = hotkey
                    && tx
                        .send(TriggerFired {
                            automation_id: automation_id.clone(),
                            source: combination.clone(),
                        })
                        .is_err()
                {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            for (id, _, _) in registered {
                let _ = unsafe { UnregisterHotKey(Some(HWND::default()), id) };
            }
        })
    }

    pub fn parse(combination: &str) -> Result<(HOT_KEY_MODIFIERS, VIRTUAL_KEY), EngineError> {
        let mut modifiers = HOT_KEY_MODIFIERS(0);
        let mut key = None;
        let mut seen = std::collections::HashSet::new();
        for token in combination
            .split('+')
            .map(str::trim)
            .filter(|x| !x.is_empty())
        {
            let upper = token.to_ascii_uppercase();
            let modifier = match upper.as_str() {
                "CTRL" | "CONTROL" => Some(MOD_CONTROL),
                "SHIFT" => Some(MOD_SHIFT),
                "ALT" => Some(MOD_ALT),
                "WIN" | "WINDOWS" => Some(MOD_WIN),
                _ => None,
            };
            if let Some(m) = modifier {
                if !seen.insert(upper) {
                    return Err(EngineError::Action(format!(
                        "duplicate hotkey modifier {token}"
                    )));
                }
                modifiers |= m;
                continue;
            }
            if key.is_some() {
                return Err(EngineError::Action(format!(
                    "multiple hotkey keys in {combination}"
                )));
            }
            key = Some(match upper.as_str() {
                "SPACE" => VIRTUAL_KEY(0x20),
                "ENTER" => VIRTUAL_KEY(0x0d),
                value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() => {
                    VIRTUAL_KEY(value.as_bytes()[0] as u16)
                }
                value
                    if value
                        .strip_prefix('F')
                        .and_then(|n| n.parse::<u16>().ok())
                        .is_some_and(|n| (1..=24).contains(&n)) =>
                {
                    VIRTUAL_KEY(0x70 + value[1..].parse::<u16>().unwrap() - 1)
                }
                _ => return Err(EngineError::Action(format!("unknown hotkey token {token}"))),
            });
        }
        key.map(|key| (modifiers, key))
            .ok_or_else(|| EngineError::Action(format!("hotkey has no key: {combination}")))
    }
}

#[cfg(windows)]
pub mod privilege {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::PathBuf};
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
        core::PCWSTR,
    };
    #[derive(Debug, Clone, Copy)]
    pub enum InternalPrivilegedTask {
        CleanTemporaryFiles,
    }
    pub fn request_elevation(task: InternalPrivilegedTask) -> std::io::Result<()> {
        let exe = std::env::current_exe()?;
        let arg = match task {
            InternalPrivilegedTask::CleanTemporaryFiles => "--internal-elevated-clean",
        };
        fn wide(value: &OsStr) -> Vec<u16> {
            value.encode_wide().chain(std::iter::once(0)).collect()
        }
        let file = wide(exe.as_os_str());
        let args = wide(OsStr::new(arg));
        let verb = wide(OsStr::new("runas"));
        let result = unsafe {
            ShellExecuteW(
                Some(HWND::default()),
                PCWSTR(verb.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(args.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as usize <= 32 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn allowed_cleanup_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Ok(t) = std::env::var("TEMP") {
            roots.push(t.into());
        }
        if let Ok(w) = std::env::var("WINDIR") {
            roots.push(PathBuf::from(&w).join("Temp"));
            roots.push(PathBuf::from(w).join("Prefetch"));
        }
        roots
    }
}

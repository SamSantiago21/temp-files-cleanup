#[cfg(windows)]
pub mod hotkey {
    use crate::domain::TriggerFired;
    use std::{
        sync::{
            atomic::{AtomicI32, Ordering},
            mpsc::Sender,
        },
        thread,
    };
    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY},
        },
    };

    static NEXT_ID: AtomicI32 = AtomicI32::new(1);

    pub fn spawn(combination: String, automation_id: String, tx: Sender<TriggerFired>) {
        thread::spawn(move || {
            let Some((modifiers, key)) = parse(&combination) else {
                tracing::error!(combination, "invalid hotkey; not registered");
                return;
            };
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            if let Err(error) = unsafe {
                RegisterHotKey(
                    Some(HWND(std::ptr::null_mut())),
                    id,
                    modifiers,
                    key.0 as u32,
                )
            } {
                tracing::error!(%error, combination, "could not register hotkey");
                return;
            }
            loop {
                let mut message = MSG::default();
                let result =
                    unsafe { GetMessageW(&mut message, Some(HWND(std::ptr::null_mut())), 0, 0) };
                if result.0 == 0 || result.0 == -1 {
                    break;
                }
                if message.message == WM_HOTKEY && message.wParam.0 as i32 == id {
                    if tx
                        .send(TriggerFired {
                            automation_id: automation_id.clone(),
                            source: combination.clone(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
            let _ = unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(
                    Some(HWND(std::ptr::null_mut())),
                    id,
                )
            };
        });
    }

    fn parse(
        combination: &str,
    ) -> Option<(
        windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS,
        VIRTUAL_KEY,
    )> {
        let mut modifiers = windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS(0);
        let mut key = None;
        for part in combination
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            match part.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
                "ALT" => modifiers |= MOD_ALT,
                "SHIFT" => modifiers |= MOD_SHIFT,
                "WIN" | "WINDOWS" => modifiers |= MOD_WIN,
                value => {
                    key = Some(match value {
                        "SPACE" => VIRTUAL_KEY(0x20),
                        "ENTER" => VIRTUAL_KEY(0x0D),
                        value
                            if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() =>
                        {
                            VIRTUAL_KEY(value.as_bytes()[0] as u16)
                        }
                        value
                            if value
                                .strip_prefix('F')
                                .and_then(|n| n.parse::<u16>().ok())
                                .is_some_and(|n| (1..=24).contains(&n)) =>
                        {
                            let n = value[1..].parse::<u16>().ok()?;
                            VIRTUAL_KEY(0x70 + n - 1)
                        }
                        _ => return None,
                    })
                }
            }
        }
        Some((modifiers, key?))
    }
}
#[cfg(windows)]
pub mod privilege {
    pub fn run_elevated(_executable: &str, _args: &[String]) -> std::io::Result<()> {
        use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
        use windows::Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };
        use windows::core::PCWSTR;
        fn wide(value: &OsStr) -> Vec<u16> {
            value.encode_wide().chain(std::iter::once(0)).collect()
        }
        let file = wide(OsStr::new(_executable));
        let args = wide(OsStr::new(&_args.join(" ")));
        let verb = wide(OsStr::new("runas"));
        let result = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
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
}

#[cfg(windows)]
pub mod hotkey {
    use crate::domain::TriggerFired;
    use std::sync::mpsc::Sender;
    pub fn spawn(_combination: String, _automation_id: String, _tx: Sender<TriggerFired>) { /* RegisterHotKey/GetMessageW boundary */
    }
}
#[cfg(windows)]
pub mod privilege {
    pub fn run_elevated(_executable: &str, _args: &[String]) -> std::io::Result<()> {
        use std::process::Command;
        Command::new("powershell")
            .args(["-Command", "Start-Process"])
            .spawn()
            .map(|_| ())
    }
}

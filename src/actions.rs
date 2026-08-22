use crate::{domain::Action, errors::EngineError};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub trait ActionExecutor: Send + Sync {
    fn execute(&self, a: &Action) -> Result<String, EngineError>;
}
#[derive(Default)]
pub struct SystemActionExecutor;
impl ActionExecutor for SystemActionExecutor {
    fn execute(&self, a: &Action) -> Result<String, EngineError> {
        match a {
            Action::CleanTemporaryFiles { directories } => clean(directories.as_ref()),
            Action::LaunchApplication { executable, args } => {
                std::process::Command::new(executable)
                    .args(args)
                    .spawn()
                    .map(|_| format!("launched {executable}"))
                    .map_err(|e| EngineError::Action(e.to_string()))
            }
            Action::ShowNotification { title, message } => {
                #[cfg(windows)]
                {
                    tauri_winrt_notification::Toast::new("TempFilesCleanup")
                        .title(title)
                        .text1(message)
                        .show()
                        .map(|_| format!("{title}: {message}"))
                        .map_err(|e| EngineError::Action(format!("notification failed: {e}")))
                }
                #[cfg(not(windows))]
                {
                    let _ = (title, message);
                    Err(EngineError::Action("notifications require Windows".into()))
                }
            }
        }
    }
}
fn clean(dirs: Option<&Vec<String>>) -> Result<String, EngineError> {
    let defaults = default_dirs();
    let paths: Vec<PathBuf> = dirs
        .map(|directories| directories.iter().map(PathBuf::from).collect())
        .unwrap_or(defaults);
    let (mut removed, mut skipped) = (0, 0);
    for p in paths {
        if !p.exists() {
            continue;
        }
        let entries = match fs::read_dir(&p) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(path=%p.display(), %e, "could not enumerate cleanup root");
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    skipped += 1;
                    tracing::warn!(%e, "could not inspect cleanup entry");
                    continue;
                }
            };
            let path = entry.path();
            let r = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            match r {
                Ok(_) => removed += 1,
                Err(e) => {
                    skipped += 1;
                    tracing::warn!(path=%path.display(), %e, "skipping cleanup entry");
                }
            }
        }
    }
    Ok(format!(
        "removed {removed} entries; skipped {skipped} protected entries"
    ))
}
fn default_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(t) = std::env::var("TEMP") {
        v.push(PathBuf::from(t))
    }
    if let Ok(w) = std::env::var("WINDIR") {
        v.push(Path::new(&w).join("Temp"));
        v.push(Path::new(&w).join("Prefetch"));
    }
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_dir_safe() {
        assert!(clean(Some(&vec!["Z:\\missing".into()])).is_ok())
    }

    #[test]
    fn cleans_only_the_isolated_fixture_directory() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_nanos();
        let fixture = std::env::temp_dir().join(format!("temp-files-cleanup-test-{suffix}"));
        fs::create_dir_all(fixture.join("nested")).expect("create fixture");
        fs::write(fixture.join("root.txt"), b"fixture").expect("write root file");
        fs::write(fixture.join("nested").join("child.txt"), b"fixture").expect("write nested file");

        let result =
            clean(Some(&vec![fixture.to_string_lossy().into_owned()])).expect("cleanup fixture");

        assert!(result.contains("removed 2 entries"));
        assert!(
            fixture.exists(),
            "the configured directory itself is retained"
        );
        assert!(!fixture.join("root.txt").exists());
        assert!(!fixture.join("nested").exists());
        fs::remove_dir(&fixture).expect("remove empty fixture directory");
    }
}

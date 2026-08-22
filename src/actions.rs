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
                tracing::info!(title=%title,message=%message,"notification");
                Ok(format!("{title}: {message}"))
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
        for entry in fs::read_dir(&p)? {
            let path = entry?.path();
            let r = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            match r {
                Ok(_) => removed += 1,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => skipped += 1,
                Err(e) => return Err(e.into()),
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
        v.push(Path::new(&w).join("Temp"))
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

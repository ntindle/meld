use crate::config::Config;
use crate::errors::{MeldError, Result};
use std::io::Write;
use std::path::PathBuf;

/// Advisory single-instance lock. Created with O_EXCL; removed on drop.
/// Stale locks (process no longer alive) are reclaimed automatically.
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    pub fn acquire(cfg: &Config) -> Result<Lock> {
        let path = cfg.lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                Ok(Lock { path })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&path) {
                    std::fs::remove_file(&path)?;
                    return Lock::acquire(cfg);
                }
                Err(MeldError::Locked(path.display().to_string()).into())
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn lock_is_stale(path: &std::path::Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return true;
    };
    let Ok(pid) = raw.trim().parse::<i32>() else {
        return true;
    };
    // kill(pid, 0) checks process existence without signaling.
    unsafe { libc_kill(pid, 0) != 0 }
}

extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

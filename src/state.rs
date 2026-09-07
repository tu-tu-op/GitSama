use std::{fs, path::PathBuf, process};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    logging,
    paths::AppPaths,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PendingBranch {
    pub repository: String,
    pub branch: String,
    pub object_id: String,
    pub created_at_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RecentBranch {
    branch: String,
    expires_at_ms: u128,
}

pub fn write_pending(paths: &AppPaths, pending: &PendingBranch) -> Result<()> {
    paths.ensure_layout()?;
    let path = pending_path(paths, &pending.repository, &pending.branch);
    let text = serde_json::to_string(pending)
        .map_err(|error| Error::message(format!("could not serialize branch state: {error}")))?;
    fs::write(&path, text).map_err(|source| Error::WriteFile { path, source })
}

pub fn take_pending(
    paths: &AppPaths,
    repository: &str,
    branch: &str,
    max_age_ms: u64,
) -> Result<Option<PendingBranch>> {
    let path = pending_path(paths, repository, branch);
    let Some(claim) = claim_file(&path)? else {
        return Ok(None);
    };

    let result = fs::read_to_string(&claim)
        .map_err(|source| Error::ReadFile {
            path: claim.clone(),
            source,
        })
        .and_then(|text| {
            serde_json::from_str::<PendingBranch>(&text)
                .map_err(|error| Error::message(format!("corrupted branch state: {error}")))
        });
    let _ = fs::remove_file(&claim);

    match result {
        Ok(pending) if !expired(pending.created_at_ms, max_age_ms) => Ok(Some(pending)),
        Ok(_) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn mark_recent(paths: &AppPaths, repository: &str, branch: &str, age_ms: u64) -> Result<()> {
    paths.ensure_layout()?;
    let path = recent_path(paths, repository, branch);
    let recent = RecentBranch {
        branch: branch.to_owned(),
        expires_at_ms: logging::now_millis() + u128::from(age_ms),
    };
    let text = serde_json::to_string(&recent)
        .map_err(|error| Error::message(format!("could not serialize branch marker: {error}")))?;
    fs::write(&path, text).map_err(|source| Error::WriteFile { path, source })
}

pub fn take_recent(paths: &AppPaths, repository: &str, branch: &str) -> Result<bool> {
    let path = recent_path(paths, repository, branch);
    let Some(claim) = claim_file(&path)? else {
        return Ok(false);
    };
    let result = fs::read_to_string(&claim)
        .ok()
        .and_then(|text| serde_json::from_str::<RecentBranch>(&text).ok())
        .is_some_and(|recent| {
            recent.branch == branch && recent.expires_at_ms >= logging::now_millis()
        });
    let _ = fs::remove_file(&claim);
    Ok(result)
}

pub fn clear_recent(paths: &AppPaths, repository: &str, branch: &str) {
    let _ = fs::remove_file(recent_path(paths, repository, branch));
}

pub fn pending_path(paths: &AppPaths, repository: &str, branch: &str) -> PathBuf {
    paths.state.join(format!(
        "pending-{}.json",
        stable_key(&format!("{repository}\0{branch}"))
    ))
}

fn recent_path(paths: &AppPaths, repository: &str, branch: &str) -> PathBuf {
    paths.state.join(format!(
        "recent-{}.json",
        stable_key(&format!("{repository}\0{branch}"))
    ))
}

fn claim_file(path: &PathBuf) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let claim = path.with_extension(format!("claim-{}", process::id()));
    match fs::rename(path, &claim) {
        Ok(()) => Ok(Some(claim)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::WriteFile {
            path: path.clone(),
            source: error,
        }),
    }
}

fn expired(created_at_ms: u128, max_age_ms: u64) -> bool {
    logging::now_millis().saturating_sub(created_at_ms) > u128::from(max_age_ms)
}

fn stable_key(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{PendingBranch, pending_path, take_pending, write_pending};
    use crate::{logging, paths::AppPaths};

    #[test]
    fn pending_state_is_consumed_once() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join(".gitsama"));
        let pending = PendingBranch {
            repository: "repo".to_owned(),
            branch: "feature".to_owned(),
            object_id: "abc".to_owned(),
            created_at_ms: logging::now_millis(),
        };
        write_pending(&paths, &pending).expect("write");
        assert_eq!(
            take_pending(&paths, "repo", "feature", 5000)
                .expect("take")
                .expect("pending")
                .branch,
            "feature"
        );
        assert!(
            take_pending(&paths, "repo", "feature", 5000)
                .expect("take")
                .is_none()
        );
    }

    #[test]
    fn expired_state_is_ignored() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join(".gitsama"));
        let pending = PendingBranch {
            repository: "repo".to_owned(),
            branch: "old".to_owned(),
            object_id: "abc".to_owned(),
            created_at_ms: 0,
        };
        write_pending(&paths, &pending).expect("write");
        assert!(
            take_pending(&paths, "repo", "old", 1)
                .expect("take")
                .is_none()
        );
        assert!(!pending_path(&paths, "repo", "old").exists());
    }

    #[test]
    fn recent_state_is_scoped_to_repository_and_branch() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join(".gitsama"));
        super::mark_recent(&paths, "repo-a", "main", 5000).expect("mark");
        assert!(super::take_recent(&paths, "repo-a", "main").expect("take"));
        super::mark_recent(&paths, "repo-a", "main", 5000).expect("mark");
        assert!(!super::take_recent(&paths, "repo-b", "main").expect("other repo"));
        assert!(!super::take_recent(&paths, "repo-a", "feature").expect("other branch"));
    }
}

use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

use crate::{
    error::{Error, Result},
    events::EventKind,
    git,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushRef {
    pub local_ref: String,
    pub local_object_id: String,
    pub remote_ref: String,
    pub remote_object_id: String,
}

impl PushRef {
    pub fn is_deleted(&self) -> bool {
        git::is_zero_oid(&self.local_object_id)
    }

    pub fn is_branch(&self) -> bool {
        self.local_ref.starts_with("refs/heads/")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PushClassification {
    pub event: EventKind,
    pub commit_count: u64,
}

pub fn parse_stdin(text: &str) -> Result<Vec<PushRef>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_line)
        .collect()
}

pub fn parse_line(line: &str) -> Result<PushRef> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 4 {
        return Err(Error::message(format!(
            "pre-push ref line should have four fields, got {}",
            parts.len()
        )));
    }
    Ok(PushRef {
        local_ref: parts[0].to_owned(),
        local_object_id: parts[1].to_owned(),
        remote_ref: parts[2].to_owned(),
        remote_object_id: parts[3].to_owned(),
    })
}

pub fn classify(refs: &[PushRef], remote_name: &str, threshold: u32) -> Result<PushClassification> {
    if threshold == 0 {
        return Err(Error::message("push threshold must be at least 1"));
    }

    let mut seen = HashSet::new();
    let mut exclusions = remote_tracking_tips(remote_name)?;
    if exclusions.is_empty() {
        exclusions = local_branch_tips(refs)?;
    }

    for push_ref in refs.iter().filter(|push_ref| {
        push_ref.is_branch()
            && !push_ref.is_deleted()
            && !git::is_zero_oid(&push_ref.local_object_id)
    }) {
        let mut args = vec!["rev-list".to_owned(), push_ref.local_object_id.clone()];
        if git::is_zero_oid(&push_ref.remote_object_id) {
            if !exclusions.is_empty() {
                args.push("--not".to_owned());
                args.extend(exclusions.iter().cloned());
            }
        } else {
            args.push("--not".to_owned());
            args.push(push_ref.remote_object_id.clone());
        }

        stream_rev_list(&args, |line| {
            let object_id = line.trim();
            if !object_id.is_empty() {
                seen.insert(object_id.to_owned());
            }
            seen.len() < threshold as usize
        })?;

        if seen.len() >= threshold as usize {
            return Ok(PushClassification {
                event: EventKind::MassivePush,
                commit_count: seen.len() as u64,
            });
        }
    }

    Ok(classify_count(seen.len() as u64, threshold))
}

pub fn classify_count(commit_count: u64, threshold: u32) -> PushClassification {
    PushClassification {
        event: if commit_count >= u64::from(threshold) {
            EventKind::MassivePush
        } else {
            EventKind::Push
        },
        commit_count,
    }
}

fn remote_tracking_tips(remote_name: &str) -> Result<Vec<String>> {
    let prefix = format!("refs/remotes/{remote_name}");
    let output = git::checked(["for-each-ref", "--format=%(objectname)", &prefix])?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn local_branch_tips(pushed_refs: &[PushRef]) -> Result<Vec<String>> {
    let output = git::checked(["for-each-ref", "--format=%(objectname)", "refs/heads/"])?;
    let pushed: HashSet<&str> = pushed_refs
        .iter()
        .map(|push_ref| push_ref.local_object_id.as_str())
        .collect();
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !pushed.contains(value))
        .map(ToOwned::to_owned)
        .collect())
}

fn stream_rev_list<F>(args: &[String], mut should_continue: F) -> Result<()>
where
    F: FnMut(&str) -> bool,
{
    let mut child = Command::new(git::git_binary())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::Git(format!("could not start rev-list: {error}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Git("rev-list stdout was not available".to_owned()))?;
    let reader = BufReader::new(stdout);
    let mut stopped = false;
    for line in reader.lines() {
        let line = line.map_err(|error| Error::Git(format!("could not read rev-list: {error}")))?;
        if !should_continue(&line) {
            stopped = true;
            break;
        }
    }

    if stopped {
        let _ = child.kill();
        let _ = child.wait();
        return Ok(());
    }

    let output = child
        .wait_with_output()
        .map_err(|error| Error::Git(format!("could not finish rev-list: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{PushRef, classify_count, parse_line, parse_stdin};
    use crate::{events::EventKind, git};

    fn ref_line(local: &str, remote: &str) -> String {
        format!("refs/heads/main {} refs/heads/main {remote}", local)
    }

    #[test]
    fn parses_push_stdin_and_deletions() {
        let parsed = parse_line(&ref_line(&"a".repeat(40), &"b".repeat(40))).expect("line");
        assert!(parsed.is_branch());
        assert!(!parsed.is_deleted());

        let deleted = parse_line(&ref_line(&"0".repeat(40), &"b".repeat(40))).expect("line");
        assert!(deleted.is_deleted());
        assert_eq!(parse_stdin("").expect("empty"), Vec::<PushRef>::new());
        assert!(parse_line("too short").is_err());
    }

    #[test]
    fn threshold_is_inclusive() {
        assert_eq!(classify_count(49, 50).event, EventKind::Push);
        assert_eq!(classify_count(50, 50).event, EventKind::MassivePush);
        assert_eq!(classify_count(51, 50).event, EventKind::MassivePush);
    }

    #[test]
    fn zero_oid_is_shared_with_git_helpers() {
        assert!(git::is_zero_oid(&"0".repeat(40)));
    }
}

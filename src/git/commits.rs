//! Git commit analysis

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::Repository;
use std::path::Path;

/// Information about a commit
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub timestamp: DateTime<Utc>,
}

/// Get recent commits
pub fn get_recent_commits(path: &Path, limit: usize) -> Result<Vec<CommitInfo>> {
    let repo = Repository::discover(path)
        .with_context(|| format!("Failed to find git repository at {}", path.display()))?;

    let mut commits = Vec::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let author = commit.author();
        let timestamp = Utc.timestamp_opt(commit.time().seconds(), 0)
            .single()
            .unwrap_or_else(Utc::now);

        commits.push(CommitInfo {
            id: oid.to_string(),
            short_id: oid.to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp,
        });
    }

    Ok(commits)
}

/// Get commits since a reference
pub fn get_commits_since(path: &Path, since: &str) -> Result<Vec<CommitInfo>> {
    let repo = Repository::discover(path)?;

    let since_obj = repo.revparse_single(since)?;
    let since_commit = since_obj.peel_to_commit()?;

    let mut commits = Vec::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.hide(since_commit.id())?;

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let author = commit.author();
        let timestamp = Utc.timestamp_opt(commit.time().seconds(), 0)
            .single()
            .unwrap_or_else(Utc::now);

        commits.push(CommitInfo {
            id: oid.to_string(),
            short_id: oid.to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp,
        });
    }

    Ok(commits)
}

/// Get the merge base between two branches
pub fn get_merge_base(path: &Path, branch1: &str, branch2: &str) -> Result<String> {
    let repo = Repository::discover(path)?;

    let obj1 = repo.revparse_single(branch1)?;
    let obj2 = repo.revparse_single(branch2)?;

    let oid1 = obj1.peel_to_commit()?.id();
    let oid2 = obj2.peel_to_commit()?.id();

    let merge_base = repo.merge_base(oid1, oid2)?;

    Ok(merge_base.to_string())
}

/// Check if a ref exists
pub fn ref_exists(path: &Path, ref_name: &str) -> Result<bool> {
    let repo = Repository::discover(path)?;
    let result = repo.revparse_single(ref_name).is_ok();
    Ok(result)
}

/// Get all branches
pub fn get_branches(path: &Path) -> Result<Vec<String>> {
    let repo = Repository::discover(path)?;
    let branches = repo.branches(None)?;

    let mut names = Vec::new();
    for branch in branches {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            names.push(name.to_string());
        }
    }

    Ok(names)
}

/// Get all tags
pub fn get_tags(path: &Path) -> Result<Vec<String>> {
    let repo = Repository::discover(path)?;
    let mut tags = Vec::new();

    repo.tag_foreach(|_, name| {
        if let Ok(name_str) = std::str::from_utf8(name) {
            if let Some(stripped) = name_str.strip_prefix("refs/tags/") {
                tags.push(stripped.to_string());
            }
        }
        true
    })?;

    Ok(tags)
}

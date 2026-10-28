//! Thin wrappers over the `git` CLI plus a `status --porcelain=v2 --branch -z` parser.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitFileStatus {
    pub path: String,
    /// Status letter: M, A, D, R, C, T (or U for unmerged).
    pub status: char,
    /// Original path for renames / copies.
    pub orig_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GitStatus {
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<GitFileStatus>,
    pub unstaged: Vec<GitFileStatus>,
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    NotARepo,
    /// git exited non-zero (stderr) or could not be spawned.
    Failed(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotARepo => write!(f, "Not a git repository"),
            GitError::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for GitError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,           // canonicalized when it exists, raw otherwise
    pub head: String,           // "" for bare
    pub branch: Option<String>, // "refs/heads/" stripped; None if detached/bare
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
    pub is_main: bool,          // first entry of the list
    pub is_current: bool,       // canonical(path) == canonical(dir)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Branches {
    pub local: Vec<String>,
    pub remote: Vec<String>,
}

/// Parse the stdout of `git status --porcelain=v2 --branch -z`.
pub fn parse_status(out: &str) -> GitStatus {
    let mut st = GitStatus::default();
    let mut recs = out.split('\0').filter(|r| !r.is_empty());
    while let Some(rec) = recs.next() {
        if let Some(h) = rec.strip_prefix("# ") {
            let (k, v) = h.split_once(' ').unwrap_or((h, ""));
            match k {
                "branch.head" if v == "(detached)" => st.detached = true,
                "branch.head" => st.branch = Some(v.to_string()),
                "branch.upstream" => st.upstream = Some(v.to_string()),
                "branch.ab" => {
                    let mut p = v.split(' ');
                    st.ahead = num(p.next(), '+');
                    st.behind = num(p.next(), '-');
                }
                _ => {}
            }
        } else if let Some(p) = rec.strip_prefix("? ") {
            st.untracked.push(p.to_string());
        } else if rec.starts_with("1 ") || rec.starts_with("2 ") || rec.starts_with("u ") {
            // Path is the last field and may contain spaces: split a fixed number of times.
            let (n, orig) = match rec.as_bytes()[0] {
                b'1' => (9, None),
                b'2' => (10, recs.next().map(str::to_string)), // `<path>\0<origPath>`
                _ => (11, None),
            };
            let f: Vec<&str> = rec.splitn(n, ' ').collect();
            if f.len() < n {
                continue;
            }
            let mut xy = f[1].chars();
            let (x, y) = (xy.next().unwrap_or('.'), xy.next().unwrap_or('.'));
            let path = f[n - 1].to_string();
            if rec.starts_with("u ") {
                st.unstaged.push(GitFileStatus {
                    path,
                    status: 'U',
                    orig_path: None,
                });
                continue;
            }
            if x != '.' && x != '?' {
                st.staged.push(GitFileStatus {
                    path: path.clone(),
                    status: x,
                    orig_path: orig.clone(),
                });
            }
            if y != '.' && y != '?' {
                st.unstaged.push(GitFileStatus {
                    path,
                    status: y,
                    orig_path: orig,
                });
            }
        }
    }
    st
}

fn num(s: Option<&str>, sign: char) -> u32 {
    s.and_then(|s| s.strip_prefix(sign))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

async fn run(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| GitError::Failed(format!("failed to run git: {e}")))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if err.contains("not a git repository") {
        Err(GitError::NotARepo)
    } else {
        Err(GitError::Failed(err))
    }
}

pub async fn git_status(dir: &Path) -> Result<GitStatus, GitError> {
    run(dir, &["status", "--porcelain=v2", "--branch", "-z"])
        .await
        .map(|s| parse_status(&s))
}
pub async fn git_stage(dir: &Path, path: &str) -> Result<(), GitError> {
    run(dir, &["add", "--", path]).await.map(drop)
}
pub async fn git_unstage(dir: &Path, path: &str) -> Result<(), GitError> {
    run(dir, &["restore", "--staged", "--", path])
        .await
        .map(drop)
}
pub async fn git_add_all(dir: &Path) -> Result<(), GitError> {
    run(dir, &["add", "-u"]).await.map(drop)
}
pub async fn git_commit(dir: &Path, msg: &str) -> Result<(), GitError> {
    run(dir, &["commit", "-m", msg]).await.map(drop)
}
pub async fn git_push(dir: &Path) -> Result<(), GitError> {
    run(dir, &["push"]).await.map(drop)
}
pub async fn git_pull(dir: &Path) -> Result<(), GitError> {
    run(dir, &["pull"]).await.map(drop)
}
pub async fn git_fetch(dir: &Path) -> Result<(), GitError> {
    run(dir, &["fetch", "--prune"]).await.map(drop)
}
pub async fn git_init(dir: &Path) -> Result<(), GitError> {
    run(dir, &["init", "."]).await.map(drop)
}
pub async fn git_status_text(dir: &Path) -> Result<String, GitError> {
    run(dir, &["status"]).await
}

/// Absolute, canonical `--git-common-dir` (same for every worktree of a repo).
pub async fn common_dir(dir: &Path) -> Result<PathBuf, GitError> {
    let out = run(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await?;
    let trimmed = out.trim();
    let p = PathBuf::from(trimmed);
    Ok(std::fs::canonicalize(&p).unwrap_or(p))
}

pub async fn is_tracked(dir: &Path, rel: &str) -> bool {
    run(dir, &["ls-files", "--error-unmatch", "--", rel])
        .await
        .is_ok()
}

pub async fn ensure_excluded(dir: &Path, rel: &str) -> Result<(), GitError> {
    if run(dir, &["check-ignore", "-q", "--", rel]).await.is_ok() {
        return Ok(());
    }
    let out = run(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-path", "info/exclude"],
    )
    .await?;
    let exclude_path = PathBuf::from(out.trim());
    if let Some(parent) = exclude_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| GitError::Failed(e.to_string()))?;
        }
    }
    let mut content = match std::fs::read_to_string(&exclude_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(GitError::Failed(e.to_string())),
    };
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    // Anchor relative to the repo root: `dir` may be a subdirectory of the repo.
    let prefix = run(dir, &["rev-parse", "--show-prefix"]).await?;
    content.push_str(&format!("/{}{rel}\n", prefix.trim()));
    std::fs::write(&exclude_path, content)
        .map_err(|e| GitError::Failed(e.to_string()))?;
    Ok(())
}

/// Pure parser for the stdout of `git worktree list --porcelain`.
pub fn parse_worktrees(out: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut cur: Option<Worktree> = None;

    for line in out.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if let Some(mut wt) = cur.take() {
                if wt.bare {
                    wt.head.clear();
                    wt.branch = None;
                }
                if wt.detached {
                    wt.branch = None;
                }
                worktrees.push(wt);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(mut wt) = cur.take() {
                if wt.bare {
                    wt.head.clear();
                    wt.branch = None;
                }
                if wt.detached {
                    wt.branch = None;
                }
                worktrees.push(wt);
            }
            let mut wt = Worktree::default();
            wt.path = path.to_string();
            cur = Some(wt);
        } else if let Some(wt) = &mut cur {
            if let Some(sha) = line.strip_prefix("HEAD ") {
                wt.head = sha.trim().to_string();
            } else if let Some(branch_ref) = line.strip_prefix("branch ") {
                let r = branch_ref.trim();
                let stripped = r.strip_prefix("refs/heads/").unwrap_or(r);
                wt.branch = Some(stripped.to_string());
            } else if line == "detached" {
                wt.detached = true;
            } else if line == "bare" {
                wt.bare = true;
            } else if line == "locked" || line.starts_with("locked ") {
                wt.locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                wt.prunable = true;
            }
        }
    }

    if let Some(mut wt) = cur.take() {
        if wt.bare {
            wt.head.clear();
            wt.branch = None;
        }
        if wt.detached {
            wt.branch = None;
        }
        worktrees.push(wt);
    }

    if let Some(first) = worktrees.first_mut() {
        first.is_main = true;
    }

    worktrees
}

pub async fn git_worktree_list(dir: &Path) -> Result<Vec<Worktree>, GitError> {
    let out = run(dir, &["worktree", "list", "--porcelain"]).await?;
    let mut entries = parse_worktrees(&out);
    let cur_dir = std::fs::canonicalize(dir)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| dir.to_string_lossy().into_owned());

    for entry in &mut entries {
        let p = std::fs::canonicalize(&entry.path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| entry.path.clone());
        entry.path = p;
        entry.is_current = entry.path == cur_dir;
    }

    Ok(entries)
}

pub async fn git_worktree_add(
    dir: &Path,
    branch: &str,
    new_branch: bool,
    base: Option<&str>,
) -> Result<String, GitError> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(GitError::Failed("Branch name is required".to_string()));
    }
    if branch.starts_with('-') || base.map_or(false, |b| b.trim().starts_with('-')) {
        return Err(GitError::Failed("Branch names cannot start with '-'".to_string()));
    }

    let worktrees = git_worktree_list(dir).await?;
    let m = worktrees
        .first()
        .ok_or_else(|| GitError::Failed("No main worktree found".to_string()))?;
    let m_path = Path::new(&m.path);
    let parent = m_path
        .parent()
        .ok_or_else(|| GitError::Failed("Main worktree has no parent directory".to_string()))?;
    let main_name = m_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| GitError::Failed("Invalid main worktree directory name".to_string()))?;

    let sanitized: String = branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(GitError::Failed("Invalid branch name".to_string()));
    }

    let wt_dir = parent.join(format!("{main_name}.worktrees"));
    std::fs::create_dir_all(&wt_dir)
        .map_err(|e| GitError::Failed(format!("Failed to create worktrees dir: {e}")))?;
    let target_path = wt_dir.join(&sanitized);
    let target_str = target_path.to_string_lossy().into_owned();

    let mut args: Vec<&str> = Vec::new();
    if new_branch {
        args.extend_from_slice(&["worktree", "add", "-b", branch, &target_str]);
        if let Some(b) = base {
            let b_trimmed = b.trim();
            if !b_trimmed.is_empty() {
                args.push(b_trimmed);
            }
        }
    } else {
        args.extend_from_slice(&["worktree", "add", &target_str, branch]);
    }

    run(dir, &args).await?;
    std::fs::canonicalize(&target_path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| GitError::Failed(e.to_string()))
}

pub async fn git_worktree_remove(dir: &Path, path: &str, force: bool) -> Result<(), GitError> {
    let worktrees = git_worktree_list(dir).await?;
    let canonical_path = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());

    let entry = worktrees
        .iter()
        .find(|e| e.path == canonical_path)
        .ok_or_else(|| GitError::Failed("Not a worktree of this repository".to_string()))?;

    if entry.is_main {
        return Err(GitError::Failed("Cannot remove the main worktree".to_string()));
    }
    if entry.is_current {
        return Err(GitError::Failed("Cannot remove the worktree this tab is using".to_string()));
    }

    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&entry.path);

    run(dir, &args).await.map(drop)
}

pub async fn git_branches(dir: &Path) -> Result<Branches, GitError> {
    let out = run(
        dir,
        &["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes"],
    )
    .await?;
    let mut branches = Branches::default();
    for line in out.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("refs/heads/") {
            branches.local.push(name.to_string());
        } else if let Some(rem) = line.strip_prefix("refs/remotes/") {
            if !rem.ends_with("/HEAD") {
                branches.remote.push(rem.to_string());
            }
        }
    }
    Ok(branches)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OID: &str = "# branch.oid 3407fb07d54690264829d9d44119d3a754e77d2f\0";
    const H: &str = "100644 100644 100644 78981922613b2afb6025042ff6bd878ac1994e85 9ad2ebbaff6f3397bb65002dcf4294d8d6243982";

    fn f(path: &str, status: char) -> GitFileStatus {
        GitFileStatus {
            path: path.into(),
            status,
            orig_path: None,
        }
    }

    #[test]
    fn initial_commit_no_upstream() {
        let s = parse_status("# branch.oid (initial)\0# branch.head main\0");
        assert_eq!(
            s,
            GitStatus {
                branch: Some("main".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn clean_no_upstream() {
        let s = parse_status(&format!("{OID}# branch.head main\0"));
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert!(!s.detached && s.upstream.is_none());
        assert_eq!((s.ahead, s.behind), (0, 0));
        assert!(s.staged.is_empty() && s.unstaged.is_empty() && s.untracked.is_empty());
    }

    #[test]
    fn empty_output() {
        assert_eq!(parse_status(""), GitStatus::default());
    }

    #[test]
    fn untracked_with_spaces() {
        let s = parse_status(&format!("{OID}# branch.head main\0? un tracked\0? z\0"));
        assert_eq!(s.untracked, vec!["un tracked", "z"]);
        assert!(s.staged.is_empty() && s.unstaged.is_empty());
    }

    #[test]
    fn staged_only_add_modify_delete() {
        let inp = format!(
            "{OID}# branch.head main\0\
             1 M. N... {H} a\0\
             1 D. N... 100644 000000 000000 4bcfe98e640c8284511312660fb8709b0afa888e 0000000000000000000000000000000000000000 d\0\
             1 A. N... 000000 100644 100644 0000000000000000000000000000000000000000 8ba3a16384aacc37d01564b28401755ce8053f51 new\0"
        );
        let s = parse_status(&inp);
        assert_eq!(s.staged, vec![f("a", 'M'), f("d", 'D'), f("new", 'A')]);
        assert!(s.unstaged.is_empty());
    }

    #[test]
    fn unstaged_only_modify_delete() {
        let inp = format!(
            "{OID}# branch.head main\0\
             1 .D N... 100644 100644 000000 61780798228d17af2d34fce4cfbdf35556832472 61780798228d17af2d34fce4cfbdf35556832472 b\0\
             1 .M N... {H} c\0"
        );
        let s = parse_status(&inp);
        assert_eq!(s.unstaged, vec![f("b", 'D'), f("c", 'M')]);
        assert!(s.staged.is_empty());
    }

    #[test]
    fn both_staged_and_unstaged() {
        let inp = format!("{OID}# branch.head main\01 MM N... {H} a\01 AM N... {H} sp ace.rs\0");
        let s = parse_status(&inp);
        assert_eq!(s.staged, vec![f("a", 'M'), f("sp ace.rs", 'A')]);
        assert_eq!(s.unstaged, vec![f("a", 'M'), f("sp ace.rs", 'M')]);
    }

    #[test]
    fn rename_with_worktree_modification() {
        // captured: `git mv c c_renamed` on a file that was already modified
        let inp = format!(
            "{OID}# branch.head main\0\
             2 RM N... {H} R100 c_renamed\0c\0\
             1 D. N... 100644 000000 000000 4bcfe98e640c8284511312660fb8709b0afa888e 0000000000000000000000000000000000000000 d\0\
             ? q\0"
        );
        let s = parse_status(&inp);
        let ren = |st| GitFileStatus {
            path: "c_renamed".into(),
            status: st,
            orig_path: Some("c".into()),
        };
        assert_eq!(s.staged, vec![ren('R'), f("d", 'D')]); // origPath record consumed, not mis-parsed
        assert_eq!(s.unstaged, vec![ren('M')]);
        assert_eq!(s.untracked, vec!["q"]);
    }

    #[test]
    fn copy_entry() {
        let inp = format!("{OID}# branch.head main\02 C. N... {H} C75 copy.txt\0orig.txt\0");
        let s = parse_status(&inp);
        assert_eq!(
            s.staged,
            vec![GitFileStatus {
                path: "copy.txt".into(),
                status: 'C',
                orig_path: Some("orig.txt".into())
            }]
        );
    }

    #[test]
    fn unmerged() {
        let inp = format!(
            "{OID}# branch.head main\0u UU N... 100644 100644 100644 100644 aaaa bbbb cccc conflict me.txt\0"
        );
        let s = parse_status(&inp);
        assert_eq!(s.unstaged, vec![f("conflict me.txt", 'U')]);
        assert!(s.staged.is_empty());
    }

    #[test]
    fn detached_head() {
        let s = parse_status("# branch.oid 6958aed4d2a4b7750b70833f5ec0a6399f646817\0# branch.head (detached)\0? d\0");
        assert!(s.detached);
        assert_eq!(s.branch, None);
        assert_eq!(s.untracked, vec!["d"]);
    }

    #[test]
    fn upstream_in_sync_and_ahead_behind() {
        let base = "# branch.oid f888590a\0# branch.head main\0# branch.upstream origin/main\0";
        let s = parse_status(&format!("{base}# branch.ab +0 -0\0"));
        assert_eq!(
            (s.upstream.as_deref(), s.ahead, s.behind),
            (Some("origin/main"), 0, 0)
        );
        let s = parse_status(&format!("{base}# branch.ab +1 -0\0"));
        assert_eq!((s.ahead, s.behind), (1, 0));
        let s = parse_status(&format!("{base}# branch.ab +12 -34\0"));
        assert_eq!((s.ahead, s.behind), (12, 34));
    }

    #[test]
    fn upstream_gone_has_no_ab() {
        let s = parse_status("# branch.head main\0# branch.upstream origin/main\0");
        assert_eq!(
            (s.upstream.as_deref(), s.ahead, s.behind),
            (Some("origin/main"), 0, 0)
        );
    }

    // ---- real git, throwaway repos ----

    struct Tmp(std::path::PathBuf);
    impl Tmp {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("git-spike-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn non_git_dir_is_detected() {
        let t = Tmp::new("nogit");
        // GIT_CEILING_DIRECTORIES-free: /tmp is not inside a repo.
        assert_eq!(git_status(&t.0).await, Err(GitError::NotARepo));
        assert_eq!(git_status_text(&t.0).await, Err(GitError::NotARepo));
    }

    #[tokio::test]
    async fn end_to_end_stage_unstage_commit() {
        let t = Tmp::new("e2e");
        let d = &t.0;
        run(d, &["init", "-q", "-b", "main"]).await.unwrap();
        run(d, &["config", "user.email", "a@b"]).await.unwrap();
        run(d, &["config", "user.name", "n"]).await.unwrap();
        std::fs::write(d.join("a"), "1").unwrap();
        git_stage(d, "a").await.unwrap();
        git_commit(d, "init").await.unwrap();

        std::fs::write(d.join("a"), "2").unwrap();
        std::fs::write(d.join("new"), "x").unwrap();
        let s = git_status(d).await.unwrap();
        assert_eq!(
            (s.staged.len(), s.unstaged, s.untracked),
            (0, vec![f("a", 'M')], vec!["new".to_string()])
        );

        git_add_all(d).await.unwrap(); // tracked only
        let s = git_status(d).await.unwrap();
        assert_eq!(s.staged, vec![f("a", 'M')]);
        assert_eq!(s.untracked, vec!["new"]);

        git_unstage(d, "a").await.unwrap();
        assert_eq!(git_status(d).await.unwrap().unstaged, vec![f("a", 'M')]);

        git_stage(d, "a").await.unwrap();
        git_commit(d, "two").await.unwrap();
        assert!(git_status_text(d)
            .await
            .unwrap()
            .contains("Untracked files"));
        // no upstream: push fails with stderr, doesn't hang
        assert!(matches!(git_push(d).await, Err(GitError::Failed(_))));
    }

    #[test]
    fn parse_worktrees_porcelain() {
        let fixture = "\
worktree /path/to/bare
bare

worktree /path/to/main
HEAD 1111111111111111111111111111111111111111
branch refs/heads/main

worktree /path/to/detached
HEAD 2222222222222222222222222222222222222222
detached

worktree /path/to/locked
HEAD 3333333333333333333333333333333333333333
branch refs/heads/feature-locked
locked reason for lock

worktree /path/to/prunable
HEAD 4444444444444444444444444444444444444444
branch refs/heads/feature-prunable
prunable gitdir file points to non-existent location
";
        let wts = parse_worktrees(fixture);
        assert_eq!(wts.len(), 5);

        assert_eq!(wts[0].path, "/path/to/bare");
        assert!(wts[0].bare);
        assert_eq!(wts[0].head, "");
        assert_eq!(wts[0].branch, None);
        assert!(wts[0].is_main);
        assert!(!wts[0].is_current);

        assert_eq!(wts[1].path, "/path/to/main");
        assert_eq!(wts[1].head, "1111111111111111111111111111111111111111");
        assert_eq!(wts[1].branch.as_deref(), Some("main"));
        assert!(!wts[1].bare);
        assert!(!wts[1].detached);
        assert!(!wts[1].is_main);
        assert!(!wts[1].is_current);

        assert_eq!(wts[2].path, "/path/to/detached");
        assert_eq!(wts[2].head, "2222222222222222222222222222222222222222");
        assert!(wts[2].detached);
        assert_eq!(wts[2].branch, None);
        assert!(!wts[2].is_main);

        assert_eq!(wts[3].path, "/path/to/locked");
        assert!(wts[3].locked);
        assert_eq!(wts[3].branch.as_deref(), Some("feature-locked"));
        assert!(!wts[3].is_main);

        assert_eq!(wts[4].path, "/path/to/prunable");
        assert!(wts[4].prunable);
        assert_eq!(wts[4].branch.as_deref(), Some("feature-prunable"));
        assert!(!wts[4].is_main);
    }

    #[tokio::test]
    async fn exclude_and_worktree_lifecycle() {
        let t = Tmp::new("wt-lifecycle");
        let d = &t.0;
        run(d, &["init", "-q", "-b", "main"]).await.unwrap();
        run(d, &["config", "user.email", "a@b"]).await.unwrap();
        run(d, &["config", "user.name", "n"]).await.unwrap();
        std::fs::write(d.join("a"), "1").unwrap();
        git_stage(d, "a").await.unwrap();
        git_commit(d, "init").await.unwrap();

        assert!(is_tracked(d, "a").await);
        assert!(!is_tracked(d, "b").await);

        let common = common_dir(d).await.unwrap();
        assert!(common.ends_with(".git"));

        // ensure_excluded twice -> contains /.omp/config.yml exactly once
        ensure_excluded(d, ".omp/config.yml").await.unwrap();
        ensure_excluded(d, ".omp/config.yml").await.unwrap();
        let excl_path = run(
            d,
            &["rev-parse", "--path-format=absolute", "--git-path", "info/exclude"],
        )
        .await
        .unwrap();
        let content = std::fs::read_to_string(excl_path.trim()).unwrap();
        assert_eq!(content.matches("/.omp/config.yml").count(), 1);

        // repo with committed .gitignore having .omp/config.yml
        let t2 = Tmp::new("wt-lifecycle-2");
        let d2 = &t2.0;
        run(d2, &["init", "-q", "-b", "main"]).await.unwrap();
        run(d2, &["config", "user.email", "a@b"]).await.unwrap();
        run(d2, &["config", "user.name", "n"]).await.unwrap();
        std::fs::write(d2.join(".gitignore"), "/.omp/config.yml\n").unwrap();
        git_stage(d2, ".gitignore").await.unwrap();
        git_commit(d2, "ignore").await.unwrap();
        ensure_excluded(d2, ".omp/config.yml").await.unwrap();
        let excl_path2 = run(
            d2,
            &["rev-parse", "--path-format=absolute", "--git-path", "info/exclude"],
        )
        .await
        .unwrap();
        let p2 = PathBuf::from(excl_path2.trim());
        if p2.exists() {
            let content2 = std::fs::read_to_string(p2).unwrap();
            assert!(!content2.contains("/.omp/config.yml"));
        }

        // worktree add, list, remove
        let added_path = git_worktree_add(d, "feat/x", true, None).await.unwrap();
        assert!(added_path.ends_with("feat-x"));

        let list = git_worktree_list(d).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_main);
        assert!(list[0].is_current);
        assert!(!list[1].is_main);
        assert!(!list[1].is_current);
        assert_eq!(list[1].branch.as_deref(), Some("feat/x"));

        // git_worktree_list(<new path>) -> is_current on the linked one
        let list_from_new = git_worktree_list(Path::new(&added_path)).await.unwrap();
        assert_eq!(list_from_new.len(), 2);
        assert!(!list_from_new[0].is_current);
        assert!(list_from_new[1].is_current);

        let branches = git_branches(d).await.unwrap();
        assert!(branches.local.contains(&"main".to_string()));
        assert!(branches.local.contains(&"feat/x".to_string()));

        // git_worktree_remove(d, d_path, false) -> Err (main)
        let main_remove_res = git_worktree_remove(d, &d.to_string_lossy(), false).await;
        assert!(main_remove_res.is_err());

        // write <new>/.omp/config.yml, then remove without force -> Ok and dir gone
        let new_wt_path = PathBuf::from(&added_path);
        std::fs::create_dir_all(new_wt_path.join(".omp")).unwrap();
        std::fs::write(new_wt_path.join(".omp").join("config.yml"), "test: 1").unwrap();

        git_worktree_remove(d, &added_path, false).await.unwrap();
        assert!(!new_wt_path.exists());

        let list_after = git_worktree_list(d).await.unwrap();
        assert_eq!(list_after.len(), 1);

        // Clean up .worktrees directory
        if let Some(parent) = new_wt_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
}

    #[tokio::test]
    async fn ensure_excluded_in_subdirectory() {
        let t = Tmp::new("excl-subdir");
        let d = &t.0;
        run(d, &["init", "-q", "-b", "main"]).await.unwrap();
        let sub = d.join("sub");
        std::fs::create_dir_all(sub.join(".omp")).unwrap();
        std::fs::write(sub.join(".omp/config.yml"), "a: 1\n").unwrap();

        ensure_excluded(&sub, ".omp/config.yml").await.unwrap();
        ensure_excluded(&sub, ".omp/config.yml").await.unwrap();

        let status = git_status(d).await.unwrap();
        assert!(status.untracked.is_empty(), "{:?}", status.untracked);
        let excl = std::fs::read_to_string(d.join(".git/info/exclude")).unwrap();
        assert_eq!(excl.matches("/sub/.omp/config.yml").count(), 1);
    }
}

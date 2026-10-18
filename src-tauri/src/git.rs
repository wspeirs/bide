//! Thin wrappers over the `git` CLI plus a `status --porcelain=v2 --branch -z` parser.

use std::fmt;
use std::path::Path;

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
}

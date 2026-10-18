//! Path normalization shared by [`Workspace`](crate::Workspace) routing.

/// Normalize a filesystem path to its segments, resolving `.` and `..` (so a
/// path means the same as in a shell, and `..` cannot escape above the root)
/// and optionally stripping a shell-style mount prefix such as `/mnt`.
///
/// ```
/// use archil::to_segments;
/// assert_eq!(to_segments("/a/b/../c", None), ["a", "c"]);
/// assert_eq!(to_segments("/mnt/x", Some("/mnt")), ["x"]);
/// assert_eq!(to_segments("/../../etc/x", None), ["etc", "x"]);
/// ```
pub fn to_segments(path: &str, container_root: Option<&str>) -> Vec<String> {
    let mut p = path.trim().to_string();
    if let Some(root) = container_root.filter(|r| !r.is_empty()) {
        let root = root.trim_end_matches('/');
        if p == root {
            p = "/".into();
        } else if let Some(rest) = p.strip_prefix(&format!("{root}/")) {
            p = format!("/{rest}");
        }
    }
    let mut resolved: Vec<String> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                resolved.pop();
            }
            s => resolved.push(s.to_string()),
        }
    }
    resolved
}

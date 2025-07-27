/// Get the category for a tool based on its name
pub fn get_tool_category(tool_name: &str) -> String {
    match tool_name {
        "Edit" | "MultiEdit" | "Write" => "Code Generation".to_string(),
        "Read" | "LS" | "Glob" => "File Operations".to_string(),
        "Bash" => "System Operations".to_string(),
        "Grep" | "WebSearch" | "WebFetch" => "Search".to_string(),
        "TodoWrite" | "TodoRead" => "Planning".to_string(),
        _ => "Other".to_string(),
    }
}

/// Calculate Actions Per Minute (APM)
pub fn calculate_apm(message_count: u32, tool_count: u32, duration_minutes: f64) -> f64 {
    if duration_minutes <= 0.0 {
        return 0.0;
    }
    (message_count as f64 + tool_count as f64) / duration_minutes
}

/// Clean project name for display
pub fn clean_project_name(project_name: &str) -> String {
    project_name
        .replace("-Users-", "~/")
        .replace("-", "/")
        .trim_start_matches("~/")
        .to_string()
}

/// Create a safe file path by sanitizing the filename
pub fn create_safe_path(base_path: &std::path::Path, filename: &str) -> std::path::PathBuf {
    // Remove any path traversal attempts and invalid characters
    let sanitized = filename
        .replace("../", "_")
        .replace("..\\", "_")
        .replace("..", "")
        .replace("/", "_")
        .replace("\\", "_");
    
    base_path.join(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_category() {
        assert_eq!(get_tool_category("Edit"), "Code Generation");
        assert_eq!(get_tool_category("MultiEdit"), "Code Generation");
        assert_eq!(get_tool_category("Write"), "Code Generation");
        assert_eq!(get_tool_category("Read"), "File Operations");
        assert_eq!(get_tool_category("LS"), "File Operations");
        assert_eq!(get_tool_category("Bash"), "System Operations");
        assert_eq!(get_tool_category("Grep"), "Search");
        assert_eq!(get_tool_category("WebSearch"), "Search");
        assert_eq!(get_tool_category("TodoWrite"), "Planning");
        assert_eq!(get_tool_category("Unknown"), "Other");
    }

    #[test]
    fn test_calculate_apm() {
        assert_eq!(calculate_apm(10, 5, 1.0), 15.0);
        assert_eq!(calculate_apm(20, 10, 2.0), 15.0);
        assert_eq!(calculate_apm(0, 0, 1.0), 0.0);
        assert_eq!(calculate_apm(10, 5, 0.0), 0.0);
        assert_eq!(calculate_apm(10, 5, -1.0), 0.0);
    }

    #[test]
    fn test_clean_project_name() {
        assert_eq!(clean_project_name("-Users-john-projects-myapp"), "john/projects/myapp");
        assert_eq!(clean_project_name("~/Documents/project"), "Documents/project");
        assert_eq!(clean_project_name("simple-name"), "simple/name");
    }

    #[test]
    fn test_create_safe_path() {
        use std::path::Path;
        
        let base = Path::new("/tmp");
        assert_eq!(create_safe_path(base, "normal.txt"), Path::new("/tmp/normal.txt"));
        assert_eq!(create_safe_path(base, "../etc/passwd"), Path::new("/tmp/_etc_passwd"));
        assert_eq!(create_safe_path(base, "../../root/.ssh/key"), Path::new("/tmp/__root_.ssh_key"));
        assert_eq!(create_safe_path(base, "file\\with\\backslashes"), Path::new("/tmp/file_with_backslashes"));
    }
}
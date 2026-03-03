use std::path::Path;

/// Format a directory listing for display in @mention context.
///
/// Returns a string showing immediate directory contents with DIR/FILE markers.
/// Directories are listed first, then files, alphabetically within each group.
///
/// # Example output
///
/// ```text
/// Directory: /path/to/dir
///
///   DIR  subdir1
///   DIR  subdir2
///   FILE config.yaml
///   FILE README.md
/// ```
pub fn format_directory_listing(path: &Path) -> String {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            return format!("Directory: {}\n\n  (permission denied)", path.display());
        }
    };

    let mut items: Vec<(bool, String)> = Vec::new(); // (is_file, name)
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Use !is_dir() so symlinks to directories are classified as DIR (follows Python)
        let is_file = entry.file_type().map(|ft| !ft.is_dir()).unwrap_or(true);
        items.push((is_file, name));
    }

    // Sort: directories first (is_file=false < is_file=true), then case-insensitive alpha
    items.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });

    if items.is_empty() {
        return format!("Directory: {}\n\n  (empty directory)", path.display());
    }

    let lines: Vec<String> = items
        .iter()
        .map(|(is_file, name)| {
            let entry_type = if *is_file { "FILE" } else { "DIR " };
            format!("  {} {}", entry_type, name)
        })
        .collect();

    format!("Directory: {}\n\n{}", path.display(), lines.join("\n"))
}

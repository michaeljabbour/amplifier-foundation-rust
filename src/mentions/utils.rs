use std::path::Path;

/// Async version of [`format_directory_listing`] using `tokio::fs::read_dir`.
///
/// Identical behavior to the sync version — same sorting, same output format,
/// same error messages. Use this inside async contexts to avoid blocking the
/// executor.
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
pub async fn format_directory_listing_async(path: &Path) -> String {
    let mut read_dir = match tokio::fs::read_dir(path).await {
        Ok(rd) => rd,
        Err(_) => {
            return format!("Directory: {}\n\n  (permission denied)", path.display());
        }
    };

    let mut items: Vec<(bool, String)> = Vec::new();
    loop {
        match read_dir.next_entry().await {
            Ok(Some(entry)) => {
                let name = entry.file_name().to_string_lossy().to_string();
                // Use !is_dir() so symlinks to directories are classified as DIR (follows Python)
                let is_file = entry
                    .file_type()
                    .await
                    .map(|ft| !ft.is_dir())
                    .unwrap_or(true);
                items.push((is_file, name));
            }
            Ok(None) => break,  // end of directory
            Err(_) => continue, // skip errors, matching sync .flatten()
        }
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

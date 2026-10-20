use super::defaults;
use super::types::Template;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use once_cell::sync::Lazy;
use std::sync::RwLock;

// Global storage for the bundled templates directory path
static BUNDLED_TEMPLATES_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// Set the bundled templates directory path (called once at app startup)
pub fn set_bundled_templates_dir(path: PathBuf) {
    info!("Bundled templates directory set to: {:?}", path);
    if let Ok(mut dir) = BUNDLED_TEMPLATES_DIR.write() {
        *dir = Some(path);
    }
}

/// Get the user's custom templates directory path
///
/// Returns the platform-specific application data directory for custom templates:
/// - macOS: ~/Library/Application Support/Intake Pro/templates/
/// - Windows: %APPDATA%\Intake Pro\templates\
/// - Linux: ~/.config/Intake Pro/templates/
fn get_custom_templates_dir() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push("Intake Pro");
    path.push("templates");
    Some(path)
}

/// Load a template from the bundled resources directory
///
/// # Arguments
/// * `template_id` - Template identifier (without .json extension)
///
/// # Returns
/// The template JSON content if found, None otherwise
fn load_bundled_template(template_id: &str) -> Option<String> {
    let bundled_dir = BUNDLED_TEMPLATES_DIR.read().ok()?.clone()?;
    let template_path = bundled_dir.join(format!("{}.json", template_id));

    debug!("Checking for bundled template at: {:?}", template_path);

    match std::fs::read_to_string(&template_path) {
        Ok(content) => {
            info!("Loaded bundled template '{}' from {:?}", template_id, template_path);
            Some(content)
        }
        Err(e) => {
            debug!("No bundled template '{}' found: {}", template_id, e);
            None
        }
    }
}

/// Load a template from the user's custom templates directory
///
/// # Arguments
/// * `template_id` - Template identifier (without .json extension)
///
/// # Returns
/// The template JSON content if found, None otherwise
fn load_custom_template(template_id: &str) -> Option<String> {
    let custom_dir = get_custom_templates_dir()?;
    let template_path = custom_dir.join(format!("{}.json", template_id));

    debug!("Checking for custom template at: {:?}", template_path);

    match std::fs::read_to_string(&template_path) {
        Ok(content) => {
            info!("Loaded custom template '{}' from {:?}", template_id, template_path);
            Some(content)
        }
        Err(e) => {
            debug!("No custom template '{}' found: {}", template_id, e);
            None
        }
    }
}

/// Load and parse a template by identifier
///
/// This function implements a fallback strategy:
/// 1. Check user's custom templates directory
/// 2. Check bundled resources directory (app templates)
/// 3. Fall back to built-in embedded templates
/// 4. Return error if not found in any location
///
/// # Arguments
/// * `template_id` - Template identifier (e.g., "daily_standup", "standard_meeting")
///
/// # Returns
/// Parsed and validated Template struct
pub fn get_template(template_id: &str) -> Result<Template, String> {
    info!("Loading template: {}", template_id);

    // Try custom template first, then bundled, then built-in
    let json_content = if let Some(custom_content) = load_custom_template(template_id) {
        debug!("Using custom template for '{}'", template_id);
        custom_content
    } else if let Some(bundled_content) = load_bundled_template(template_id) {
        debug!("Using bundled template for '{}'", template_id);
        bundled_content
    } else if let Some(builtin_content) = defaults::get_builtin_template(template_id) {
        debug!("Using built-in template for '{}'", template_id);
        builtin_content.to_string()
    } else {
        return Err(format!(
            "Template '{}' not found. Available templates: {}",
            template_id,
            list_template_ids().join(", ")
        ));
    };

    // Parse and validate
    validate_and_parse_template(&json_content)
}

/// Validate and parse template JSON
///
/// # Arguments
/// * `json_content` - Raw JSON string
///
/// # Returns
/// Parsed and validated Template struct
pub fn validate_and_parse_template(json_content: &str) -> Result<Template, String> {
    let template: Template = serde_json::from_str(json_content)
        .map_err(|e| format!("Failed to parse template JSON: {}", e))?;

    template.validate()?;

    Ok(template)
}

/// List all available template identifiers
///
/// Returns a combined list of:
/// - Built-in template IDs
/// - Bundled template IDs (from app resources)
/// - Custom template IDs (from user's data directory)
pub fn list_template_ids() -> Vec<String> {
    let mut ids: Vec<String> = defaults::list_builtin_template_ids()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    // Add bundled templates if directory is set
    if let Ok(bundled_dir_lock) = BUNDLED_TEMPLATES_DIR.read() {
        if let Some(bundled_dir) = bundled_dir_lock.as_ref() {
            if bundled_dir.exists() {
                match std::fs::read_dir(bundled_dir) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    let id = filename.trim_end_matches(".json").to_string();
                                    if !ids.contains(&id) {
                                        ids.push(id);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read bundled templates directory: {}", e);
                    }
                }
            }
        }
    }

    // Add custom templates if directory exists
    if let Some(custom_dir) = get_custom_templates_dir() {
        if custom_dir.exists() {
            match std::fs::read_dir(&custom_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if let Some(filename) = entry.file_name().to_str() {
                            if filename.ends_with(".json") {
                                let id = filename.trim_end_matches(".json").to_string();
                                if !ids.contains(&id) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read custom templates directory: {}", e);
                }
            }
        }
    }

    ids.sort();
    ids
}

/// List all available templates with their metadata
///
/// Returns a list of (id, name, description) tuples
pub fn list_templates() -> Vec<(String, String, String)> {
    let mut templates = Vec::new();

    for id in list_template_ids() {
        match get_template(&id) {
            Ok(template) => {
                templates.push((id, template.name, template.description));
            }
            Err(e) => {
                warn!("Failed to load template '{}': {}", id, e);
            }
        }
    }

    templates
}

// ---------------------------------------------------------------------------
// Custom summary types (user-authored: a name and one prompt)
// ---------------------------------------------------------------------------

/// Prefix reserved for user-created summary types. Built-ins never use it, so a
/// custom type can neither shadow nor delete a built-in one.
pub const CUSTOM_PREFIX: &str = "custom_";
const MAX_NAME_CHARS: usize = 80;
const MAX_PROMPT_CHARS: usize = 4000;

pub fn is_custom_id(id: &str) -> bool {
    id.starts_with(CUSTOM_PREFIX)
        && id.len() > CUSTOM_PREFIX.len()
        && id[CUSTOM_PREFIX.len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    s.chars().take(24).collect()
}

fn custom_template_json(name: &str, prompt: &str) -> String {
    serde_json::json!({
        "name": name,
        "description": "Custom summary type",
        "sections": [{ "title": "Summary", "instruction": prompt, "format": "paragraph" }]
    })
    .to_string()
}

fn save_custom_in(
    dir: &std::path::Path,
    taken_names: &[(String, String)],
    id: Option<&str>,
    name: &str,
    prompt: &str,
) -> Result<String, String> {
    let name = name.trim();
    let prompt = prompt.trim();
    if name.is_empty() {
        return Err("Summary type name cannot be empty".into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("Summary type name is too long (max {MAX_NAME_CHARS})"));
    }
    if prompt.is_empty() {
        return Err("Prompt template cannot be empty".into());
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(format!("Prompt template is too long (max {MAX_PROMPT_CHARS})"));
    }
    if let Some(id) = id {
        if !is_custom_id(id) {
            return Err("Only custom summary types can be edited".into());
        }
    }
    let clash = taken_names
        .iter()
        .any(|(other_id, other)| Some(other_id.as_str()) != id && other.trim().eq_ignore_ascii_case(name));
    if clash {
        return Err(format!("A summary type named '{name}' already exists"));
    }
    let id = match id {
        Some(id) => id.to_string(),
        None => format!("{CUSTOM_PREFIX}{}-{}", slug(name), &uuid::Uuid::new_v4().simple().to_string()[..6]),
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create templates folder: {e}"))?;
    let json = custom_template_json(name, prompt);
    validate_and_parse_template(&json)?;
    let tmp = dir.join(format!("{id}.json.tmp"));
    std::fs::write(&tmp, json).map_err(|e| format!("Cannot save summary type: {e}"))?;
    std::fs::rename(&tmp, dir.join(format!("{id}.json"))).map_err(|e| format!("Cannot save summary type: {e}"))?;
    Ok(id)
}

fn delete_custom_in(dir: &std::path::Path, id: &str) -> Result<(), String> {
    if !is_custom_id(id) {
        return Err("Only custom summary types can be deleted".into());
    }
    match std::fs::remove_file(dir.join(format!("{id}.json"))) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Cannot delete summary type: {e}")),
    }
}

/// Create (`id = None`) or update a custom summary type; returns its id.
pub fn save_custom_template(id: Option<&str>, name: &str, prompt: &str) -> Result<String, String> {
    let dir = get_custom_templates_dir().ok_or("No application data folder available")?;
    let taken: Vec<(String, String)> = list_templates().into_iter().map(|(i, n, _)| (i, n)).collect();
    save_custom_in(&dir, &taken, id, name, prompt)
}

pub fn delete_custom_template(id: &str) -> Result<(), String> {
    let dir = get_custom_templates_dir().ok_or("No application data folder available")?;
    delete_custom_in(&dir, id)
}

/// The prompt of a custom summary type, for the edit form.
pub fn custom_template_prompt(id: &str) -> Option<String> {
    if !is_custom_id(id) {
        return None;
    }
    get_template(id).ok().and_then(|t| t.sections.into_iter().next().map(|s| s.instruction))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_template() {
        let template = get_template("daily_standup");
        assert!(template.is_ok());

        let template = template.unwrap();
        assert_eq!(template.name, "Daily Standup");
        assert!(!template.sections.is_empty());
    }

    #[test]
    fn test_get_nonexistent_template() {
        let result = get_template("nonexistent_template");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_template_ids() {
        let ids = list_template_ids();
        assert!(ids.contains(&"daily_standup".to_string()));
        assert!(ids.contains(&"standard_meeting".to_string()));
    }

    #[test]
    fn test_validate_invalid_json() {
        let result = validate_and_parse_template("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn custom_types_create_edit_delete_and_never_touch_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let taken = vec![("standard_meeting".to_string(), "Standard Meeting Notes".to_string())];

        let id = save_custom_in(dir.path(), &taken, None, "  Risk review ", "List risks and owners").unwrap();
        assert!(is_custom_id(&id));
        let t: Template = serde_json::from_str(&std::fs::read_to_string(dir.path().join(format!("{id}.json"))).unwrap()).unwrap();
        assert_eq!(t.name, "Risk review");
        assert_eq!(t.sections[0].instruction, "List risks and owners");

        // edit keeps the id
        let same = save_custom_in(dir.path(), &taken, Some(&id), "Risk review", "New prompt").unwrap();
        assert_eq!(same, id);
        assert!(std::fs::read_to_string(dir.path().join(format!("{id}.json"))).unwrap().contains("New prompt"));

        // names are unique (any case) against built-ins and other custom types
        assert!(save_custom_in(dir.path(), &taken, None, "standard meeting notes", "x").unwrap_err().contains("already exists"));
        let other_taken = vec![(id.clone(), "Risk review".to_string())];
        assert!(save_custom_in(dir.path(), &other_taken, None, "RISK REVIEW", "x").is_err());
        // renaming a type to its own name is fine
        assert!(save_custom_in(dir.path(), &other_taken, Some(&id), "Risk review", "p").is_ok());

        // validation
        assert!(save_custom_in(dir.path(), &taken, None, "  ", "p").is_err());
        assert!(save_custom_in(dir.path(), &taken, None, "n", "  ").is_err());
        assert!(save_custom_in(dir.path(), &taken, None, "n", &"x".repeat(4001)).is_err());

        // built-ins cannot be edited or deleted; path tricks are rejected
        assert!(save_custom_in(dir.path(), &taken, Some("standard_meeting"), "n", "p").is_err());
        assert!(delete_custom_in(dir.path(), "standard_meeting").is_err());
        assert!(delete_custom_in(dir.path(), "custom_../../etc/passwd").is_err());

        delete_custom_in(dir.path(), &id).unwrap();
        assert!(!dir.path().join(format!("{id}.json")).exists());
        delete_custom_in(dir.path(), &id).unwrap(); // idempotent
    }
}

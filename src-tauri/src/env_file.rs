//! Minimal `.env` reader for development.
//! It never expands `$`: CurseForge keys look like `$2a$10$...` and variable expansion would truncate them.

use std::fs;

/// Looks for `name` in a `.env` file in the working directory or any parent.
pub fn read_value(name: &str) -> Option<String> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if let Ok(text) = fs::read_to_string(dir.join(".env"))
            && let Some(value) = find_value(&text, name)
        {
            return Some(value);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn find_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, value) = line
            .strip_prefix("export ")
            .unwrap_or(line)
            .split_once('=')?;
        (key.trim() == name).then(|| unquote(value.trim()))
    })
}

fn unquote(value: &str) -> String {
    for quote in ['\'', '"'] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner.to_owned();
        }
    }
    value
        .split(" #")
        .next()
        .unwrap_or(value)
        .trim_end()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::find_value;

    #[test]
    fn keeps_dollar_signs_untouched() {
        let text = "OTHER=1\nCURSEFORGE_API_KEY=$2a$10$abc.DEF\n";
        assert_eq!(
            find_value(text, "CURSEFORGE_API_KEY").as_deref(),
            Some("$2a$10$abc.DEF")
        );
    }

    #[test]
    fn strips_quotes_export_and_comments() {
        assert_eq!(find_value("K='$2a$x'", "K").as_deref(), Some("$2a$x"));
        assert_eq!(find_value("export K=\"v1\"", "K").as_deref(), Some("v1"));
        assert_eq!(find_value("# K=no\nK=v # note", "K").as_deref(), Some("v"));
        assert_eq!(find_value("OTHER=1", "K"), None);
    }
}

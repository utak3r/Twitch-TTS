use regex::Regex;
use std::collections::HashMap;

pub struct AliasFilter {
    aliases: HashMap<String, String>,
}

impl AliasFilter {
    pub fn new(aliases: HashMap<String, String>) -> Self {
        Self { aliases }
    }

    pub fn apply(&self, input: &str) -> String {
        if self.aliases.is_empty() || input.is_empty() {
            return input.to_string();
        }

        let mut result = input.to_string();
        for (target, replacement) in &self.aliases {
            if target.is_empty() {
                continue;
            }
            // Case-insensitive word-boundary or direct replacement
            let pattern = format!(r"(?i)\b{}\b", regex::escape(target));
            if let Ok(re) = Regex::new(&pattern) {
                result = re.replace_all(&result, replacement.as_str()).to_string();
            } else {
                result = result.replace(target, replacement);
            }
        }
        result
    }
}

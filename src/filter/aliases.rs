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
            let left_boundary = if target.chars().next().map_or(false, is_word_char) {
                r"\b"
            } else {
                ""
            };
            let right_boundary = if target.chars().next_back().map_or(false, is_word_char) {
                r"\b"
            } else {
                ""
            };

            // Case-insensitive word-boundary or direct replacement
            let pattern = format!(r"(?i){}{}{}", left_boundary, regex::escape(target), right_boundary);
            if let Ok(re) = Regex::new(&pattern) {
                result = re.replace_all(&result, regex::NoExpand(replacement.as_str())).to_string();
            } else {
                result = result.replace(target, replacement);
            }
        }
        result
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

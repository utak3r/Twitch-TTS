use regex::Regex;
use std::collections::HashMap;

pub struct AliasFilter {
    compiled: Vec<(Regex, String)>,
}

impl AliasFilter {
    pub fn new(aliases: HashMap<String, String>) -> Self {
        let mut compiled = Vec::with_capacity(aliases.len());
        for (target, replacement) in aliases {
            if target.is_empty() {
                continue;
            }
            let left_boundary = if target.chars().next().map_or(false, is_word_char) {
                r"@?\b"
            } else {
                ""
            };
            let right_boundary = if target.chars().next_back().map_or(false, is_word_char) {
                r"\b"
            } else {
                ""
            };

            let pattern = format!(r"(?i){}{}{}", left_boundary, regex::escape(&target), right_boundary);
            if let Ok(re) = Regex::new(&pattern) {
                compiled.push((re, replacement));
            }
        }
        Self { compiled }
    }

    pub fn apply(&self, input: &str) -> String {
        if self.compiled.is_empty() || input.is_empty() {
            return input.to_string();
        }

        let mut result = input.to_string();
        for (re, replacement) in &self.compiled {
            result = re.replace_all(&result, regex::NoExpand(replacement.as_str())).to_string();
        }
        result
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

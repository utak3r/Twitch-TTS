use regex::Regex;
use std::borrow::Cow;
use std::fs;
use std::path::Path;
use tracing::warn;

pub struct ProfanityFilter {
    regex: Option<Regex>,
}

impl ProfanityFilter {
    pub fn new(words_file: &str) -> Self {
        let words = Self::load_words(words_file);
        Self::from_words(&words)
    }

    pub fn from_words(words: &[String]) -> Self {
        let mut patterns = Vec::new();
        for word in words {
            let trimmed = word.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let left_boundary = if trimmed.chars().next().map_or(false, is_word_char) {
                r"\b"
            } else {
                ""
            };
            let right_boundary = if trimmed.chars().next_back().map_or(false, is_word_char) {
                r"\b"
            } else {
                ""
            };
            patterns.push(format!(
                "(?:{}{}{})",
                left_boundary,
                regex::escape(trimmed),
                right_boundary
            ));
        }

        let regex = if patterns.is_empty() {
            None
        } else {
            let combined = format!(r"(?i)(?:{})", patterns.join("|"));
            match Regex::new(&combined) {
                Ok(re) => Some(re),
                Err(err) => {
                    warn!("Failed to compile profanity regex: {}", err);
                    None
                }
            }
        };

        Self { regex }
    }

    pub fn load_words(file_path: &str) -> Vec<String> {
        if !Path::new(file_path).exists() {
            return Vec::new();
        }

        match fs::read_to_string(file_path) {
            Ok(content) => content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect(),
            Err(err) => {
                warn!("Failed to read profanity words file {}: {}", file_path, err);
                Vec::new()
            }
        }
    }

    pub fn censor(&self, input: &str) -> (String, bool) {
        if input.is_empty() {
            return (input.to_string(), false);
        }

        let Some(ref re) = self.regex else {
            return (input.to_string(), false);
        };

        match re.replace_all(input, "piiiiiip") {
            Cow::Borrowed(_) => (input.to_string(), false),
            Cow::Owned(s) => (s, true),
        }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}


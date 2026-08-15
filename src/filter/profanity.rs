use regex::Regex;
use std::fs;
use std::path::Path;
use tracing::warn;

pub struct ProfanityFilter {
    regexes: Vec<Regex>,
}

impl ProfanityFilter {
    pub fn new(words_file: &str) -> Self {
        let words = Self::load_words(words_file);
        Self::from_words(&words)
    }

    pub fn from_words(words: &[String]) -> Self {
        let mut regexes = Vec::new();
        for word in words {
            let trimmed = word.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let pattern = format!(r"(?i)\b{}\b", regex::escape(trimmed));
            if let Ok(re) = Regex::new(&pattern) {
                regexes.push(re);
            }
        }
        Self { regexes }
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
        if self.regexes.is_empty() || input.is_empty() {
            return (input.to_string(), false);
        }

        let mut result = input.to_string();
        let mut censored = false;

        for re in &self.regexes {
            if re.is_match(&result) {
                censored = true;
                result = re.replace_all(&result, "piiiiiip").to_string();
            }
        }

        (result, censored)
    }
}

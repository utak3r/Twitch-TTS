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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_filter_and_empty_input() {
        let filter = AliasFilter::new(HashMap::new());
        assert_eq!(filter.apply(""), "");
        assert_eq!(filter.apply("Hello world"), "Hello world");

        let mut aliases = HashMap::new();
        aliases.insert("cat".to_string(), "dog".to_string());
        let populated_filter = AliasFilter::new(aliases);
        assert_eq!(populated_filter.apply(""), "");
    }

    #[test]
    fn test_bad_input_keys_and_values() {
        let mut aliases = HashMap::new();
        // Empty key should be ignored
        aliases.insert("".to_string(), "ignored".to_string());
        // Empty replacement should remove the target word
        aliases.insert("removeme".to_string(), "".to_string());
        // Whitespace phrase
        aliases.insert("see you later".to_string(), "cya".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("Hello removeme world"), "Hello  world");
        assert_eq!(filter.apply("Well, see you later!"), "Well, cya!");
    }

    #[test]
    fn test_special_regex_characters_in_target() {
        let mut aliases = HashMap::new();
        aliases.insert("[admin]".to_string(), "moderator".to_string());
        aliases.insert("(test)".to_string(), "exam".to_string());
        aliases.insert("$100".to_string(), "hundred bucks".to_string());
        aliases.insert("a*b+c?".to_string(), "math".to_string());
        aliases.insert("user.name".to_string(), "username".to_string());
        aliases.insert("foo|bar".to_string(), "foobar".to_string());
        aliases.insert("^start".to_string(), "beginning".to_string());
        aliases.insert("end$".to_string(), "finish".to_string());
        aliases.insert(r"\backslash".to_string(), "slash".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("Call [admin] now"), "Call moderator now");
        assert_eq!(filter.apply("Take the (test) today"), "Take the exam today");
        assert_eq!(filter.apply("Cost is $100 total"), "Cost is hundred bucks total");
        assert_eq!(filter.apply("Expression a*b+c? here"), "Expression math here");
        assert_eq!(filter.apply("Check user.name please"), "Check username please");
        assert_eq!(filter.apply("Select foo|bar option"), "Select foobar option");
        assert_eq!(filter.apply("At the ^start here"), "At the beginning here");
        assert_eq!(filter.apply("Reach the end$ here"), "Reach the finish here");
        assert_eq!(filter.apply(r"Found \backslash character"), "Found slash character");
    }

    #[test]
    fn test_regex_special_characters_in_replacement() {
        let mut aliases = HashMap::new();
        // Ensure replacement containing $, $1, \n, ${name} are treated as literal text
        aliases.insert("price".to_string(), "$100".to_string());
        aliases.insert("group".to_string(), "$1 $2 ${foo}".to_string());
        aliases.insert("escape".to_string(), r"\n \t \r".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("The price is right"), "The $100 is right");
        assert_eq!(filter.apply("Test group value"), "Test $1 $2 ${foo} value");
        assert_eq!(filter.apply("Show escape chars"), r"Show \n \t \r chars");
    }

    #[test]
    fn test_word_boundaries_and_substrings() {
        let mut aliases = HashMap::new();
        aliases.insert("cat".to_string(), "dog".to_string());
        aliases.insert("bot".to_string(), "human".to_string());

        let filter = AliasFilter::new(aliases);
        // Direct matches with word boundary and optional @
        assert_eq!(filter.apply("cat"), "dog");
        assert_eq!(filter.apply("@cat"), "dog");
        assert_eq!(filter.apply("A cat in the room"), "A dog in the room");
        assert_eq!(filter.apply("Hey @bot!"), "Hey human!");

        // Substrings inside words should NOT match
        assert_eq!(filter.apply("concatenate"), "concatenate");
        assert_eq!(filter.apply("bobcat"), "bobcat");
        assert_eq!(filter.apply("robotics"), "robotics");
        assert_eq!(filter.apply("bottom"), "bottom");
    }

    #[test]
    fn test_symbols_and_emoticons() {
        let mut aliases = HashMap::new();
        aliases.insert(":smile:".to_string(), "smiling".to_string());
        aliases.insert("<3".to_string(), "heart".to_string());
        aliases.insert("c++".to_string(), "cpp".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("Send :smile: here"), "Send smiling here");
        assert_eq!(filter.apply("Love you <3"), "Love you heart");
        assert_eq!(filter.apply("I code in c++ daily"), "I code in cpp daily");
        assert_eq!(filter.apply("@c++ is awesome"), "cpp is awesome");
        // 'c' has word boundary on left, so 'abc++' does not match 'c++'
        assert_eq!(filter.apply("abc++def"), "abc++def");
    }

    #[test]
    fn test_case_insensitivity_and_unicode() {
        let mut aliases = HashMap::new();
        aliases.insert("streamer".to_string(), "host".to_string());
        aliases.insert("żółć".to_string(), "zolc".to_string());
        aliases.insert("привет".to_string(), "czesc".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("STREAMER is live"), "host is live");
        assert_eq!(filter.apply("StReAmEr is live"), "host is live");
        assert_eq!(filter.apply("@STREAMER is live"), "host is live");
        assert_eq!(filter.apply("Zażółć żółć"), "Zażółć zolc");
        assert_eq!(filter.apply("ŻÓŁĆ"), "zolc");
        assert_eq!(filter.apply("ПРИВЕТ всем"), "czesc всем");
    }

    #[test]
    fn test_multiple_occurrences() {
        let mut aliases = HashMap::new();
        aliases.insert("a".to_string(), "1".to_string());
        aliases.insert("b".to_string(), "2".to_string());

        let filter = AliasFilter::new(aliases);
        assert_eq!(filter.apply("a a a"), "1 1 1");
        assert_eq!(filter.apply("a and b and a"), "1 and 2 and 1");
    }
}

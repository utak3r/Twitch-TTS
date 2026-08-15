use regex::Regex;
use std::sync::LazyLock;

static URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:https?://|www\.)\S+\b").unwrap()
});

static COMMON_EMOTES: &[&str] = &[
    // Global & Popular Twitch / BTTV / 7TV / FFZ Emotes
    "Kappa", "KappaPride", "KappaRoss", "Keepo", "PogChamp", "Pog", "PogBones", "POGGERS",
    "LUL", "LULW", "KEKW", "KEKWait", "OMEGALUL", "monkaS", "monkaW", "monkaOMEGA", "monkaEyes", "monkaHmm",
    "Pepega", "PepeHands", "PepeLaugh", "pepeJAM", "pepeL", "pepeW", "pepeBASS", "HYPERS",
    "FeelsGoodMan", "FeelsBadMan", "FeelsStrongMan", "FeelsWeirdMan", "FeelsOkayMan",
    "BibleThump", "ResidentSleeper", "Kreygasm", "HeyGuys", "TriHard", "NotLikeThis", "CoolCat",
    "WutFace", "AYAYA", "Sadge", "Smoge", "5Head", "EZ", "Clap", "widepeepoHappy", "widepeepoSad", "widepeepoEvil",
    "catJAM", "pepeJAMJAM", "PauseChamp", "gigaChad", "GIGACHAD", "NODDERS", "NOPERS", "COCKA", "HUH",
    "Aware", "Clueless", "DESPAIR", "modCheck", "banger", "copium", "hopium", "pepeMeltdown", "VoHiYo",
    "DankG", "DANKIES", "Prayge", "Susge", "WICKED", "YEP", "NOP",
    "BabyRage", "BatChest", "BlessRNG", "BloodTrail", "CoolStoryBob", "DansGame", "DatSheffy",
    "DogFace", "FrankerZ", "GivePLZ", "TakeNRG", "Jebaited", "KAPOW", "KonCha", "Mau5",
    "mrdestructoid", "MrDestructoid", "MVGame", "NinjaGrumpy", "NomNom", "PanicVis", "PraiseIt",
    "RitzMitz", "RlyTho", "RuleFive", "SeemsGood", "Shush", "SipSlick", "SMOrc", "SSSsss",
    "StinkyCheese", "SwiftRage", "TF2John", "ThinkingCap", "ThunBeast", "TooSpicy", "UnSane",
    "UncleNox", "VirtualHug", "YouDontSay", "bleedPurple", "duDuDu", "riPepperonis", "TwitchUnity",
    // Emoticons & Text Chat Emotes
    ":D", ":-D", ":)", ":-)", ";)", ";-)", ":(", ":-(", ";(", ";-(",
    ":P", ":-P", ":p", ":-p", ";P", ";-P", ";p", ";-p",
    ":O", ":-O", ":o", ":-o", ":3", "<3", "</3", "o_O", "O_o", "O_O", "o_o",
    "xD", "XD", "Xd", "xd", "xDD", "XDD", "XDDD", "xDDD", "Dx", "DX",
    "rawr", "uwu", "OwO", "UWU", "OWO", "UwU"
];

pub fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' | // Emoticons
        '\u{1F300}'..='\u{1F5FF}' | // Misc Symbols and Pictographs
        '\u{1F680}'..='\u{1F6FF}' | // Transport and Map
        '\u{1F700}'..='\u{1F77F}' | // Alchemical Symbols
        '\u{1F780}'..='\u{1F7FF}' | // Geometric Shapes Extended
        '\u{1F800}'..='\u{1F8FF}' | // Supplemental Arrows-C
        '\u{1F900}'..='\u{1F9FF}' | // Supplemental Symbols and Pictographs
        '\u{1FA00}'..='\u{1FA6F}' | // Chess Symbols
        '\u{1FA70}'..='\u{1FAFF}' | // Symbols and Pictographs Extended-A
        '\u{2600}'..='\u{26FF}'   | // Misc symbols
        '\u{2700}'..='\u{27BF}'   | // Dingbats
        '\u{FE00}'..='\u{FE0F}'   | // Variation Selectors
        '\u{1F1E6}'..='\u{1F1FF}' | // Regional indicator symbols (flags)
        '\u{200D}'                | // Zero-width joiner
        '\u{231A}'..='\u{231B}'   |
        '\u{23E9}'..='\u{23EC}'   |
        '\u{23F0}'                |
        '\u{23F3}'                |
        '\u{25FD}'..='\u{25FE}'   |
        '\u{2934}'..='\u{2935}'   |
        '\u{2B05}'..='\u{2B07}'   |
        '\u{2B1B}'..='\u{2B1C}'   |
        '\u{2B50}'                |
        '\u{2B55}'                |
        '\u{3030}'                |
        '\u{303D}'                |
        '\u{3297}'                |
        '\u{3299}'
    )
}

pub struct SpamFilter;

impl SpamFilter {
    pub fn remove_urls(input: &str) -> String {
        URL_REGEX.replace_all(input, "").to_string()
    }

    pub fn filter_emotes(input: &str) -> String {
        Self::filter_emotes_with_extra(input, &[])
    }

    pub fn filter_emotes_with_extra(input: &str, extra_emotes: &[String]) -> String {
        // 1. Remove all Unicode emojis
        let no_emojis: String = input.chars().filter(|&c| !is_emoji(c)).collect();

        // 2. Tokenize by whitespace and filter out emote tokens
        let words: Vec<&str> = no_emojis.split_whitespace().collect();
        let mut filtered_words = Vec::new();

        for word in words {
            if !Self::is_emote_token(word, extra_emotes) {
                filtered_words.push(word);
            }
        }

        filtered_words.join(" ")
    }

    fn is_emote_token(word: &str, extra_emotes: &[String]) -> bool {
        // Direct match (case-insensitive) for emoticons and word emotes
        for &emote in COMMON_EMOTES {
            if word.eq_ignore_ascii_case(emote) {
                return true;
            }
        }

        for extra in extra_emotes {
            if word.eq_ignore_ascii_case(extra) {
                return true;
            }
        }

        // Match with punctuation stripped (e.g. "Kappa!" -> "Kappa", "streamerHype," -> "streamerHype")
        let trimmed = word.trim_matches(|c: char| c.is_ascii_punctuation() && c != ':' && c != ';' && c != '<' && c != '3');
        if !trimmed.is_empty() && trimmed != word {
            for &emote in COMMON_EMOTES {
                if trimmed.eq_ignore_ascii_case(emote) {
                    return true;
                }
            }
            for extra in extra_emotes {
                if trimmed.eq_ignore_ascii_case(extra) {
                    return true;
                }
            }
        }

        false
    }

    pub fn reduce_repeated_chars(input: &str, max_repeat: usize) -> String {
        if max_repeat == 0 || input.is_empty() {
            return input.to_string();
        }

        let mut result = String::with_capacity(input.len());
        let mut prev_char = None;
        let mut repeat_count = 0;

        for ch in input.chars() {
            if Some(ch) == prev_char {
                repeat_count += 1;
                if repeat_count <= max_repeat {
                    result.push(ch);
                }
            } else {
                prev_char = Some(ch);
                repeat_count = 1;
                result.push(ch);
            }
        }

        result
    }

    pub fn reduce_consecutive_words(input: &str, max_consecutive: usize) -> String {
        let words: Vec<&str> = input.split_whitespace().collect();
        if words.is_empty() {
            return String::new();
        }

        let mut result = Vec::new();
        let mut prev_word: Option<&str> = None;
        let mut count = 0;

        for word in words {
            if let Some(prev) = prev_word {
                if word.eq_ignore_ascii_case(prev) {
                    count += 1;
                    if count <= max_consecutive {
                        result.push(word);
                    }
                } else {
                    prev_word = Some(word);
                    count = 1;
                    result.push(word);
                }
            } else {
                prev_word = Some(word);
                count = 1;
                result.push(word);
            }
        }

        result.join(" ")
    }

    pub fn truncate_with_ellipsis(input: &str, max_chars: usize) -> String {
        let char_count = input.chars().count();
        if char_count <= max_chars {
            return input.to_string();
        }

        let truncated: String = input.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated.trim_end())
    }
}

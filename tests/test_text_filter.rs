use std::collections::HashMap;
use twitch_tts::config::FiltersConfig;
use twitch_tts::domain::models::{FilterResult, MessageStatus};
use twitch_tts::filter::aliases::AliasFilter;
use twitch_tts::filter::profanity::ProfanityFilter;
use twitch_tts::filter::spam::SpamFilter;
use twitch_tts::filter::TextFilter;

#[test]
fn test_alias_replacement() {
    let mut aliases = HashMap::new();
    aliases.insert("utak3r".to_string(), "utaker".to_string());
    aliases.insert("Dr3gu".to_string(), "Dregu".to_string());
    aliases.insert("@streamer".to_string(), "Piotr".to_string());
    aliases.insert(":smile:".to_string(), "uśmiech".to_string());
    aliases.insert("c++".to_string(), "cpp".to_string());

    let filter = AliasFilter::new(aliases);
    assert_eq!(filter.apply("Hej utak3r! Co tam?"), "Hej utaker! Co tam?");
    assert_eq!(filter.apply("UTAK3R jest super"), "utaker jest super");
    assert_eq!(filter.apply("Dr3gu pozdrawia"), "Dregu pozdrawia");
    assert_eq!(filter.apply("Pozdrawiam @streamer"), "Pozdrawiam Piotr");
    assert_eq!(filter.apply("Wysyłam :smile: dla was"), "Wysyłam uśmiech dla was");
    assert_eq!(filter.apply("Lubię c++ bardzo"), "Lubię cpp bardzo");
    assert_eq!(filter.apply("abc++def"), "abc++def"); // 'c' has word boundary on left, so inside 'abc' it shouldn't match
}

#[test]
fn test_profanity_censorship() {
    let words = vec!["kurwa".to_string(), "chuj".to_string(), "fuck".to_string()];
    let filter = ProfanityFilter::from_words(&words);

    let (res1, censored1) = filter.censor("O kurwa, co za akcja!");
    assert!(censored1);
    assert_eq!(res1, "O piiiiiip, co za akcja!");

    let (res2, censored2) = filter.censor("Wszystko w porządku.");
    assert!(!censored2);
    assert_eq!(res2, "Wszystko w porządku.");
}

#[test]
fn test_spam_reduction() {
    // 1. Repeated chars
    let res = SpamFilter::reduce_repeated_chars("siemanko noooieeeee", 3);
    assert_eq!(res, "siemanko noooieee");

    // 2. Duplicate consecutive words
    let res_words = SpamFilter::reduce_consecutive_words("elo elo elo elo ziom", 2);
    assert_eq!(res_words, "elo elo ziom");

    // 3. URLs
    let res_url = SpamFilter::remove_urls("Sprawdź https://twitch.tv/utak3r lub www.google.com teraz");
    assert_eq!(res_url, "Sprawdź  lub  teraz");

    // 4. Truncation
    let res_trunc = SpamFilter::truncate_with_ellipsis("Długa wiadomość powyżej limitu", 15);
    assert_eq!(res_trunc, "Długa wiadom...");
}

#[test]
fn test_full_filter_pipeline() {
    let mut config = FiltersConfig::default();
    config.announce_username = true;
    config.username_template = "{nick} mówi: {message}".to_string();
    config.enable_profanity_filter = true;
    config.filter_emotes = true;
    config.max_characters = 100;
    config.max_repeated_chars = 3;
    config.ignore_users = vec!["Nightbot".to_string()];

    let mut filter = TextFilter::new(config);
    filter.update_profanity_words(&["kurwa".to_string(), "fuck".to_string()]);

    // 1. Ignored Bot
    let bot_res = filter.process("Nightbot", "Koniec konkursu!", false);
    match bot_res {
        FilterResult::Ignored(item) => {
            assert_eq!(item.status, MessageStatus::IgnoredBot);
        }
        _ => panic!("Expected Ignored result for blacklisted bot"),
    }

    // 2. Regular message with alias and profanity
    let msg_res = filter.process("utak3r", "Siemaaaa! O kurwa Kappa!", false);
    match msg_res {
        FilterResult::Ready(item) => {
            assert_eq!(item.status, MessageStatus::FilteredProfanity);
            assert!(item.spoken_text.contains("utaker"));
            assert!(item.spoken_text.contains("piiiiiip"));
            assert!(!item.spoken_text.contains("Kappa")); // Emote filtered
        }
        _ => panic!("Expected Ready result"),
    }
}

#[test]
fn test_emote_and_emoji_filtering() {
    // 1. Standard Twitch & 3rd party emotes
    let filtered_emotes = SpamFilter::filter_emotes("Siemanko Kappa, co tam? :D PogChamp KEKW");
    assert_eq!(filtered_emotes, "Siemanko co tam?");

    // 2. Emojis filtering
    let filtered_emojis = SpamFilter::filter_emotes("Super stream! 🔥🔥🔥 👍 ❤️");
    assert_eq!(filtered_emojis, "Super stream!");

    // 3. Emotes with custom extra Twitch fragments
    let extra = vec!["streamerSubEmote".to_string(), "channelLove".to_string()];
    let filtered_custom = SpamFilter::filter_emotes_with_extra(
        "Dzięki za suba streamerSubEmote! channelLove",
        &extra,
    );
    assert_eq!(filtered_custom, "Dzięki za suba");

    // 4. Message only containing emotes is filtered
    let mut config = FiltersConfig::default();
    config.filter_emotes = true;
    let filter = TextFilter::new(config);

    let res = filter.process("Viewer", "Kappa PogChamp 🔥 :D", false);
    match res {
        FilterResult::Filtered(item) => {
            assert_eq!(item.status, MessageStatus::FilteredEmote);
            assert_eq!(item.spoken_text, "[Filtered Emotes/Empty]");
        }
        _ => panic!("Expected Filtered result for emote-only message"),
    }

    // 5. When filter_emotes is false, emotes remain intact
    let mut config_disabled = FiltersConfig::default();
    config_disabled.filter_emotes = false;
    config_disabled.announce_username = false;
    let filter_disabled = TextFilter::new(config_disabled);

    let res_disabled = filter_disabled.process("Viewer", "Kappa PogChamp", false);
    match res_disabled {
        FilterResult::Ready(item) => {
            assert_eq!(item.spoken_text, "Kappa PogChamp");
        }
        _ => panic!("Expected Ready result when emote filter is disabled"),
    }
}

#[test]
fn test_inspect_stages_with_emotes() {
    let mut config = FiltersConfig::default();
    config.filter_emotes = true;
    let filter = TextFilter::new(config);

    let (aliased, _, _) = filter.inspect_stages("Cześć Kappa https://twitch.tv/test");
    assert_eq!(aliased, "Cześć");
}

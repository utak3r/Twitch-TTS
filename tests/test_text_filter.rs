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
    assert_eq!(filter.apply("Hej @utak3r! Co tam?"), "Hej utaker! Co tam?");
    assert_eq!(filter.apply("UTAK3R jest super"), "utaker jest super");
    assert_eq!(filter.apply("@UTAK3R jest super"), "utaker jest super");
    assert_eq!(filter.apply("Dr3gu pozdrawia"), "Dregu pozdrawia");
    assert_eq!(filter.apply("Pozdrawiam @Dr3gu"), "Pozdrawiam Dregu");
    assert_eq!(filter.apply("Pozdrawiam @streamer"), "Pozdrawiam Piotr");
    assert_eq!(filter.apply("Wysyłam :smile: dla was"), "Wysyłam uśmiech dla was");
    assert_eq!(filter.apply("Lubię c++ bardzo"), "Lubię cpp bardzo");
    assert_eq!(filter.apply("abc++def"), "abc++def"); // 'c' has word boundary on left, so inside 'abc' it shouldn't match
}

#[test]
fn test_profanity_censorship() {
    let words = vec![
        "kurwa".to_string(),
        "chuj".to_string(),
        "fuck".to_string(),
        "# comment".to_string(),
        "".to_string(),
    ];
    let filter = ProfanityFilter::from_words(&words);

    let (res1, censored1) = filter.censor("O kurwa, co za akcja!");
    assert!(censored1);
    assert_eq!(res1, "O piiiiiip, co za akcja!");

    let (res2, censored2) = filter.censor("Wszystko w porządku.");
    assert!(!censored2);
    assert_eq!(res2, "Wszystko w porządku.");

    // Multiple profanities in single message
    let (res3, censored3) = filter.censor("O kurwa, fuck that chuj!");
    assert!(censored3);
    assert_eq!(res3, "O piiiiiip, piiiiiip that piiiiiip!");
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

    // 5. Mention prefixes stripping
    assert_eq!(SpamFilter::remove_mention_prefixes("@unknownusername"), "unknownusername");
    assert_eq!(SpamFilter::remove_mention_prefixes("Hej @unknown_123, jak tam?"), "Hej unknown_123, jak tam?");
    assert_eq!(SpamFilter::remove_mention_prefixes("Kontakt: email@domain.com tutaj"), "Kontakt: email@domain.com tutaj");
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

    // 3. Message with unaliased mention and @ in raw_user
    let unaliased_res = filter.process("@Viewer_99", "Cześć @unknownusername co słychać?", false);
    match unaliased_res {
        FilterResult::Ready(item) => {
            assert_eq!(item.spoken_text, "Viewer_99 mówi: Cześć unknownusername co słychać?");
        }
        _ => panic!("Expected Ready result for unaliased mention"),
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

#[test]
fn test_command_filtering_basic() {
    // 1. Basic bot commands
    assert_eq!(SpamFilter::filter_commands("!mycommand"), "");
    assert_eq!(SpamFilter::filter_commands("!points"), "");
    assert_eq!(SpamFilter::filter_commands("!mycommand i jeszcze !points"), "i jeszcze");

    // 2. Command at different positions
    assert_eq!(SpamFilter::filter_commands("!sr gramy dalej"), "gramy dalej");
    assert_eq!(SpamFilter::filter_commands("gramy dalej !sr"), "gramy dalej");
    assert_eq!(SpamFilter::filter_commands("gramy !sr dalej"), "gramy dalej");

    // 3. Multiple consecutive commands
    assert_eq!(SpamFilter::filter_commands("!points !rank !uptime"), "");
    assert_eq!(SpamFilter::filter_commands("!cmd1 !cmd2 tekst !cmd3"), "tekst");

    // 4. Underscores, hyphens, numbers
    assert_eq!(SpamFilter::filter_commands("!drop_item !song-request !123 !a !A"), "");
    assert_eq!(SpamFilter::filter_commands("!giveaway_2026"), "");

    // 5. Unicode command names
    assert_eq!(SpamFilter::filter_commands("!żółw !cześć !привет"), "");
    assert_eq!(SpamFilter::filter_commands("sprawdź !żółw teraz"), "sprawdź teraz");
}

#[test]
fn test_command_filtering_punctuation_and_wrappers() {
    // 1. Commands with trailing punctuation
    assert_eq!(SpamFilter::filter_commands("Wpisz !points, aby sprawdzić stan."), "Wpisz aby sprawdzić stan.");
    assert_eq!(SpamFilter::filter_commands("Sprawdź !help!"), "Sprawdź");
    assert_eq!(SpamFilter::filter_commands("Czy to !komenda?"), "Czy to");
    assert_eq!(SpamFilter::filter_commands("Uruchom !run;"), "Uruchom");

    // 2. Commands wrapped in brackets or quotes
    assert_eq!(SpamFilter::filter_commands("Zobacz to (!points) teraz"), "Zobacz to teraz");
    assert_eq!(SpamFilter::filter_commands("Zobacz to [!points]"), "Zobacz to");
    assert_eq!(SpamFilter::filter_commands("Wpisz \"!komenda\" w czacie"), "Wpisz w czacie");
    assert_eq!(SpamFilter::filter_commands("Wpisz '!komenda' w czacie"), "Wpisz w czacie");
}

#[test]
fn test_command_filtering_non_commands_preserved() {
    // 1. Standalone exclamation marks and spaces
    assert_eq!(SpamFilter::filter_commands("!"), "!");
    assert_eq!(SpamFilter::filter_commands("Uwaga ! Sprawdź to"), "Uwaga ! Sprawdź to");
    assert_eq!(SpamFilter::filter_commands("! mycommand"), "! mycommand");

    // 2. Exclamation marks at end or inside words
    assert_eq!(SpamFilter::filter_commands("Cześć! Jak leci?"), "Cześć! Jak leci?");
    assert_eq!(SpamFilter::filter_commands("Niesamowite!! Super!!!"), "Niesamowite!! Super!!!");
    assert_eq!(SpamFilter::filter_commands("Hello!World"), "Hello!World");

    // 3. Punctuation combinations
    assert_eq!(SpamFilter::filter_commands("Co to jest?!"), "Co to jest?!");
    assert_eq!(SpamFilter::filter_commands("!!"), "!!");
    assert_eq!(SpamFilter::filter_commands("!??"), "!?\?");
    assert_eq!(SpamFilter::filter_commands("!..."), "!...");

    // 4. Comparison operators
    assert_eq!(SpamFilter::filter_commands("x != 5"), "x != 5");
}

#[test]
fn test_command_filtering_edge_cases_and_bad_input() {
    // 1. Empty strings and whitespace variations
    assert_eq!(SpamFilter::filter_commands(""), "");
    assert_eq!(SpamFilter::filter_commands("   "), "");
    assert_eq!(SpamFilter::filter_commands("\t\n\r"), "");
    assert_eq!(SpamFilter::filter_commands("  !cmd   !test   "), "");

    // 2. Control characters and special tokens
    assert_eq!(SpamFilter::filter_commands("! \0 !cmd \u{200B}"), "! \0 \u{200B}");

    // 3. Only exclamation marks / symbols
    assert_eq!(SpamFilter::filter_commands("! ! !"), "! ! !");
    assert_eq!(SpamFilter::filter_commands("!@#$"), "!@#$");
}

#[test]
fn test_full_pipeline_with_command_filtering() {
    let mut config = FiltersConfig::default();
    config.announce_username = false;
    config.filter_commands = true;
    let filter = TextFilter::new(config);

    // 1. Command filtered in message
    let res = filter.process("Viewer", "Siemanko !punkty co słychać?", false);
    match res {
        FilterResult::Ready(item) => {
            assert_eq!(item.spoken_text, "Siemanko co słychać?");
        }
        _ => panic!("Expected Ready result"),
    }

    // 2. Message consisting only of commands is filtered as empty
    let res_cmd_only = filter.process("Viewer", "!points !rank !uptime", false);
    match res_cmd_only {
        FilterResult::Filtered(item) => {
            assert_eq!(item.status, MessageStatus::FilteredEmote);
            assert_eq!(item.spoken_text, "[Filtered Emotes/Empty]");
        }
        _ => panic!("Expected Filtered result for command-only message"),
    }

    // 3. When filter_commands is disabled, commands are preserved
    let mut config_disabled = FiltersConfig::default();
    config_disabled.announce_username = false;
    config_disabled.filter_commands = false;
    let filter_disabled = TextFilter::new(config_disabled);

    let res_disabled = filter_disabled.process("Viewer", "Siemanko !punkty", false);
    match res_disabled {
        FilterResult::Ready(item) => {
            assert_eq!(item.spoken_text, "Siemanko !punkty");
        }
        _ => panic!("Expected Ready result when filter_commands is false"),
    }
}


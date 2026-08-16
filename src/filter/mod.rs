pub mod aliases;
pub mod profanity;
pub mod spam;

use crate::config::FiltersConfig;
use crate::domain::models::{FilterResult, MessageStatus, SpokenItem};
use aliases::AliasFilter;
use profanity::ProfanityFilter;
use spam::SpamFilter;

pub struct TextFilter {
    config: FiltersConfig,
    alias_filter: AliasFilter,
    profanity_filter: ProfanityFilter,
}

impl TextFilter {
    pub fn new(config: FiltersConfig) -> Self {
        let alias_filter = AliasFilter::new(config.username_aliases.clone());
        let profanity_filter = ProfanityFilter::new(&config.profanity_words_file);

        Self {
            config,
            alias_filter,
            profanity_filter,
        }
    }

    pub fn update_config(&mut self, config: FiltersConfig) {
        self.alias_filter = AliasFilter::new(config.username_aliases.clone());
        self.profanity_filter = ProfanityFilter::new(&config.profanity_words_file);
        self.config = config;
    }

    pub fn update_profanity_words(&mut self, words: &[String]) {
        self.profanity_filter = ProfanityFilter::from_words(words);
    }

    pub fn is_ignored(&self, user: &str) -> bool {
        self.config
            .ignore_users
            .iter()
            .any(|ignored| ignored.eq_ignore_ascii_case(user.trim()))
    }

    pub fn process(&self, raw_user: &str, raw_text: &str, is_custom_reward: bool) -> FilterResult {
        self.process_with_emotes(raw_user, raw_text, &[], is_custom_reward)
    }

    pub fn process_with_emotes(
        &self,
        raw_user: &str,
        raw_text: &str,
        extra_emotes: &[String],
        _is_custom_reward: bool,
    ) -> FilterResult {
        // 1. Blacklisted bot check
        if self.is_ignored(raw_user) {
            let item = SpokenItem::new(
                raw_user.to_string(),
                raw_text.to_string(),
                format!("[Ignored Bot {}] {}", raw_user, raw_text),
                MessageStatus::IgnoredBot,
            );
            return FilterResult::Ignored(item);
        }

        // 2. Remove URLs
        let no_urls = SpamFilter::remove_urls(raw_text);

        // 3. Emotes stripping
        let no_emotes = if self.config.filter_emotes {
            SpamFilter::filter_emotes_with_extra(&no_urls, extra_emotes)
        } else {
            no_urls
        };

        if no_emotes.trim().is_empty() {
            let item = SpokenItem::new(
                raw_user.to_string(),
                raw_text.to_string(),
                "[Filtered Emotes/Empty]".to_string(),
                MessageStatus::FilteredEmote,
            );
            return FilterResult::Filtered(item);
        }

        // 4. Apply phonetic aliases
        let clean_user = raw_user.trim_start_matches('@');
        let aliased_user = self.alias_filter.apply(clean_user);
        let aliased_text = self.alias_filter.apply(&no_emotes);
        let no_mentions = SpamFilter::remove_mention_prefixes(&aliased_text);

        // 5. Reduce repeated characters & duplicate words
        let reduced_chars = SpamFilter::reduce_repeated_chars(&no_mentions, self.config.max_repeated_chars);
        let unspammed = SpamFilter::reduce_consecutive_words(&reduced_chars, 2);

        // 6. Profanity censorship
        let (censored, was_profane) = if self.config.enable_profanity_filter {
            self.profanity_filter.censor(&unspammed)
        } else {
            (unspammed, false)
        };

        // 7. Truncate to max characters
        let truncated = SpamFilter::truncate_with_ellipsis(&censored, self.config.max_characters);

        // 8. Username announcement template
        let final_text = if self.config.announce_username {
            self.format_template(&aliased_user, &truncated)
        } else {
            truncated
        };

        let status = if was_profane {
            MessageStatus::FilteredProfanity
        } else {
            MessageStatus::Queued
        };

        let item = SpokenItem::new(
            raw_user.to_string(),
            raw_text.to_string(),
            final_text,
            status,
        );

        FilterResult::Ready(item)
    }

    pub fn inspect_stages(&self, input: &str) -> (String, String, String) {
        let no_urls = SpamFilter::remove_urls(input);
        let no_emotes = if self.config.filter_emotes {
            SpamFilter::filter_emotes(&no_urls)
        } else {
            no_urls
        };
        let aliased = self.alias_filter.apply(&no_emotes);
        let no_mentions = SpamFilter::remove_mention_prefixes(&aliased);
        let reduced = SpamFilter::reduce_repeated_chars(&no_mentions, self.config.max_repeated_chars);
        let unspammed = SpamFilter::reduce_consecutive_words(&reduced, 2);
        let (censored, _) = if self.config.enable_profanity_filter {
            self.profanity_filter.censor(&unspammed)
        } else {
            (unspammed, false)
        };
        let truncated = SpamFilter::truncate_with_ellipsis(&censored, self.config.max_characters);
        (aliased, censored, truncated)
    }

    fn format_template(&self, user: &str, message: &str) -> String {
        let tpl = &self.config.username_template;
        if tpl.contains("{nick}") || tpl.contains("{message}") {
            tpl.replace("{nick}", user).replace("{message}", message)
        } else {
            format!("{}: {}", user, message)
        }
    }
}

//! Tokenizers, filters and the analyzer pipeline.

use quarry_core::tokenize::ascii::{fold, needs_folding, AsciiFolding};
use quarry_core::tokenize::ngram::{EdgeNgramTokenizer, NgramTokenizer};
use quarry_core::tokenize::standard::{KeywordTokenizer, StandardTokenizer, WhitespaceTokenizer};
use quarry_core::tokenize::stopwords::{StopwordFilter, ENGLISH};
use quarry_core::tokenize::{Analyzer, LengthFilter, Lowercase, Token, TokenFilter, Tokenizer};

fn texts(tokens: &[Token]) -> Vec<&str> {
    tokens.iter().map(|t| t.text.as_str()).collect()
}

// ---- StandardTokenizer ----------------------------------------------------

#[test]
fn splits_a_sentence_on_spaces() {
    let tokens = StandardTokenizer::default().tokenize("the quick brown fox");
    assert_eq!(texts(&tokens), ["the", "quick", "brown", "fox"]);
}

#[test]
fn splits_on_punctuation() {
    let tokens = StandardTokenizer::default().tokenize("one,two;three!four");
    assert_eq!(texts(&tokens), ["one", "two", "three", "four"]);
}

#[test]
fn collapses_runs_of_separators() {
    let tokens = StandardTokenizer::default().tokenize("a   ,,,   b");
    assert_eq!(texts(&tokens), ["a", "b"]);
}

#[test]
fn returns_nothing_for_an_empty_input() {
    assert!(StandardTokenizer::default().tokenize("").is_empty());
}

#[test]
fn returns_nothing_for_only_separators() {
    assert!(StandardTokenizer::default().tokenize("  ,.;!  ").is_empty());
}

#[test]
fn keeps_an_internal_apostrophe() {
    // Splitting `wasn't` into `wasn` and `t` makes it unfindable.
    let tokens = StandardTokenizer::default().tokenize("it wasn't there");
    assert_eq!(texts(&tokens), ["it", "wasn't", "there"]);
}

#[test]
fn keeps_a_typographic_apostrophe() {
    let tokens = StandardTokenizer::default().tokenize("it wasn\u{2019}t there");
    assert_eq!(texts(&tokens), ["it", "wasn\u{2019}t", "there"]);
}

#[test]
fn keeps_an_internal_hyphen() {
    let tokens = StandardTokenizer::default().tokenize("a well-known e-mail");
    assert_eq!(texts(&tokens), ["a", "well-known", "e-mail"]);
}

#[test]
fn keeps_an_internal_period() {
    let tokens = StandardTokenizer::default().tokenize("version v1.2.3 shipped");
    assert_eq!(texts(&tokens), ["version", "v1.2.3", "shipped"]);
}

#[test]
fn strips_a_trailing_sentence_period() {
    // The period is punctuation here, not part of the word.
    let tokens = StandardTokenizer::default().tokenize("the end.");
    assert_eq!(texts(&tokens), ["the", "end"]);
}

#[test]
fn strips_a_leading_hyphen_from_a_list_item() {
    let tokens = StandardTokenizer::default().tokenize("- item");
    assert_eq!(texts(&tokens), ["item"]);
}

#[test]
fn strips_surrounding_quotes_but_keeps_the_word() {
    let tokens = StandardTokenizer::default().tokenize("he said 'hello'");
    assert_eq!(texts(&tokens), ["he", "said", "hello"]);
}

#[test]
fn drops_a_token_that_is_only_internal_characters() {
    let tokens = StandardTokenizer::default().tokenize("a --- b");
    assert_eq!(texts(&tokens), ["a", "b"]);
}

#[test]
fn keeps_digits_as_tokens() {
    let tokens = StandardTokenizer::default().tokenize("page 42 of 100");
    assert_eq!(texts(&tokens), ["page", "42", "of", "100"]);
}

#[test]
fn keeps_an_underscore_inside_a_token() {
    let tokens = StandardTokenizer::default().tokenize("snake_case_name");
    assert_eq!(texts(&tokens), ["snake_case_name"]);
}

#[test]
fn numbers_positions_from_zero() {
    let tokens = StandardTokenizer::default().tokenize("a b c");
    assert_eq!(
        tokens.iter().map(|t| t.position).collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn reports_byte_offsets_that_index_the_original_text() {
    let input = "the quick fox";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn reports_offsets_correctly_after_trimming_punctuation() {
    let input = "hello, 'world'.";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn reports_offsets_correctly_through_multibyte_text() {
    let input = "café naïve dog";
    for token in StandardTokenizer::default().tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn tokenizes_a_token_running_to_the_end_of_input() {
    let tokens = StandardTokenizer::default().tokenize("trailing");
    assert_eq!(texts(&tokens), ["trailing"]);
}

#[test]
fn keeps_non_latin_letters_as_tokens() {
    let tokens = StandardTokenizer::default().tokenize("Привет мир");
    assert_eq!(texts(&tokens), ["Привет", "мир"]);
}

#[test]
fn strict_mode_splits_on_every_non_alphanumeric() {
    let tokens = StandardTokenizer::strict().tokenize("wasn't well-known");
    assert_eq!(texts(&tokens), ["wasn", "t", "well", "known"]);
}

// ---- KeywordTokenizer ----------------------------------------------------

#[test]
fn keyword_tokenizer_emits_the_whole_input() {
    let tokens = KeywordTokenizer.tokenize("New York City");
    assert_eq!(texts(&tokens), ["New York City"]);
}

#[test]
fn keyword_tokenizer_trims_surrounding_whitespace() {
    let tokens = KeywordTokenizer.tokenize("  padded  ");
    assert_eq!(texts(&tokens), ["padded"]);
}

#[test]
fn keyword_tokenizer_returns_nothing_for_blank_input() {
    assert!(KeywordTokenizer.tokenize("   ").is_empty());
}

#[test]
fn keyword_tokenizer_offsets_point_at_the_trimmed_span() {
    let input = "  padded  ";
    let tokens = KeywordTokenizer.tokenize(input);
    assert_eq!(&input[tokens[0].start..tokens[0].end], "padded");
}

#[test]
fn keyword_tokenizer_keeps_punctuation() {
    let tokens = KeywordTokenizer.tokenize("SKU-1234/A");
    assert_eq!(texts(&tokens), ["SKU-1234/A"]);
}

// ---- WhitespaceTokenizer ------------------------------------------------

#[test]
fn whitespace_tokenizer_keeps_punctuation_attached() {
    let tokens = WhitespaceTokenizer.tokenize("hello, world!");
    assert_eq!(texts(&tokens), ["hello,", "world!"]);
}

#[test]
fn whitespace_tokenizer_handles_tabs_and_newlines() {
    let tokens = WhitespaceTokenizer.tokenize("a\tb\nc");
    assert_eq!(texts(&tokens), ["a", "b", "c"]);
}

#[test]
fn whitespace_tokenizer_reports_usable_offsets() {
    let input = "alpha beta";
    for token in WhitespaceTokenizer.tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

// ---- Lowercase ----------------------------------------------------------

#[test]
fn lowercase_filter_lowers_every_token() {
    let tokens = Lowercase.filter(vec![Token::new("Rust", 0, 0, 4)]);
    assert_eq!(tokens[0].text, "rust");
}

#[test]
fn lowercase_filter_preserves_positions_and_offsets() {
    let tokens = Lowercase.filter(vec![Token::new("ABC", 7, 10, 13)]);
    assert_eq!(
        (tokens[0].position, tokens[0].start, tokens[0].end),
        (7, 10, 13)
    );
}

#[test]
fn lowercase_filter_handles_non_ascii() {
    let tokens = Lowercase.filter(vec![Token::new("ÉCOLE", 0, 0, 7)]);
    assert_eq!(tokens[0].text, "école");
}

// ---- LengthFilter -------------------------------------------------------

#[test]
fn length_filter_drops_tokens_below_the_minimum() {
    let filter = LengthFilter { min: 3, max: 10 };
    let tokens = filter.filter(vec![Token::new("a", 0, 0, 1), Token::new("abc", 1, 2, 5)]);
    assert_eq!(texts(&tokens), ["abc"]);
}

#[test]
fn length_filter_drops_tokens_above_the_maximum() {
    let filter = LengthFilter { min: 1, max: 4 };
    let tokens = filter.filter(vec![Token::new("abcdefgh", 0, 0, 8)]);
    assert!(tokens.is_empty());
}

#[test]
fn length_filter_measures_characters_not_bytes() {
    // Four characters, eight bytes: a byte-based limit would wrongly drop it.
    let filter = LengthFilter { min: 1, max: 4 };
    let tokens = filter.filter(vec![Token::new("éééé", 0, 0, 8)]);
    assert_eq!(tokens.len(), 1);
}

#[test]
fn length_filter_default_keeps_ordinary_words() {
    let filter = LengthFilter::default();
    let tokens = filter.filter(vec![Token::new("ownership", 0, 0, 9)]);
    assert_eq!(tokens.len(), 1);
}

// ---- StopwordFilter -----------------------------------------------------

#[test]
fn stopword_filter_removes_listed_words() {
    let filter = StopwordFilter::english();
    let tokens = filter.filter(vec![
        Token::new("the", 0, 0, 3),
        Token::new("king", 1, 4, 8),
    ]);
    assert_eq!(texts(&tokens), ["king"]);
}

#[test]
fn stopword_filter_preserves_positions_so_phrases_stay_honest() {
    // Removing `the` must leave a gap, or "king of the hill" would match the
    // phrase "king of hill" exactly.
    let filter = StopwordFilter::english();
    let tokens = filter.filter(vec![
        Token::new("king", 0, 0, 4),
        Token::new("of", 1, 5, 7),
        Token::new("the", 2, 8, 11),
        Token::new("hill", 3, 12, 16),
    ]);

    assert_eq!(texts(&tokens), ["king", "hill"]);
    assert_eq!(tokens[1].position, 3);
}

#[test]
fn stopword_filter_is_case_insensitive_in_its_list() {
    let filter = StopwordFilter::new(["The", "AND"]);
    assert!(filter.contains("the"));
    assert!(filter.contains("and"));
}

#[test]
fn empty_stopword_filter_removes_nothing() {
    let filter = StopwordFilter::empty();
    let tokens = filter.filter(vec![Token::new("the", 0, 0, 3)]);
    assert_eq!(tokens.len(), 1);
}

#[test]
fn stopword_filter_reports_its_size() {
    assert_eq!(StopwordFilter::english().len(), ENGLISH.len());
    assert!(StopwordFilter::empty().is_empty());
}

#[test]
fn stopword_filter_accepts_an_extra_word() {
    let filter = StopwordFilter::empty().with_word("Quarry");
    assert!(filter.contains("quarry"));
}

#[test]
fn english_stopword_list_has_no_duplicates() {
    let mut sorted = ENGLISH.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(sorted.len(), before);
}

#[test]
fn english_stopword_list_is_all_lowercase() {
    assert!(ENGLISH.iter().all(|w| w.to_lowercase() == *w));
}

// ---- AsciiFolding -------------------------------------------------------

#[test]
fn folds_accented_vowels() {
    assert_eq!(fold("àáâãäå"), "aaaaaa");
    assert_eq!(fold("èéêë"), "eeee");
}

#[test]
fn folds_uppercase_accents_to_uppercase_ascii() {
    assert_eq!(fold("ÀÉÎÕÜ"), "AEIOU");
}

#[test]
fn folds_cafe_to_cafe() {
    assert_eq!(fold("café"), "cafe");
}

#[test]
fn folds_multi_character_ligatures() {
    assert_eq!(fold("æon"), "aeon");
    assert_eq!(fold("Œuvre"), "OEuvre");
    assert_eq!(fold("straße"), "strasse");
}

#[test]
fn folds_thorn_and_eth() {
    assert_eq!(fold("þ"), "th");
    assert_eq!(fold("ð"), "d");
}

#[test]
fn folds_slashed_o_and_barred_l() {
    assert_eq!(fold("ø"), "o");
    assert_eq!(fold("ł"), "l");
}

#[test]
fn folds_typographic_punctuation() {
    assert_eq!(fold("don\u{2019}t"), "don't");
    assert_eq!(fold("\u{201C}quoted\u{201D}"), "\"quoted\"");
    assert_eq!(fold("dash\u{2014}dash"), "dash-dash");
}

#[test]
fn leaves_plain_ascii_untouched() {
    assert_eq!(fold("plain ascii text"), "plain ascii text");
}

#[test]
fn leaves_unmapped_scripts_untouched() {
    // Mangling text it cannot fold would be worse than leaving it alone.
    assert_eq!(fold("日本語"), "日本語");
    assert_eq!(fold("Привет"), "Привет");
}

#[test]
fn needs_folding_detects_whether_work_is_required() {
    assert!(needs_folding("café"));
    assert!(!needs_folding("cafe"));
    assert!(!needs_folding("日本語"));
}

#[test]
fn folding_filter_skips_allocation_for_ascii_tokens() {
    let tokens = AsciiFolding.filter(vec![Token::new("ascii", 0, 0, 5)]);
    assert_eq!(tokens[0].text, "ascii");
}

#[test]
fn folding_filter_preserves_positions() {
    let tokens = AsciiFolding.filter(vec![Token::new("café", 3, 10, 15)]);
    assert_eq!((tokens[0].text.as_str(), tokens[0].position), ("cafe", 3));
}

// ---- NgramTokenizer -----------------------------------------------------

#[test]
fn ngram_tokenizer_emits_windows_in_the_configured_range() {
    let tokens = NgramTokenizer::new(2, 2).tokenize("abcd");
    assert_eq!(texts(&tokens), ["ab", "bc", "cd"]);
}

#[test]
fn ngram_tokenizer_emits_several_widths() {
    let tokens = NgramTokenizer::new(1, 2).tokenize("abc");
    assert_eq!(texts(&tokens), ["a", "ab", "b", "bc", "c"]);
}

#[test]
fn ngram_tokenizer_skips_windows_spanning_whitespace() {
    // A window straddling a word boundary matches nothing useful.
    let tokens = NgramTokenizer::new(2, 2).tokenize("ab cd");
    assert_eq!(texts(&tokens), ["ab", "cd"]);
}

#[test]
fn ngram_tokenizer_handles_input_shorter_than_the_window() {
    assert!(NgramTokenizer::new(5, 5).tokenize("abc").is_empty());
}

#[test]
fn ngram_tokenizer_handles_multibyte_characters() {
    let tokens = NgramTokenizer::new(2, 2).tokenize("日本語");
    assert_eq!(texts(&tokens), ["日本", "本語"]);
}

#[test]
fn ngram_tokenizer_reports_usable_offsets() {
    let input = "héllo";
    for token in NgramTokenizer::new(2, 3).tokenize(input) {
        assert_eq!(&input[token.start..token.end], token.text);
    }
}

#[test]
fn ngram_tokenizer_clamps_a_reversed_range() {
    // A reversed range should degrade, not produce nothing at all.
    let tokens = NgramTokenizer::new(4, 2).tokenize("abcde");
    assert!(!tokens.is_empty());
}

#[test]
fn ngram_tokenizer_clamps_a_zero_minimum() {
    let tokenizer = NgramTokenizer::new(0, 2);
    assert_eq!(tokenizer.min, 1);
}

#[test]
fn fixed_ngram_tokenizer_uses_one_width() {
    let tokenizer = NgramTokenizer::fixed(3);
    assert_eq!((tokenizer.min, tokenizer.max), (3, 3));
}

// ---- EdgeNgramTokenizer -------------------------------------------------

#[test]
fn edge_ngram_tokenizer_emits_prefixes() {
    let tokens = EdgeNgramTokenizer::new(1, 4).tokenize("rust");
    assert_eq!(texts(&tokens), ["r", "ru", "rus", "rust"]);
}

#[test]
fn edge_ngram_tokenizer_stops_at_the_word_length() {
    let tokens = EdgeNgramTokenizer::new(2, 10).tokenize("ab");
    assert_eq!(texts(&tokens), ["ab"]);
}

#[test]
fn edge_ngram_tokenizer_handles_several_words() {
    let tokens = EdgeNgramTokenizer::new(2, 3).tokenize("rust go");
    assert_eq!(texts(&tokens), ["ru", "rus", "go"]);
}

#[test]
fn edge_ngram_tokenizer_gives_one_words_prefixes_one_position() {
    // The prefixes are alternatives at the same slot, not a sequence.
    let tokens = EdgeNgramTokenizer::new(1, 3).tokenize("ab cd");
    let first: Vec<u32> = tokens
        .iter()
        .filter(|t| t.text.starts_with('a'))
        .map(|t| t.position)
        .collect();

    assert!(first.iter().all(|p| *p == 0));
}

#[test]
fn edge_ngram_tokenizer_skips_punctuation() {
    let tokens = EdgeNgramTokenizer::new(2, 3).tokenize("a-b");
    assert!(texts(&tokens).iter().all(|t| !t.contains('-')));
}

// ---- Analyzer -----------------------------------------------------------

#[test]
fn english_analyzer_lowercases_folds_and_stems() {
    let terms = Analyzer::english().terms("Running CAFÉS quickly");
    assert_eq!(terms, ["run", "cafe", "quickli"]);
}

#[test]
fn english_analyzer_drops_stopwords() {
    let terms = Analyzer::english().terms("the quick brown fox");
    assert!(!terms.contains(&"the".to_string()));
}

#[test]
fn english_analyzer_is_stable_across_index_and_query() {
    // The property the whole pipeline exists for: a query must produce the
    // same terms the document did, or the document is unfindable.
    let analyzer = Analyzer::english();
    assert_eq!(analyzer.terms("Running"), analyzer.terms("runs"));
}

#[test]
fn english_analyzer_handles_an_empty_string() {
    assert!(Analyzer::english().terms("").is_empty());
}

#[test]
fn english_analyzer_handles_only_stopwords() {
    assert!(Analyzer::english().terms("the and of to").is_empty());
}

#[test]
fn english_analyzer_folds_before_checking_stopwords() {
    // Order matters: without folding first, `thé` survives a list with `the`.
    assert!(Analyzer::english().terms("thé").is_empty());
}

#[test]
fn simple_analyzer_neither_stems_nor_drops_stopwords() {
    let terms = Analyzer::simple().terms("The Running Cafés");
    assert_eq!(terms, ["the", "running", "cafes"]);
}

#[test]
fn keyword_analyzer_emits_one_lowercased_token() {
    let terms = Analyzer::keyword().terms("New York City");
    assert_eq!(terms, ["new york city"]);
}

#[test]
fn analyzer_reports_its_filter_chain() {
    let names = Analyzer::english().filter_names();
    assert!(names.contains(&"lowercase"));
    assert!(names.contains(&"porter"));
    assert!(names.contains(&"stopwords"));
}

#[test]
fn analyzer_applies_filters_in_order() {
    // Stemming before lowercasing would leave `Running` unstemmed, since the
    // stemmer's rules are lowercase byte patterns.
    let analyzer = Analyzer::new(Box::new(StandardTokenizer::default()))
        .with(Box::new(Lowercase))
        .with(Box::new(StopwordFilter::new(["rust"])));

    assert!(analyzer.terms("Rust").is_empty());
}

#[test]
fn analyzer_with_no_filters_returns_raw_tokens() {
    let analyzer = Analyzer::new(Box::new(StandardTokenizer::default()));
    assert_eq!(analyzer.terms("The Fox"), ["The", "Fox"]);
}

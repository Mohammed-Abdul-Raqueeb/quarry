//! Turning text into terms.
//!
//! The pipeline is a tokenizer followed by an ordered list of filters, which is
//! the arrangement every mature search engine converges on. The important
//! property is that indexing and querying run the *same* pipeline: if `Running`
//! is indexed as `run`, the query `Running` has to become `run` too, or the
//! document is unfindable. [`Analyzer`] is the single object both sides use so
//! they cannot drift.

pub mod ascii;
pub mod ngram;
pub mod standard;
pub mod stemmer;
pub mod stopwords;

use std::fmt::Debug;

/// One term, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The term text, after filtering.
    pub text: String,
    /// Ordinal within the field, counting from zero.
    ///
    /// Phrase matching compares these, so a filter that drops a token must
    /// leave a gap rather than renumber — otherwise removing a stopword would
    /// make `"king of pop"` match `"king pop"`.
    pub position: u32,
    /// Byte offset of the original text in the input.
    pub start: usize,
    /// Byte offset just past the original text.
    pub end: usize,
}

impl Token {
    /// A token covering the given span.
    #[must_use]
    pub fn new(text: impl Into<String>, position: u32, start: usize, end: usize) -> Self {
        Self {
            text: text.into(),
            position,
            start,
            end,
        }
    }

    /// The token's text replaced, keeping its position and span.
    #[must_use]
    pub fn with_text(&self, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: self.position,
            start: self.start,
            end: self.end,
        }
    }

    /// Length in bytes of the original text.
    #[must_use]
    pub fn span_len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Splits text into tokens.
pub trait Tokenizer: Debug + Send + Sync {
    /// Splits `input`.
    fn tokenize(&self, input: &str) -> Vec<Token>;
}

/// Transforms a token stream.
pub trait TokenFilter: Debug + Send + Sync {
    /// Rewrites, drops or expands tokens.
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token>;

    /// A short name, used when describing an analyzer in diagnostics.
    fn name(&self) -> &'static str;
}

/// Lowercases every token.
///
/// Almost always the first filter: without it `Rust` and `rust` are different
/// terms, and a user typing either expects both.
#[derive(Debug, Default, Clone, Copy)]
pub struct Lowercase;

impl TokenFilter for Lowercase {
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token> {
        tokens
            .into_iter()
            .map(|t| {
                let lowered = t.text.to_lowercase();
                t.with_text(lowered)
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "lowercase"
    }
}

/// Drops tokens outside a length range.
///
/// Single characters are rarely worth an entry in the dictionary, and a 300
/// character "word" is a URL or a hash that will never be searched for.
#[derive(Debug, Clone, Copy)]
pub struct LengthFilter {
    /// Shortest token kept, in characters.
    pub min: usize,
    /// Longest token kept, in characters.
    pub max: usize,
}

impl Default for LengthFilter {
    fn default() -> Self {
        Self { min: 1, max: 64 }
    }
}

impl TokenFilter for LengthFilter {
    fn filter(&self, tokens: Vec<Token>) -> Vec<Token> {
        tokens
            .into_iter()
            .filter(|t| {
                let len = t.text.chars().count();
                len >= self.min && len <= self.max
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "length"
    }
}

/// A tokenizer plus its filters.
#[derive(Debug)]
pub struct Analyzer {
    tokenizer: Box<dyn Tokenizer>,
    filters: Vec<Box<dyn TokenFilter>>,
}

impl Analyzer {
    /// An analyzer with no filters.
    #[must_use]
    pub fn new(tokenizer: Box<dyn Tokenizer>) -> Self {
        Self {
            tokenizer,
            filters: Vec::new(),
        }
    }

    /// Appends a filter. Order matters.
    #[must_use]
    pub fn with(mut self, filter: Box<dyn TokenFilter>) -> Self {
        self.filters.push(filter);
        self
    }

    /// Runs the tokenizer and every filter.
    #[must_use]
    pub fn analyze(&self, input: &str) -> Vec<Token> {
        let mut tokens = self.tokenizer.tokenize(input);
        for filter in &self.filters {
            tokens = filter.filter(tokens);
        }
        tokens
    }

    /// Just the term strings.
    #[must_use]
    pub fn terms(&self, input: &str) -> Vec<String> {
        self.analyze(input).into_iter().map(|t| t.text).collect()
    }

    /// The names of the filters, in order.
    #[must_use]
    pub fn filter_names(&self) -> Vec<&'static str> {
        self.filters.iter().map(|f| f.name()).collect()
    }

    /// The default English pipeline.
    ///
    /// Lowercase, fold accents, drop stopwords, then stem. Folding before the
    /// stopword list matters: otherwise `thé` survives a list that contains
    /// `the`.
    #[must_use]
    pub fn english() -> Self {
        Self::new(Box::new(standard::StandardTokenizer::default()))
            .with(Box::new(Lowercase))
            .with(Box::new(ascii::AsciiFolding))
            .with(Box::new(LengthFilter::default()))
            .with(Box::new(stopwords::StopwordFilter::english()))
            .with(Box::new(stemmer::PorterStemmer))
    }

    /// Lowercase and fold, but neither stem nor drop stopwords.
    ///
    /// For fields where exactness matters more than recall — a product code, or
    /// prose that will only ever be phrase-matched.
    #[must_use]
    pub fn simple() -> Self {
        Self::new(Box::new(standard::StandardTokenizer::default()))
            .with(Box::new(Lowercase))
            .with(Box::new(ascii::AsciiFolding))
    }

    /// Emits the whole input as a single token.
    #[must_use]
    pub fn keyword() -> Self {
        Self::new(Box::new(standard::KeywordTokenizer)).with(Box::new(Lowercase))
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::english()
    }
}

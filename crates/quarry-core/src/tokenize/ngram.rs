//! Character n-grams.
//!
//! Two jobs that the word tokenizer cannot do:
//!
//! - **Languages without word separators.** Chinese and Japanese do not put
//!   spaces between words, so splitting on non-alphanumerics yields one
//!   enormous token. Bigrams are the standard cheap answer.
//! - **Substring and typo matching.** Indexing edge n-grams makes prefix
//!   matching a plain term lookup, which is how an as-you-type box stays fast
//!   without a wildcard scan over the dictionary.
//!
//! The cost is index size: every word becomes several terms, so postings grow
//! roughly with the n-gram range. Use it on one field, not everywhere.

use super::{Token, Tokenizer};

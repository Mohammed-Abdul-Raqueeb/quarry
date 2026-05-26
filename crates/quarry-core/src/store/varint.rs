//! LEB128 variable-length integers.
//!
//! Postings are the bulk of an index, and almost every number in one is small:
//! a document gap of 3, a term frequency of 1, a position of 12. A fixed `u32`
//! spends four bytes on each. LEB128 spends one for anything under 128, which
//! is where most of the compression in the segment format comes from.

use crate::error::{Error, Result};

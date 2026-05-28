//! Field definitions.
//!
//! A schema is fixed before indexing starts. Quarry is not schemaless on
//! purpose: knowing a field is text rather than a keyword at index time is what
//! decides whether it gets analyzed, whether positions are stored, and how it
//! is scored. Deciding that per document instead produces an index where the
//! same query means different things for different documents.

use crate::error::{Error, Result};
use std::collections::HashMap;

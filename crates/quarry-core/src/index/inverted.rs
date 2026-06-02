//! The in-memory inverted index.

use super::posting::{Posting, PostingList};
use crate::doc::DocId;
use crate::schema::FieldId;
use std::collections::{BTreeMap, HashMap};

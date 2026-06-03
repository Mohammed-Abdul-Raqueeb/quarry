//! Storage for the original field values.
//!
//! The inverted index can tell you *which* documents match but not what they
//! said — analysis is lossy, so `run` cannot be turned back into `running`.
//! Anything shown to a user, and anything highlighted, comes from here.

use super::blob::{BlobReader, BlobWriter};
use crate::doc::{DocId, Document, Value};
use crate::error::{Error, Result};
use crate::schema::{FieldId, FieldType, Schema};
use std::collections::BTreeMap;

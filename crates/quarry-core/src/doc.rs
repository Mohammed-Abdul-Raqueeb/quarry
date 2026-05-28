//! Documents as they enter the index.

use crate::error::{Error, Result};
use crate::json::Json;
use crate::schema::{FieldId, FieldType, Schema};
use std::collections::BTreeMap;

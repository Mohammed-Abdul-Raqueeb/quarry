//! A small JSON reader and writer.
//!
//! Quarry takes documents as JSON and the server answers in JSON, which is the
//! only reason this exists. `serde_json` would be the obvious dependency, but
//! the engine ships with none — and the subset needed here is a few hundred
//! lines, with the recursion depth capped so a hostile document cannot blow the
//! stack.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

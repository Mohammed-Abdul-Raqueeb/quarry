//! Posting lists.

use crate::doc::DocId;
use crate::error::{Error, Result};
use crate::schema::FieldId;
use crate::store::blob::{BlobReader, BlobWriter};

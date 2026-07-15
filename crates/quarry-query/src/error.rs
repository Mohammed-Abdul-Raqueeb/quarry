//! Query errors.
//!
//! These are separate from [`quarry_core::Error`] because they are *user*
//! errors, not engine errors. A malformed query is something to show the person
//! who typed it, with a position, so it carries structure a message string
//! could not.

use std::fmt;

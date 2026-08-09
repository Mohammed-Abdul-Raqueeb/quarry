//! HTTP parsing, routing and the thread pool.

use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::tokenize::Analyzer;
use quarry_core::{Document, IndexDir, IndexWriter, Value};
use quarry_server::http::{
    escape_json_string, parse_query, percent_decode, reason_phrase, Request, Response, MAX_BODY,
};
use quarry_server::routes::{route, AppState, MAX_LIMIT};
use quarry_server::ThreadPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

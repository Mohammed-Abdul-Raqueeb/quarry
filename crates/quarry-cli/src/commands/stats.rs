//! `quarry stats` — report what an index contains.

use crate::args::Args;
use crate::output::{fit, human_bytes, thousands, Style};
use quarry_core::{Error, IndexDir, IndexStats, Result};

/// Runs the command.
pub fn run(args: &Args, style: Style) -> Result<()> {
    let path = args.require("index", Some("i")).map_err(Error::schema)?;
    let show_terms = args.number("terms", Some("t"), 0).map_err(Error::schema)?;

    let dir = IndexDir::open(path)?;
    let schema = dir.schema();
    let stats = IndexStats::collect(dir.segments(), schema);

    println!("{}", style.bold("Index"));
    println!("  path        {}", style.cyan(path));
    println!("  documents   {}", thousands(u64::from(stats.docs)));
    println!("  segments    {}", stats.segments);
    println!("  terms       {}", thousands(stats.terms as u64));
    println!("  on disk     {}", human_bytes(dir.size_on_disk()?));

    if stats.docs > 0 {
        println!("  per doc     {:.0} bytes", stats.bytes_per_doc());
    }

    println!();
    println!("{}", style.bold("Fields"));
    println!(
        "  {} {} {} {} {}",
        fit("name", 16),
        fit("type", 9),
        fit("indexed", 8),
        fit("stored", 7),
        fit("positions", 9)
    );

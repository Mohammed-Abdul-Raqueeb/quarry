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

    for field in schema.fields() {
        println!(
            "  {} {} {} {} {}",
            style.cyan(&fit(&field.name, 16)),
            fit(field.ty.as_str(), 9),
            fit(yes_no(field.options.indexed), 8),
            fit(yes_no(field.options.stored), 7),
            fit(yes_no(field.options.positions), 9)
        );
    }

    // Per-field term counts come from the segments, so they are only available
    // once something has been indexed.
    if stats.docs > 0 {
        println!();
        println!("{}", style.bold("Terms per field"));

        for field in schema.fields() {
            let count: usize = dir
                .segments()
                .iter()
                .map(|s| s.index.terms_in_field(field.id).len())
                .sum();

            let total_length: u64 = dir
                .segments()
                .iter()
                .map(|s| s.index.field_stats(field.id).total_length)
                .sum();

            println!(
                "  {} {} distinct, {} total",
                style.cyan(&fit(&field.name, 16)),
                thousands(count as u64),
                thousands(total_length)
            );
        }
    }

    if show_terms > 0 {
        println!();
        println!("{}", style.bold("Most common terms"));

        for field in schema.fields() {
            // Frequencies are summed across segments rather than concatenated:
            // the same term lives in several segments, and listing it once per
            // segment reports the same word three times with a third of its
            // real count.
            let mut totals: std::collections::BTreeMap<String, u32> =
                std::collections::BTreeMap::new();

            for segment in dir.segments() {
                // Asking each segment for more than we will show leaves room
                // for a term that is mid-ranked everywhere but common overall.
                for (term, freq) in segment.index.most_frequent_terms(field.id, show_terms * 4) {
                    *totals.entry(term.to_string()).or_default() += freq;
                }
            }

            if totals.is_empty() {
                continue;
            }

            let mut all: Vec<(String, u32)> = totals.into_iter().collect();
            all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            all.truncate(show_terms);

            let rendered: Vec<String> = all
                .iter()
                .map(|(term, freq)| format!("{term} ({freq})"))
                .collect();

            println!(
                "  {} {}",
                style.cyan(&fit(&field.name, 16)),
                rendered.join(", ")
            );
        }
    }

    if stats.should_merge() {
        println!();
        println!(
            "{} {} segments; merging would reduce per-query overhead",
            style.dim("hint:"),
            stats.segments
        );
    }

    Ok(())
}

//! `quarry merge` — fold segments into one.

use crate::args::Args;
use crate::output::{human_bytes, human_duration, thousands, Style};
use quarry_core::index::merge::merge_with_report;
use quarry_core::{Error, IndexDir, Result};
use std::time::Instant;

/// Runs the command.
pub fn run(args: &Args, style: Style) -> Result<()> {
    let path = args.require("index", Some("i")).map_err(Error::schema)?;
    let dry_run = args.flag("dry-run", None);

    let mut dir = IndexDir::open(path)?;

    let removed = dir.clean_temp_files()?;
    if removed > 0 {
        println!(
            "{} {removed} temporary file(s) from an interrupted write",
            style.yellow("removed")
        );
    }

    if dir.segments().len() < 2 {
        println!(
            "{} {} segment; nothing to merge",
            style.dim("skipped:"),
            dir.segments().len()
        );
        return Ok(());
    }

    let before = dir.segments().len();
    let started = Instant::now();

    if dry_run {
        // Reports what a merge would achieve without touching the directory,
        // which is the only honest way to answer "is this worth doing".
        let (_, report) = merge_with_report(dir.segments(), dir.schema())?;

        println!("{}", style.bold("Dry run"));
        println!("  segments    {before} -> 1");
        println!(
            "  bytes       {} -> {} ({:.1}% smaller)",
            human_bytes(report.bytes_before as u64),
            human_bytes(report.bytes_after as u64),
            report.bytes_saved_ratio() * 100.0
        );
        println!(
            "  terms       {} -> {} ({} collapsed)",
            thousands(report.terms_before as u64),
            thousands(report.terms_after as u64),
            thousands(report.terms_collapsed() as u64)
        );
        println!();
        println!("{}", style.dim("nothing was changed"));
        return Ok(());
    }

    let (_, report) = merge_with_report(dir.segments(), dir.schema())?;
    dir.merge_all()?;

    println!("{}", style.green("merged"));
    println!("  segments    {before} -> {}", dir.segments().len());
    println!("  documents   {}", thousands(u64::from(report.docs)));
    println!(
        "  bytes       {} -> {}",
        human_bytes(report.bytes_before as u64),
        human_bytes(report.bytes_after as u64)
    );
    println!(
        "  terms       {} collapsed",
        thousands(report.terms_collapsed() as u64)
    );
    println!("  took        {}", human_duration(started.elapsed()));

    Ok(())
}

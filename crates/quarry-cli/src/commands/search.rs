//! `quarry search` — run a query.

use crate::args::Args;
use crate::output::{fit, human_duration, thousands, Style};
use quarry_core::search::HighlightOptions;
use quarry_core::tokenize::Analyzer;
use quarry_core::{Error, IndexDir, Result, Searcher};
use quarry_query::{parse, plan, PlanContext};

/// Runs the command.
pub fn run(args: &Args, style: Style) -> Result<()> {
    let path = args.require("index", Some("i")).map_err(Error::schema)?;
    let limit = args.number("limit", Some("n"), 10).map_err(Error::schema)?;
    let as_json = args.flag("json", None);
    let explain = args.flag("explain", None);
    let no_snippets = args.flag("no-snippets", None);

    // Everything after the subcommand is the query, so it need not be quoted.
    let query_text = args.joined_positional(0);
    if query_text.trim().is_empty() {
        return Err(Error::schema(
            "no query given; try `quarry search --index ./idx rust ownership`",
        ));
    }

    let dir = IndexDir::open(path)?;
    let schema = dir.schema();
    let analyzer = Analyzer::english();

    let ast = parse(&query_text).map_err(|err| Error::schema(err.to_string()))?;

    if explain {
        println!("{} {}", style.dim("parsed:"), ast);
    }

    let ctx = PlanContext {
        schema,
        analyzer: &analyzer,
        segments: dir.segments(),
    };

    let planned = plan(&ast, &ctx).map_err(|err| Error::schema(err.to_string()))?;

    for note in &planned.notes {
        eprintln!("{} {note}", style.yellow("note:"));
    }

    if explain {
        println!(
            "{} {} must, {} should, {} must-not, {} phrase",
            style.dim("planned:"),
            planned.query.must.len(),
            planned.query.should.len(),
            planned.query.must_not.len(),
            planned.query.phrases.len()
        );
    }

    let searcher = Searcher::new(dir.segments(), schema);
    let results = searcher.search(&planned.query, limit)?;

    if as_json {
        print_json(&searcher, &results, schema);
        return Ok(());
    }

    print_human(
        &searcher,
        &results,
        schema,
        &analyzer,
        &planned.query.all_terms(),
        style,
        no_snippets,
    );

    Ok(())
}

fn print_human(
    searcher: &Searcher<'_>,
    results: &quarry_core::Results,
    schema: &quarry_core::Schema,
    analyzer: &Analyzer,
    terms: &[String],
    style: Style,
    no_snippets: bool,
) {
    if results.is_empty() {
        println!("{}", style.dim("no matches"));
        return;
    }

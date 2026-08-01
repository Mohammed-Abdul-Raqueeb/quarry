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

    println!(
        "{} match{} in {}",
        thousands(results.total_hits),
        if results.total_hits == 1 { "" } else { "es" },
        human_duration(results.elapsed)
    );
    println!();

    let snippet_options = if style.is_enabled() {
        HighlightOptions::terminal()
    } else {
        HighlightOptions::brackets()
    };

    for (rank, hit) in results.hits.iter().enumerate() {
        println!(
            "{} {}  {}",
            style.dim(&format!("{:>3}.", rank + 1)),
            style.bold(&format!("{:.4}", hit.score)),
            style.dim(&format!("#{}", hit.doc))
        );

        let Some(doc) = searcher.document(hit) else {
            continue;
        };

        for field in schema.fields() {
            let values = doc.values(field.id);
            if values.is_empty() {
                continue;
            }

            let text = values[0].as_text();

            let rendered = if no_snippets {
                fit(&text, 100)
            } else {
                searcher
                    .snippet(hit, field.id, terms, analyzer, &snippet_options)
                    .unwrap_or_else(|| fit(&text, 100))
            };

            println!(
                "     {} {}",
                style.cyan(&format!("{}:", field.name)),
                rendered.trim()
            );
        }

        println!();
    }
}

fn print_json(
    searcher: &Searcher<'_>,
    results: &quarry_core::Results,
    schema: &quarry_core::Schema,
) {
    use quarry_core::json::{Json, JsonBuilder};

    let hits: Vec<Json> = results
        .hits
        .iter()
        .map(|hit| {
            let document = searcher
                .document(hit)
                .map_or(Json::Null, |doc| doc.to_json(schema));

            JsonBuilder::new()
                .num("doc", f64::from(hit.doc))
                .num("score", f64::from(hit.score))
                .set("document", document)
                .build()
        })
        .collect();

    let body = JsonBuilder::new()
        .num("total", results.total_hits as f64)
        .num("took_us", results.elapsed.as_micros() as f64)
        .set("hits", Json::Array(hits))
        .build();

    println!("{body}");
}

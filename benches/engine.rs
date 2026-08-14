//! Indexing and query benchmarks.
//!
//! Written against `std::time` rather than Criterion, because Criterion is a
//! dependency and the point of a benchmark here is a number you can reproduce,
//! not a statistical model of it. Each case runs a warmup, then enough
//! iterations to take a measurable amount of time, and reports the median plus
//! the spread so an outlier is visible rather than averaged away.
//!
//!   cargo bench

use quarry_core::schema::{Schema, SchemaBuilder};
use quarry_core::tokenize::Analyzer;
use quarry_core::{
    BooleanQuery, Document, IndexWriter, PhraseQuery, Searcher, Segment, TermQuery, Value,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Words assembled into synthetic documents.
///
/// A fixed vocabulary with a skewed distribution, because a uniform one makes
/// every term equally selective and hides the difference between a common and
/// a rare term — which is most of what query performance depends on.
const VOCABULARY: &[&str] = &[
    "rust",
    "ownership",
    "borrow",
    "checker",
    "compile",
    "time",
    "memory",
    "safety",
    "thread",
    "concurrency",
    "iterator",
    "abstraction",
    "zero",
    "cost",
    "trait",
    "lifetime",
    "reference",
    "mutable",
    "immutable",
    "allocation",
    "stack",
    "heap",
    "pointer",
    "slice",
    "vector",
    "string",
    "closure",
    "generic",
    "monomorphise",
    "inline",
    "optimise",
    "benchmark",
    "profile",
    "latency",
    "throughput",
    "index",
    "segment",
    "posting",
    "dictionary",
    "term",
    "frequency",
    "document",
    "query",
    "parser",
    "planner",
    "scorer",
    "collector",
    "snippet",
    "highlight",
    "merge",
];

fn schema() -> Schema {
    Schema::builder()
        .text("title")
        .and_then(|b| b.text("body"))
        .and_then(|b| b.keyword("tag"))
        .and_then(SchemaBuilder::build)
        .expect("schema should build")
}

/// A deterministic pseudo-random generator.
///
/// Seeded and reproducible so two runs benchmark the same corpus; a real RNG
/// would make run-to-run comparison meaningless.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*, which is three shifts and a multiply.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    /// A word drawn from a Zipf-like distribution.
    ///
    /// Real prose has a few very common words and a long tail of rare ones, and
    /// that shape is most of what determines query cost: a common term means a
    /// huge posting list to score, a rare one means a short walk. A fixed
    /// fifty-word vocabulary produces neither — every list is enormous, the
    /// dictionary is tiny, and front-coding and merging have nothing to show.
    ///
    /// So the common words come from the hand-written list and the tail is
    /// synthesised, giving a dictionary of roughly `TAIL_SIZE` entries with
    /// realistically skewed frequencies.
    fn word(&mut self) -> String {
        const TAIL_SIZE: usize = 20_000;

        let roll = self.below(1000);

        if roll < 400 {
            // The 400-per-mille head: a handful of words, very frequent.
            VOCABULARY[self.below(8)].to_string()
        } else if roll < 700 {
            VOCABULARY[self.below(VOCABULARY.len())].to_string()
        } else if roll < 950 {
            // Mid-frequency band.
            format!("term{:04}", self.below(TAIL_SIZE / 20))
        } else {
            // The long tail, where most of the dictionary lives.
            format!("term{:04}", self.below(TAIL_SIZE))
        }
    }

    fn sentence(&mut self, words: usize) -> String {
        (0..words)
            .map(|_| self.word())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn corpus(count: usize) -> Vec<Document> {
    let mut rng = Rng(0x5EED_1234_5678_9ABC);

    (0..count)
        .map(|_| {
            let mut doc = Document::new();
            doc.add(0, Value::Text(rng.sentence(6)));
            doc.add(1, Value::Text(rng.sentence(60)));
            doc.add(2, Value::Keyword(VOCABULARY[rng.below(8)].to_string()));
            doc
        })
        .collect()
}

fn index_corpus(docs: &[Document]) -> Vec<Segment> {
    let mut writer = IndexWriter::new(schema());
    for doc in docs {
        writer.add(doc).expect("should index");
    }
    writer.flush().map(|s| vec![s]).unwrap_or_default()
}

/// One measured result.
struct Measurement {
    label: String,
    median: Duration,
    best: Duration,
    worst: Duration,
    iterations: u32,
    per_op: Option<f64>,
}

impl Measurement {
    fn report(&self) {
        let median = format_duration(self.median);
        let spread = format!(
            "{} … {}",
            format_duration(self.best),
            format_duration(self.worst)
        );

        print!(
            "  {:<44} {:>12}   [{:>24}]  x{}",
            self.label, median, spread, self.iterations
        );

        if let Some(rate) = self.per_op {
            print!("   {rate:>12.0}/s");
        }
        println!();
    }
}

/// Runs `body` repeatedly and reports the median of several samples.
fn measure(label: &str, samples: u32, iterations: u32, mut body: impl FnMut()) -> Measurement {
    // One untimed pass, so the first measurement does not pay for cold caches
    // and lazily initialised statics.
    body();

    let mut timings = Vec::with_capacity(samples as usize);

    for _ in 0..samples {
        let started = Instant::now();
        for _ in 0..iterations {
            body();
        }
        timings.push(started.elapsed() / iterations);
    }

    timings.sort_unstable();

    let median = timings[timings.len() / 2];
    let per_op = if median.as_secs_f64() > 0.0 {
        Some(1.0 / median.as_secs_f64())
    } else {
        None
    };

    Measurement {
        label: label.to_string(),
        median,
        best: timings[0],
        worst: timings[timings.len() - 1],
        iterations,
        per_op,
    }
}

fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();

    if nanos < 1_000 {
        format!("{nanos}ns")
    } else if nanos < 1_000_000 {
        format!("{:.1}µs", nanos as f64 / 1_000.0)
    } else if nanos < 1_000_000_000 {
        format!("{:.2}ms", nanos as f64 / 1_000_000.0)
    } else {
        format!("{:.2}s", nanos as f64 / 1_000_000_000.0)
    }
}

fn section(title: &str) {
    println!();
    println!("{title}");
    println!("{}", "─".repeat(100));
}

fn main() {
    println!("Quarry {} benchmarks", quarry_core::VERSION);
    println!("Each line: median of 9 samples, with the best and worst sample in brackets.");

    bench_analysis();
    bench_indexing();
    bench_codec();
    bench_queries();
    bench_merge();

    println!();
}

fn bench_analysis() {
    section("Analysis");

    let analyzer = Analyzer::english();
    let short = "Rust ownership and the borrow checker";
    let long = Rng(1).sentence(500);

    measure("analyze a 6-word title", 9, 20_000, || {
        black_box(analyzer.terms(short));
    })
    .report();

    measure("analyze a 500-word body", 9, 500, || {
        black_box(analyzer.terms(&long));
    })
    .report();

    measure("stem one word", 9, 200_000, || {
        black_box(quarry_core::tokenize::stemmer::stem("connections"));
    })
    .report();
}

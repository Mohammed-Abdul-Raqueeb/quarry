# Performance

Numbers from `cargo bench` on the machine they were measured on. Reproduce them before trusting
them — the absolute values are hardware, and only the *ratios* are properties of the code.

```bash
cargo bench
```

## The corpus

The benchmark generates documents from a Zipf-distributed vocabulary: eight very common words, a
fifty-word head, and a synthesised tail of about 20,000 terms. Each document has a six-word title
and a sixty-word body.

The distribution is the point. A fixed small vocabulary makes every posting list enormous and the
dictionary tiny, which hides the single biggest factor in query cost — how selective a term is — and
leaves merging and front-coding with nothing to collapse. An earlier version of this benchmark did
exactly that and reported a 0.1% merge saving, which was an artefact of the generator rather than a
fact about the code.

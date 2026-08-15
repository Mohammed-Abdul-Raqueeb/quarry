# Segment format

Version 4. One segment is one immutable file, conventionally `NNNNNNNN.qseg` inside an index
directory.

## Framing

```
┌──────────────┬─────────┬──────────┬─────────────┬──────────────┐
│ magic "QRYS" │ version │ body     │ CRC32(body) │ magic "QRYE" │
│ 4 bytes      │ u32 LE  │ variable │ u32 LE      │ 4 bytes      │
└──────────────┴─────────┴──────────┴─────────────┴──────────────┘
```

Minimum valid size is 16 bytes. Everything multi-byte is little-endian.

Both magics are load-bearing, and the reader checks the trailing one **before** the version:

- No trailing magic means the file was truncated, which is what a process killed mid-write leaves.
- A trailing magic with a failing CRC means the bytes were damaged in storage.

They are reported differently because they call for different responses, and reading the version out
of a truncated file would otherwise produce a nonsense version number instead of a clear diagnosis.

## Body

Sections in order. `varint` is unsigned LEB128; `string` is a varint length followed by UTF-8 bytes.

### 1. Schema digest

```
u64   FNV-1a of the schema shape
```

Field ids are positional, so reading a segment with a reordered schema would attribute every term to
the wrong field. The digest covers field names, types, `indexed` and `positions`. It deliberately
excludes `stored` and `boost`: changing either does not invalidate existing postings, so including
them would force a needless reindex.

### 2. Document count

```
varint   documents in this segment
```

### 3. Field statistics

```
varint   number of fields
  varint   field id
  varint   documents with any value for the field
  varint   total terms across those documents
```

Used for BM25's average field length.

### 4. Per-document field lengths

```
varint   number of field groups
  varint   field id
  varint   number of entries
  entries, ascending by document id:
    varint   document id, as a gap from the previous
    varint   length in terms
```

Grouped by field so the ids ascend within each group and gap-code well.

### 5. Term dictionary and postings

```
varint   number of terms
  varint   field id
  varint   shared prefix length with the previous term
  string   the remaining suffix
  posting list
```

Terms arrive sorted, so adjacent entries usually share a prefix. `borrow`, `borrowed` and
`borrowing` cost the six shared characters once plus two short suffixes, rather than twenty-one
characters.

A posting list:

```
varint   field id
varint   number of postings
  varint          document id, as a gap from the previous
  varint          term frequency
  sorted u32 list positions
```

A sorted list is a varint count followed by varint gaps. Positions are empty when the field was
indexed without them.

### 6. Stored fields

```
varint   number of documents
  varint   document id, as a gap from the previous
  varint   number of values
  values:
    u8       type tag: 1 text, 2 keyword, 3 number
    string, or u64 of f64::to_bits for a number
```

Only fields the schema marks `stored` appear here. Numbers are stored as raw bits so they
round-trip exactly rather than through a decimal rendering.

## Why gap encoding

Almost every number in a posting list is small: a document gap of 3, a frequency of 1, a position of
12. A fixed `u32` spends four bytes on each; LEB128 spends one for anything under 128.

Measured on 300 consecutive document ids with a frequency of 1, the encoded list comes in under four
bytes per posting against eight for a fixed layout — and that ratio is what the whole format is
built around.

## Reader guarantees

Every length is bounded and every read is checked, because the input is a file that may have been
truncated or damaged:

- A `Vec::with_capacity` taken from an on-disk count is clamped, so a corrupt length cannot request
  a multi-gigabyte allocation before the first read fails.
- Document ids are checked against the segment's document count; a posting referencing a
  nonexistent document is corruption.
- A front-coded prefix length longer than the previous term, or one landing mid-character in UTF-8,
  is rejected.
- A varint with redundant trailing-zero padding is rejected, so two byte sequences cannot encode the
  same value.
- Every gap addition is checked for overflow.

The test suite flips every single byte of a small segment's body and asserts each one is detected.

## Compatibility

The version is checked exactly: a segment written by a different version is refused rather than read
on a best-effort basis. There is no migration path — reindex instead. For a format this young that
is the honest trade, and the error message names both versions so the situation is obvious.

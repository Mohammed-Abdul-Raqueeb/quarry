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

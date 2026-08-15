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

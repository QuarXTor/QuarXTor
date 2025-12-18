# Block Model

Block is the minimal unit of QuarXTor storage.

## Block Structure

Each block has:

- `id` — content hash (BLAKE3)
- `size` — size in bytes
- `tier` — RAM / SSD / HDD / remote
- `refs` — reference count
- `entropy` — approximate entropy (used for type inference)
- `meta` — optional flags or extended attributes

## Chunking

Initial prototype uses fixed-size blocks.
Later versions may introduce variable-size chunking.

## On-Disk Layout

Blocks are stored inside segment files:

- `segment_0001.dat`
- `segment_0002.dat`
- ...

Index maps:

- hash → (segment, offset, size)

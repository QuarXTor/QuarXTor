# Hash Graph

Hash Graph models structural and semantic relationships between blocks.

## Vertices

Vertices correspond to block IDs.

## Edges

Edges represent relationships:

- structural:
  - next block in a file
  - directory → file
- semantic:
  - versioning
  - snapshots

## Purpose

Graph enables:

- deduction of repeated structures
- global deduplication across datasets
- reconstruction of logical objects
- analytics and pattern detection

## Storage

Graph is stored as:

- adjacency lists
- lightweight metadata per edge
- compressed index

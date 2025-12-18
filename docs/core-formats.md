# QuarXTor core on-disk formats

This document summarizes the stable on-disk layouts of the QuarXCore engine. These layouts are treated as ABI: any incompatible change requires an explicit format/versioning decision and a migration plan. Network protocol formats are documented separately.

## General invariants

- On-disk layouts used by QuarXCore are treated as stable ABI.
- The byte-level encoding is defined by the implementations in `core-rs/src/store/*.rs` and `core-rs/src/codec/*.rs`.
- Any incompatible change to these layouts requires explicit versioning and migration.
- This document describes the logical block families and their roles; consult the source code for exact framing and encoding details.

## Block model overview

- `BlockKind` (`core-rs/src/types.rs`) enumerates block families stored in the repository.
- `L0Block` (`core-rs/src/block/l0.rs`): atomic physical block.
- `MultiBlock`, `MultiRecipe`, `CodecRef`, `DictRef` (`core-rs/src/block/multi.rs`): logical multi-blocks referencing L0 blocks or codec recipes.
- `ZBlock` (`core-rs/src/block/zblock.rs`) with `ZPayload` (`core-rs/src/codec/z.rs`): structural overlays over ranges of L0 blocks.
- `Object` (`core-rs/src/block/object.rs`) with `ObjectPayload` (`core-rs/src/codec/object.rs`): top-level objects with root references and metadata.

## L0 blocks

- Purpose: atomic physical blocks stored by identifier and hash.
- Types: `L0Block` and its payload codec in `core-rs/src/codec/l0.rs`.
- The encoder/decoder modules define how headers and payloads are serialized; this representation is treated as stable ABI.

## Multi blocks

- Purpose: logical blocks that aggregate multiple L0 blocks and/or reference codec-based recipes.
- Types: `MultiBlock`, `MultiRecipe`, `CodecRef`, `DictRef`.
- `MultiRecipe` describes how a logical block is reconstructed (aggregation, codec/dictionary references, or custom recipes).
- The encoding of Multi recipes and payloads is defined in `core-rs/src/codec/multi.rs` and is treated as stable ABI.

## Structural / Z-blocks

- Purpose: structural or analytical overlays attached to a range of L0 blocks.
- Types: `ZBlock` and `ZPayload`.
- A Z-block carries a reference to a contiguous range of L0 identifiers plus payload describing structural metadata.
- The concrete encoding is defined in `core-rs/src/codec/z.rs` and is treated as stable ABI.

## Object blocks

- Purpose: top-level objects such as files, snapshots, images, or other application-defined entities.
- Types: `Object` and `ObjectPayload`.
- An Object carries a root `BlockRef`, an object type identifier, and opaque metadata.
- The exact payload encoding is defined in `core-rs/src/codec/object.rs` and is treated as stable ABI.

## Store layout

- The repository store is implemented as an append-only sequence of encoded blocks (e.g., `FileBlockStore` in `core-rs/src/store/file_store.rs`).
- Framing and on-disk headers for blocks are defined in `core-rs/src/store/encode.rs` and `core-rs/src/store/decode.rs`.
- These framing rules constitute part of the stable on-disk ABI and must not be changed without explicit versioning/migration.

## Extension and compatibility rules

- On-disk structures must not be changed in backward-incompatible ways without a format bump and migration plan.
- Prefer additive/tagged extensions while keeping existing encodings stable.
- Readers should handle reserved or unknown data according to existing decoder behavior.

## Code reference (stable ABI types)

- `core-rs/src/block/l0.rs`: `L0Block`
- `core-rs/src/block/multi.rs`: `MultiBlock`, `MultiRecipe`, `CodecRef`, `DictRef`
- `core-rs/src/block/zblock.rs`: `ZBlock`
- `core-rs/src/block/object.rs`: `Object`
- `core-rs/src/codec/l0.rs`: L0 payload codec
- `core-rs/src/codec/multi.rs`: Multi payload codec
- `core-rs/src/codec/z.rs`: Z payload codec
- `core-rs/src/codec/object.rs`: Object payload codec
- `core-rs/src/store/encode.rs` / `core-rs/src/store/decode.rs`: block framing and store-level encoding/decoding

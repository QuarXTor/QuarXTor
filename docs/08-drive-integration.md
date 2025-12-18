# QuarXDrive – Clients and Integration

## CLI Tools

Examples:

- `quarx put <path>`
- `quarx get <id>`
- `quarx stats`

## Filesystem Integration

FUSE read-only view (later write support):

- map directories to logical objects
- reconstruct paths via hash-graph

## SDKs

Language bindings for:

- Python
- Go
- Rust

## ZNode / Proto

- ZNode/ZNodeKind are defined in the shared crate `quarxproto` (bincode ABI).
- `quarxdrive` writes/reads ZNode via this proto format (repo.znode helpers).
- Network path (`quarxnet` client/server) uses the same proto for `client_push_znode` / `client_get_znode`.
- Treat `quarxproto` as the single source of truth for ZNode payloads; avoid duplicating definitions in drive/net.

# QuarXNet API (RPC / HTTP)

QuarXNet exposes a simplified network interface around QuarXCore.

## Endpoints (prototype)

### PUT /object
Upload object → returns object ID.

### GET /object/<id>
Download object.

### GET /stats
Return engine statistics.

### GET /block/<hash>
Return info / raw block.

## Proto / ZNode

- ZNode/ZNodeKind live in the shared crate `quarxproto` (bincode format).
- Client helpers `client_push_znode` / `client_get_znode` in `quarxnet` use this format unchanged.
- Treat proto types as part of the stable surface between `quarxnet` and `quarxdrive`; no ad-hoc structs in net.

## Transport

Plan to use:
- HTTP(s)
- optional gRPC

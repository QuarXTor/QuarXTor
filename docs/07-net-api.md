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

## Transport

Plan to use:
- HTTP(s)
- optional gRPC

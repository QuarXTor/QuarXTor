# Tiering (RAM / SSD / HDD)

QuarXTor supports multi-tier data placement.

## Tiers

- Tier 0 — RAM (hot data)
- Tier 1 — SSD (warm data)
- Tier 2 — HDD (cold data)

## Promotion

Blocks are promoted if:

- frequently accessed
- recently used
- part of active logical objects

## Demotion

Blocks are demoted if:

- idle
- rarely accessed
- large but cold

Promotion/demotion strategies evolve over time.

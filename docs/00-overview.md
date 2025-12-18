# Overview

QuarXTor is a hash-graph-based storage system designed around:

- content addressing
- structural deduplication
- RAM-aware data placement
- multi-tier storage (RAM, SSD, HDD)
- introspection and telemetry
- pluggable networking and orchestration layers

The system is split into four major components:

- QuarXCore — core storage engine
- QuarXNet — networking and RPC layer
- QuarXDrive — client-side tools and filesystem integration
- QuarXCloud — multi-node orchestration

This document describes the roles and interactions of each subsystem.

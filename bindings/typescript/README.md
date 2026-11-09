# TypeScript bindings

This directory contains the TypeScript/Node binding planned for npm and
consumption by the Vectora GH Bot. It delegates execution to the canonical Rust
implementation; TypeScript code here is limited to the API/FFI boundary, Node
ergonomics, packaging, and compatibility tests.

Define and test ownership, error mapping, cancellation, async streaming, and
shutdown behavior against the shared engine contracts. Do not reimplement
provider, runtime, or storage policy in TypeScript. Keep npm metadata, native
artifact build configuration, generated TypeScript declarations, and
binding-specific tests here.

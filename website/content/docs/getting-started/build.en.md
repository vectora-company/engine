---
title: Build and verify
description: Build the Rust core and language bindings with SCons.
weight: 2
---

Run commands from the repository root:

```bash
scons                 # list targets
scons install         # dependencies and development sidecars; no compilation
scons build           # compile the Rust core and present bindings
scons build-core      # compile only the core crate
scons build-python    # build the PyO3 package
scons build-typescript # build the N-API package
scons lint
scons tests
```

`scons build` shares Cargo's `target/` directory across the core and native bindings, so Cargo's incremental artifacts can be reused on subsequent builds. A clean build or a toolchain/feature change may require substantial compilation.

The test target runs Rust tests, Python tests, and TypeScript binding tests. Some storage integration cases require PostgreSQL, Qdrant, or Redis services and are environment-dependent.

For local website work, use `scons docs-serve`; validate a static build with `scons docs-build`. Neither command publishes the site.

# Python bindings

This directory contains the Python binding planned for PyPI and consumption by
the Vectora Backend. It delegates execution to the canonical Rust
implementation; Python code here is limited to the API/FFI boundary, Python
ergonomics, packaging, and compatibility tests.

Define and test ownership, error mapping, cancellation, async streaming, and
shutdown behavior against the shared engine contracts. Do not reimplement
provider, runtime, or storage policy in Python. Keep PyPI metadata, wheel/source
build configuration, and binding-specific tests here.

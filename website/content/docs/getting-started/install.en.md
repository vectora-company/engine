---
title: Prerequisites and installation
description: Install host tooling separately from project dependencies.
weight: 1
---

## Linux host setup

The repository's `dotfiles.sh` prepares global Linux development tooling only. It detects the distribution and installs host packages and toolchains such as Rust, Protocol Buffers, Podman, Python via pyenv, Node.js, uv, pnpm, SCons, and pre-commit. It does not install this checkout's dependencies, compile the project, or start containers.

```bash
./dotfiles.sh --check  # report missing host prerequisites
./dotfiles.sh          # install/configure host prerequisites
```

On macOS and Windows, install the prerequisites listed in the repository's `CONTRIBUTING.md` using the platform's package managers.

## Install checkout dependencies

From the repository root:

```bash
scons install
```

This synchronizes locked Rust/Python/Node dependencies, installs the NATS development sidecar, pulls PostgreSQL, Qdrant, and Redis development images with Podman, and downloads Go modules if Go bindings exist. It does not compile the engine or start those services.

Python dependencies are managed with `uv`; Node dependencies use pnpm. `pyenv` is only used to install/select a Python interpreter on Linux. Use `uv` for project environments and Python commands.

## Optional integration services

The storage integration tests can use local Podman services. `scons install` pulls their images but does not start them. Start services only when you need those integration tests, following the repository's development instructions. Core unit tests and normal build do not require these servers to be running.

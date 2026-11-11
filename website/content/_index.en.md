---
title: Vectora Engine
type: docs
description: A reusable Rust runtime and framework for building agentic applications.
locale: en_US
---

# Build agentic products on one runtime

Vectora Engine is an open-source Rust runtime for reusable agent capabilities. It provides a canonical implementation with Python and TypeScript/Node bindings, so different products can share contracts and behavior instead of maintaining separate engines.

The engine is designed to serve products with different interfaces: an engineering assistant, a GitHub bot, a personal assistant, or an application builder. Each host owns its product experience, identity, credentials, integrations, and deployment choices; the engine supplies reusable runtime capabilities.

{{< hextra/feature-grid >}}
  {{< hextra/feature-card title="One Rust core" subtitle="Python and TypeScript bindings call the same implementation." >}}
  {{< hextra/feature-card title="Composable capabilities" subtitle="Chat, streaming, tool coordination, policy, persistence, and verification contracts." >}}
  {{< hextra/feature-card title="Explicit boundaries" subtitle="Experimental status and host responsibilities are documented alongside each capability." >}}
{{< /hextra/feature-grid >}}

## Start here

- [Getting started](docs/getting-started/): install the development prerequisites and build the workspace.
- [Architecture](docs/concepts/architecture/): understand the engine and host boundary.
- [Capabilities and maturity](docs/reference/capabilities/): see what is implemented, experimental, partial, or unavailable.
- [Guides](docs/guides/): use chat, harness, and storage APIs responsibly.
- [Language bindings](docs/languages/): Rust, Python, and TypeScript/Node.

> **Maturity notice:** contracts and bindings are evolving. Most runtime interfaces are experimental and are not yet stable integration targets. The Python and npm packages are not currently published; use the repository checkout for development.

The source repository is [vectora-ltda/engine](https://github.com/vectora-ltda/engine), licensed under GNU AGPL-3.0.

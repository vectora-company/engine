---
title: TypeScript and Node.js
description: napi-rs binding for Node.js consumers.
weight: 3
---

The `@vectora/engine` package uses napi-rs to expose the Rust core to Node.js. It is intended for TypeScript hosts such as GH Bot. The npm package is not published yet; build it from this checkout.

```bash
scons install
cd bindings/typescript
pnpm build
pnpm test
```

The TypeScript facade uses camelCase names such as `chatComplete`, `chatStream`, `nativeChatComplete`, `assembleTurn`, `startModelToolLoop`, `prepareToolDispatch`, `evaluateVerification`, and SQLite store operations. Streams are async iterables; stopping iteration cancels the native stream path.

```ts
import { chatContractVersion } from "@vectora/engine";

console.log(chatContractVersion());
```

For a local checkout, use the package workspace/build instructions before importing. See `bindings/typescript/README.md` for current details. The interface is experimental and not every Rust module is necessarily exported.

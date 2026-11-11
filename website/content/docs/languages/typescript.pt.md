---
title: TypeScript e Node.js
description: Binding napi-rs para consumidores Node.js.
weight: 3
---

O pacote `@vectora/engine` usa napi-rs para expor o núcleo Rust ao Node.js. Ele é destinado a hosts TypeScript, como o GH Bot. Ainda não está publicado no npm; compile a partir deste checkout.

```bash
scons install
cd bindings/typescript
pnpm build
pnpm test
```

A fachada TypeScript usa nomes em camelCase, como `chatComplete`, `chatStream`, `nativeChatComplete`, `assembleTurn`, `startModelToolLoop`, `prepareToolDispatch`, `evaluateVerification` e operações dos stores SQLite. Streams são async iterables; interromper a iteração cancela o caminho de streaming nativo.

```ts
import { chatContractVersion } from "@vectora/engine";

console.log(chatContractVersion());
```

Em checkout local, siga as instruções de workspace/build do pacote antes de importá-lo. Consulte `bindings/typescript/README.md` para os detalhes atuais. A interface é experimental e nem todo módulo Rust necessariamente está exportado.

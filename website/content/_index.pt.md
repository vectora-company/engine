---
title: Vectora Engine
type: docs
description: Um runtime e framework reutilizável em Rust para criar aplicações agênticas.
locale: pt_BR
---

# Crie produtos agênticos sobre um único runtime

A Vectora Engine é um runtime open source em Rust para capacidades reutilizáveis de agentes. Ela fornece uma implementação canônica com bindings Python e TypeScript/Node, permitindo que produtos diferentes compartilhem contratos e comportamento em vez de manter motores independentes.

A engine foi pensada para produtos com interfaces distintas: um assistente de engenharia, um bot para GitHub, um assistente pessoal ou um construtor de aplicações. Cada aplicação hospedeira é responsável pela experiência de produto, identidade, credenciais, integrações e implantação; a engine fornece capacidades reutilizáveis do runtime.

{{< hextra/feature-grid >}}
  {{< hextra/feature-card title="Um núcleo Rust" subtitle="Os bindings Python e TypeScript chamam a mesma implementação." >}}
  {{< hextra/feature-card title="Capacidades combináveis" subtitle="Contratos de chat, streaming, coordenação de tools, policy, persistência e verificação." >}}
  {{< hextra/feature-card title="Limites explícitos" subtitle="Maturidade experimental e responsabilidades do host são documentadas junto a cada capacidade." >}}
{{< /hextra/feature-grid >}}

## Comece por aqui

- [Primeiros passos](docs/getting-started/): instale os pré-requisitos de desenvolvimento e compile o workspace.
- [Arquitetura](docs/concepts/architecture/): entenda a fronteira entre engine e aplicação hospedeira.
- [Capacidades e maturidade](docs/reference/capabilities/): veja o que está implementado, experimental, parcial ou indisponível.
- [Guias](docs/guides/): use as APIs de chat, harness e storage com segurança.
- [Bindings por linguagem](docs/languages/): Rust, Python e TypeScript/Node.

> **Maturidade:** contratos e bindings continuam evoluindo. A maioria das interfaces do runtime é experimental e ainda não é um destino estável de integração. Os pacotes Python e npm ainda não foram publicados; use o checkout do repositório para desenvolvimento.

O repositório oficial é [vectora-ltda/engine](https://github.com/vectora-ltda/engine), sob a licença GNU AGPL-3.0.

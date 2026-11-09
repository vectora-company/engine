# Especificações da Vectora Engine

Estes documentos adaptam especificações de produto para capacidades genéricas de runtime reutilizáveis por diferentes aplicações com IA. Eles não são cópias 1:1 nem issues públicas prontas para publicação.

Uma engine compartilhada permite que um assistente de engenharia, um bot de GitHub, um assistente pessoal com ferramentas de computador ou um construtor de aplicações usem o mesmo núcleo de execução. Cada produto fornece sua identidade, interface, catálogo de ferramentas, integrações, política confiável e configuração de deployment; o runtime comum implementa contratos genéricos e mantém o ciclo de execução consistente. Vectora App e Vectora GH Bot são consumers independentes: o Bot pode usar microserviços de Vectora Services pelos adapters do próprio produto sem depender da UI ou do processo do app Vectora.

## Implementação e distribuição

Rust é a única implementação da engine e define a API canônica. A distribuição
prevista tem três entradas: crate Rust no crates.io; binding Python no PyPI,
usado pelo backend Python do Vectora; e binding TypeScript/Node no npm, usado
pelo Vectora GH Bot. PyPI e npm não terão implementações paralelas: os bindings
encaminham chamadas ao mesmo núcleo Rust e compartilham schemas, semântica de
erros, cancelamento e testes de conformidade. Cada binding só expõe capacidades
suportadas pelo contrato Rust versionado.

| Documento da engine | Referência local | Capacidades reutilizáveis |
|---|---|---|
| [01 — Runtime agêntico e harness](01-runtime-agentico-e-harness.md) | `../../issues/01-loop-agentico-e-harness.md` | Execuções, loop, tools, políticas, aprovações, subagentes, verificação e decisões tipadas. |
| [02 — Storage, memória, contexto e recuperação](02-storage-memoria-contexto-rag.md) | `../../issues/02-storage-memoria-contexto-rag.md` | Contratos SQL, vetoriais, KV, filas/eventos, ledger, contexto e RAG. |
| [04 — Conversas, streaming e threads](04-conversas-streaming-e-threads.md) | `../../issues/04-chat-streaming-e-threads.md` | Turnos, envelopes de evento, progresso, replay, cancelamento e adaptação a diferentes canais. |

## Regras de leitura

- A issue de referência descreve requisitos e observações do Vectora; a especificação aqui define o corte apropriado à engine.
- Requisitos de interface, UX, apresentação, layout e notificações de produto ficam fora da engine, salvo o contrato de eventos necessário para qualquer consumidor.
- Integrações concretas e identidade/autorização são fornecidas por cada consumidor por interfaces tipadas. O runtime não confia em identidade, permissões ou escopo declarados dentro de conteúdo não confiável do modelo.
- APIs públicas só são introduzidas após identificação de consumidores, contrato de dados e erros, cancelamento, timeout, ownership, observabilidade, compatibilidade e evidência de conformidade.
- Mudanças no núcleo Rust devem manter os bindings Python e TypeScript coerentes; publicar um binding exige o artefato Rust correspondente e conformidade aprovada.
- As specs são propostas de capacidade; maturidade e disponibilidade devem ser marcadas como implementada, experimental, planejada ou indisponível. Não presumem paridade nem melhoria de desempenho.

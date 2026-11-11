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

| Documento da engine                                                                 | Referência local                                                                                                        | Capacidades reutilizáveis                                                                                  |
| ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| [01 — Runtime agêntico e harness](01-runtime-agentico-e-harness.md)                 | `../../issues/01-loop-agentico-e-harness.md`                                                                            | Execuções, loop, tools, políticas, aprovações, subagentes, verificação e decisões tipadas.                 |
| [02 — Storage, memória, contexto e recuperação](02-storage-memoria-contexto-rag.md) | `../../issues/02-storage-memoria-contexto-rag.md`                                                                       | Contratos SQL, vetoriais, KV, filas/eventos, ledger, contexto e RAG.                                       |
| [03 — Contrato de chat OpenAI-compatible](03-chat-openai-compatible.md)             | `../plans/01-plano-portabilidade-llm-rust.md` e o backend Python consumidor                                             | Contrato versionado de chat, transporte inicial, streaming SSE, erros e conformidade.                      |
| [04 — Conversas, streaming e threads](04-conversas-streaming-e-threads.md)          | `../../issues/04-chat-streaming-e-threads.md`                                                                           | Turnos, envelopes de evento, progresso, replay, cancelamento e adaptação a diferentes canais.              |
| [05 — Providers nativos de chat](05-native-chat-providers.md)                       | Backend Python de referência (`backend/llm/anthropic/`, `google/` e `cohere/`)                                          | Anthropic Messages, Google Gemini e Cohere Chat v2, conversão multimodal/tool, streaming e usage.          |
| [06 — Fallback de chat](06-chat-fallback.md)                                        | Backend Python consumidor (`backend/llm/fallback_chat_client.py`)                                                       | Seleção explícita e ordenada de providers, erros elegíveis e mudança segura durante streaming.             |
| [07 — Embeddings e rerank](07-embeddings-rerank.md)                                 | Backend Python consumidor (`backend/llm/*/embeddings.py`, `*/*rerank.py`)                                               | Embeddings, rerank, ordenação, dimensões, scores e fallback explícito por quota.                           |
| [08 — Ciclo de vida de execução](08-run-lifecycle.md)                               | `../../issues/01-loop-agentico-e-harness.md`                                                                            | Identidade versionada da execução e máquina de estados experimental, exposta em Rust, Python e TypeScript. |
| [09 — Montagem de turno](09-turn-assembly.md)                                       | Backend consumidor (`backend/engine/conversation_loop.py`)                                                              | Acumulação Rust de eventos streaming em resposta canônica, com os mesmos bindings do bootstrap.            |
| [10 — Contratos de tools e dispatch](10-tool-contracts-e-dispatch.md)               | `../../issues/01-loop-agentico-e-harness.md` e `backend/tools/registry.py`                                              | Schemas validados, capabilities default-deny, decisões de invocação e modo de execução do lote.            |
| [11 — Rodadas de modelo e resultados de tools](11-model-tool-loop.md)               | `../../issues/01-loop-agentico-e-harness.md` e `backend/engine/conversation_loop.py`                                    | Coordenação versionada entre respostas do modelo, adapters consumidores e resultados.                      |
| [12 — Política e aprovação de tools](12-policy-and-approval.md)                     | `../../issues/01-loop-agentico-e-harness.md` e `backend/engine/guardrails.py`, `hitl.py`, `services/policy_compiler.py` | Snapshot genérico por run, decisões default-deny e aprovação vinculada à chamada e à policy.               |

## Regras de leitura

- A issue de referência descreve requisitos e observações do Vectora; a especificação aqui define o corte apropriado à engine.
- Requisitos de interface, UX, apresentação, layout e notificações de produto ficam fora da engine, salvo o contrato de eventos necessário para qualquer consumidor.
- Integrações concretas e identidade/autorização são fornecidas por cada consumidor por interfaces tipadas. O runtime não confia em identidade, permissões ou escopo declarados dentro de conteúdo não confiável do modelo.
- APIs públicas só são introduzidas após identificação de consumidores, contrato de dados e erros, cancelamento, timeout, ownership, observabilidade, compatibilidade e evidência de conformidade.
- Mudanças no núcleo Rust devem manter os bindings Python e TypeScript coerentes; publicar um binding exige o artefato Rust correspondente e conformidade aprovada.
- As specs são propostas de capacidade; maturidade e disponibilidade devem ser marcadas como implementada, experimental, planejada ou indisponível. Não presumem paridade nem melhoria de desempenho.

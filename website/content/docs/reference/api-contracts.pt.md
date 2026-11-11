---
title: Referência dos contratos de API
description: Resumo público dos contratos versionados do runtime.
weight: 2
---

Esta página resume os contratos compartilhados por Rust, Python e TypeScript/Node. É um guia de integração e navegação, não uma promessa de estabilidade. Os campos serializados exatos são definidos pelos tipos do código e pelos documentos de contrato vinculados.

## Requisições e eventos de chat

O chat compatível com OpenAI usa o contrato versão `1`. A requisição contém `contract_version`, `model`, `messages` e `options`, `tools` e `extensions` específicos do provider como campos opcionais. O consumidor fornece `base_url` e, opcionalmente, `api_key`, headers, timeouts, limite de tentativas e limites de tamanho de resposta/evento. A engine preserva prefixos de caminho e não carrega credenciais de arquivos de ambiente.

Os tipos de evento de streaming são `text_delta`, `reasoning_delta`, `tool_call_delta`, `model_switched`, `usage` e `completed`. Deltas de tool call são fragmentos indexados; o consumidor os acumula e valida os argumentos finais antes do dispatch. Fechar o stream cancela a requisição nativa.

Chat nativo usa `NativeChatSettings` com provider `anthropic`, `google` ou `cohere`; os campos específicos permanecem explícitos. Requisições de embedding e rerank têm versões próprias e retornam vetores ou índices ordenados dos documentos de origem. Lotes vazios não fazem chamada de rede.

## Coordenação de runs e tools

`RunIdentity` é um snapshot versionado de identidade/escopo/capabilities fornecido pelo host. As funções de lifecycle validam transições, mas não autenticam a identidade. `ToolContract` define schema, capability, classe de efeito, timeout e declaração de idempotência. O dispatch retorna decisões ordenadas; não invoca adapters.

`ModelToolLoopState` coordena turnos de modelo e resultados correlacionados das tools. O contrato de estado atual é versão `2`; resultados de tool usam versão `1`. Cancelamento pode retornar chamadas pendentes para o host interromper. Recovery e resultados externos desconhecidos exigem reconciliação pelo host; isso não implica replay de adapters nem garantia exactly-once.

`PolicySnapshot` e grants de aprovação vinculam uma decisão ao run, turno, chamada, argumentos normalizados, escopo, revisão e digest da policy. O host autentica aprovadores e valida a origem confiável. Reports de verificação distinguem `passed`, `failed`, `infra_error` e `not_run`; checks obrigatórios e evidências determinam o veredito do gate.

## Stores duráveis

Operações dos stores SQLite recebem caminho do banco e escopo autorizado pelo host. Mutações duráveis usam `operation_id` para replay idempotente e rejeitam snapshots obsoletos ou conflitantes. Gravações de evento/checkpoint são atômicas somente dentro da mesma transação SQLite. Há stores especializados para loop modelo/tool, lifecycle/verificação de run e policy/aprovação.

O host precisa redigir valores sensíveis antes da persistência, controlar permissões e retenção de arquivos e reconciliar efeitos externos após resultados incertos. Os stores não autenticam o chamador nem incluem efeitos de provider/tool numa transação do banco.

## Nomes nos bindings

| Capability | Python | TypeScript |
| --- | --- | --- |
| Chat | `chat_complete`, `chat_stream` | `chatComplete`, `chatStream` |
| Chat nativo | `native_chat_complete`, `native_chat_stream` | `nativeChatComplete`, `nativeChatStream` |
| Montagem de turno | `assemble_turn` | `assembleTurn` |
| Dispatch de tools | `prepare_tool_dispatch`, `authorize_tool_dispatch` | `prepareToolDispatch`, `authorizeToolDispatch` |
| Loop modelo/tool | `start_model_tool_loop`, `accept_model_turn`, `submit_tool_result`, `recover_model_tool_loop`, `cancel_model_tool_loop`, `reconcile_tool_result` | `startModelToolLoop`, `acceptModelTurn`, `submitToolResult`, `recoverModelToolLoop`, `cancelModelToolLoop`, `reconcileToolResult` |
| Verificação | `evaluate_verification` | `evaluateVerification` |
| SQLite durável | funções `sqlite_*` | funções `sqlite*` |
| Vetores | `vector_store_upsert`, `vector_store_search`, `vector_store_delete` | `vectorStoreUpsert`, `vectorStoreSearch`, `vectorStoreDelete` |
| KV | `kv_store_put`, `kv_store_get` | `kvStorePut`, `kvStoreGet` |
| Fila | `queue_enqueue`, `queue_receive`, `queue_ack`, `queue_release` | `queueEnqueue`, `queueReceive`, `queueAck`, `queueRelease` |
| Pub/sub | `pubsub_publish`, `pubsub_subscribe`, `pubsub_receive`, `pubsub_unsubscribe` | `pubsubPublish`, `pubsubSubscribe`, `pubsubReceive`, `pubsubUnsubscribe` |
| Ledger com projeção resiliente | `resilient_ledger_append` | `resilientLedgerAppend` |

Consumidores Rust usam a API tipada canônica nos módulos do crate. Essas superfícies de linguagem não prometem expor todos os tipos internos Rust. PostgreSQL é exposto atualmente pela operação de ledger local-first resiliente; a conexão direta ao ledger PostgreSQL permanece apenas em Rust. Para schemas completos, códigos de erro e regras de versão, consulte as [especificações de contrato no repositório](https://github.com/vectora-ltda/engine/tree/main/docs/specs) e os READMEs dos bindings.

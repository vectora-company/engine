# Checklist completo do estado da Engine

**Auditoria documental e de código:** 17 de novembro de 2026, checkout da área de trabalho. O inventário cobre 60 Markdown em `docs/`, documentação de raiz, implementação e testes disponíveis. Issues abertas são consultadas no GitHub como fontes de requisitos, não como estrutura deste checklist. As issues abertas atuais são #2, #20 e #58; seus corpos foram comparados com os contratos/código e há requisitos desatualizados em #58. Este estado descreve o checkout auditado, que pode conter capacidades ainda não integradas à branch principal.

O status não representa aprovação de release, adoção por produto, paridade total com SDKs/serviços ou garantia de produção. Propostas sem aprovação permanecem sugestões.

## Legenda

| Símbolo | Status                        | Interpretação                                                                                                          |
| ------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| ☑️      | Implementado                  | A capacidade está implementada no checkout auditado para o escopo descrito.                                            |
| 🟨      | Parcialmente implementado     | Há uma parte funcional, mas faltam operações, paridade ou critérios declarados.                                        |
| 🔍      | Implementado; precisa revisão | O código existe, porém documentação contraditória, integração real, cobertura ou limites impedem fechar a verificação. |
| ☐       | Não implementado              | Não há implementação identificada no checkout, ou a proposta ainda não foi aprovada.                                   |

## 1. Núcleo, API e harness

| Status | Capacidade / verificação                                                     | Estado e evidência                                                                                                                                                                      |
| ------ | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | Core canônico em Rust                                                        | O crate `vectora-engine` reúne chat, LLM, storage e harness; bindings encaminham ao código nativo.                                                                                      |
| ☑️     | Erros tipados, validação e configuração por capability                       | Presentes nos módulos públicos; configurações de produto e segredos permanecem responsabilidade do host.                                                                                |
| ☑️     | Lifecycle de run                                                             | Estados e transições do run expostos pelo harness e persistidos em SQLite.                                                                                                              |
| ☑️     | Montagem de turn a partir de eventos                                         | Acumula deltas, tool calls e término em resultado comum.                                                                                                                                |
| ☑️     | Contrato de tools e preparação de dispatch                                   | Valida identificadores/schema, capabilities default-deny, efeitos e decisões de despacho. O host executa a tool.                                                                        |
| ☑️     | Loop de modelo e tools                                                       | Coordena iterações, chamadas/resultados, limites e estados; não executa processos ou tools por conta própria.                                                                           |
| ☑️     | Policy snapshot e aprovação                                                  | Decisões default-deny e grants vinculados à chamada/policy; SQLite persiste os estados e rejeita reutilização inconsistente de digest e estado acima de 512 KiB.                        |
| ☑️     | Gates e evidências de verificação                                            | Relatórios estruturados e associação a run; a Engine não prova que uma alegação externa é verdadeira.                                                                                   |
| ☑️     | Stores SQLite duráveis de lifecycle, loop e policy                           | Implementados com schemas versionados; `submit_tool_result` idempotente, `recover` sem replay e reconciliação sobrevivem a reopen. Efeitos externos ainda exigem reconciliação do host. |
| 🟨     | Resume/recovery ponta a ponta de run                                         | Persistência e APIs existem, mas retomada de efeitos externos exige reconciliação explícita pelo host; não há exactly-once.                                                             |
| 🟨     | Extensibilidade uniforme de tools nativas, runtime e externas                | Há contratos de tool/dispatch; registro dinâmico completo, isolamento e adapters externos uniformes não estão fechados.                                                                 |
| ☐      | Execução de tools isolada/sandbox, RBAC e identidade de aprovador            | Permanecem no consumidor/infraestrutura e não são fornecidos pelo núcleo.                                                                                                               |
| ☐      | Subagents, scheduler geral de workflows e Hooks/Skills como runtime completo | Não encontrados como capacidades gerais implementadas no checkout auditado.                                                                                                             |
| ☐      | Garantia de correção de resultado por verificação automática                 | Gate tipado existe; a semântica e a evidência confiável da verificação pertencem ao host.                                                                                               |

## 2. Chat, providers e inferência

### Chat e protocolos

| Status | Capacidade / verificação                                                    | Estado e evidência                                                                                                                                                 |
| ------ | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ☑️     | Transporte OpenAI-compatible                                                | Completion, streaming SSE, erros, limites, timeout e cancelamento do stream em Rust.                                                                               |
| ☑️     | Perfis OpenAI Chat Completions e OpenAI Responses                           | Perfis separados existem no core e bindings; diferenças de formato não são achatadas numa promessa de equivalência total.                                          |
| ☑️     | OpenRouter Chat Completions                                                 | Perfil nomeado, endpoint/autenticação e fixtures locais.                                                                                                           |
| ☑️     | Anthropic Messages, Google Gemini e Cohere Chat v2                          | Adapters nativos com conversão de mensagens, tools, eventos de streaming e usage nos casos cobertos.                                                               |
| ☑️     | Ollama `/api/chat`                                                          | Perfil nativo, JSON/NDJSON, tools, imagens, `think`, `format`, `keep_alive`, `options` e autenticação opcional conforme contrato implementado.                     |
| 🔍     | llama-server v1.0.0: chat, Decision Model, embeddings, rerank, TTS/realtime | Endpoints implementados em Rust/bindings; fixtures cobrem contratos, mas smoke com servidor/modelo real não está demonstrado para todos os cenários.               |
| 🔍     | Paridade de streaming, erros e extensões em todos os providers              | Casos cobertos por fixtures, mas não existe prova de paridade integral com todos os SDKs, endpoints, modelos e gateways.                                           |
| 🟨     | 9Router, OmniRouter e Kimi                                                  | Podem usar endpoint OpenAI-compatible quando compatível; não há perfil nominal nem conformidade própria por rota/provedor.                                         |
| 🔍     | Descoberta de modalidades e limites por modelo                              | Discovery e overrides são suportados em parte; metadados do servidor não provam capacidade efetiva, e não há catálogo completo e autoritativo por modelo/endpoint. |
| ☑️     | Fallback de chat configurado pelo consumidor                                | Tenta candidatos elegíveis em ordem; evita replay após conteúdo parcial e expõe troca de modelo. Não escolhe credenciais/candidatos automaticamente.               |
| 🟨     | Fallback e compatibilidade multimodal em toda a matriz de providers         | Retry textual e classificação existem nos fluxos cobertos; catálogo completo e casos de erro de cada protocolo seguem incompletos.                                 |

### Decision Models, áudio, imagem e vídeo

| Status | Capacidade / verificação                                    | Estado e evidência                                                                                                                                                       |
| ------ | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ☑️     | Decision Model do llama-server `/v1/systemone`              | Choice/score/noul, validação, probabilidades e requests efêmeros com mídia; endpoint não é streaming.                                                                    |
| ☑️     | TTS bufferizado do llama-server                             | `/v1/audio/speech` e `/tts`, formatos de áudio cobertos, bytes e metadados preservados.                                                                                  |
| ☑️     | Streaming TTS                                               | Chunks binários, limites, backpressure e cancelamento nos bindings, conforme fixtures.                                                                                   |
| ☑️     | Sessão realtime de áudio/transcrição/conversa               | WebSocket `/v1/realtime`, eventos forward-compatible, envio/recepção e fechamento cancelável no cliente.                                                                 |
| 🔍     | Interoperabilidade real de TTS/realtime/Decision Model      | Implementação e fixtures existem; execução com servidor e modelos reais compatíveis não está comprovada de forma abrangente.                                             |
| ☐      | STT/transcrição batch geral                                 | Realtime não equivale a uma API batch; não há adapter batch geral identificado.                                                                                          |
| ☐      | Geração de imagem                                           | Nenhuma API de geração de imagem implementada no escopo atual.                                                                                                           |
| ☐      | Geração de vídeo                                            | Nenhuma API de geração de vídeo implementada no escopo atual.                                                                                                            |
| 🟨     | Vision, entrada de áudio e vídeo em chat                    | Tipos e serialização existem nos providers/casos compatíveis; suporte efetivo depende do endpoint/modelo e não há catálogo abrangente nem testes reais de toda a matriz. |
| ☐      | Outros provedores dedicados de fala, visão, imagem ou vídeo | Não assumir suporte nominal sem perfil, operação e teste próprios.                                                                                                       |

### Embeddings e reranking

| Status | Capacidade / verificação                                                        | Estado e evidência                                                                                           |
| ------ | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| ☑️     | Embeddings OpenAI-compatible, OpenRouter, Cohere, Voyage, Ollama e llama-server | Clientes no núcleo Rust; llama-server também oferece entrada multimodal via asset map.                       |
| ☑️     | Rerank Cohere, Voyage, OpenRouter e llama-server                                | Clientes no núcleo Rust; llama-server cobre texto e imagem efêmera com validação de resposta.                |
| ☑️     | Fallback explícito de embeddings/rerank                                         | Fallback opt-in nos casos de quota definidos; reexecuta a operação inteira, sem persistir resultado parcial. |
| 🔍     | Conformidade completa de erros, limites e modelos de embeddings/rerank          | Contratos e fixtures cobrem os casos declarados, não todas as versões/endpoints/modelos dos providers.       |

## 3. Mídia e requests

| Status | Capacidade / verificação                                       | Estado e evidência                                                                                                                                                                                                          |
| ------ | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | Partes de conteúdo tipadas                                     | Image, Audio, Video, Document e File aparecem no contrato da Engine.                                                                                                                                                        |
| ☑️     | Mapa temporário de assets nos bindings                         | Assets efêmeros são referenciados por ID/hash e encaminhados ao mesmo núcleo Rust.                                                                                                                                          |
| ☑️     | Request persistível separado do envio efêmero                  | Bytes inline e URLs `data:` não entram nos requests persistíveis; validação/hash/MIME/limite ocorrem no envio.                                                                                                              |
| ☑️     | Três modos de integrar assets                                  | Preparação manual, mídia efêmera por request e contexto padrão, disponíveis na superfície implementada.                                                                                                                     |
| ☑️     | Resultado de tool multimodal                                   | `ToolResult` usa partes tipadas; serialização de imagem coberta nos providers suportados e fallback textual conforme contrato.                                                                                              |
| ☑️     | Reclassificação de MIME por bytes                              | Bytes reidratados são validados antes de montar payload; PDF e previews seguem regras de assinatura/tipo.                                                                                                                   |
| ☑️     | Limite global e overrides por provider/modelo/modalidade       | Há teto global e configuração específica; limite global continua máximo absoluto.                                                                                                                                           |
| 🟨     | Catálogo automático de capabilities/limites multimodais        | Overrides e parte do discovery existem; limites oficiais por modelo/endpoint e de duração/resolução ainda não estão completos. Orçamento agregado opcional de bytes/assets cobre chat, Decision Model, embeddings e rerank. |
| 🔍     | Matriz de serialização de mídia por provider                   | Adapters cobrem modos selecionados; faltam fixtures comuns completas e catálogo confiável por operação/modelo. A issue #58 ainda descreve um contrato anterior.                                                             |
| 🟨     | Recuperação textual após rejeição de payload multimodal        | Retry textual limitado aos erros e fluxos classificados; não substitui conversão de PDF/vídeo nem processamento local de mídia.                                                                                             |
| ☐      | Extração/transcrição local de PDF/áudio/vídeo dentro da Engine | Não faz parte das APIs de geração/inferência atuais; não confundir com passagem de mídia a um provider.                                                                                                                     |

## 4. Storage, ledger e mensageria

### SQL, SQLite e PostgreSQL

| Status | Capacidade / verificação                              | Estado e evidência                                                                                                                                                                                   |
| ------ | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | Ledger SQLite local-first                             | SQLite mantém autoridade local para append, idempotência, conflito de versão e recuperação.                                                                                                          |
| ☑️     | Stores SQLite de lifecycle, loop e policy             | Persistência transacional local integrada ao ledger/projeção configurada.                                                                                                                            |
| ☑️     | Backup/restore SQLite                                 | Modos online e offline coordenado pelo host, verificação e restauração implementados nos bindings cobertos.                                                                                          |
| ☑️     | Projeção PostgreSQL retomável                         | Outbox SQLite-first projeta eventos, checkpoints, reservations/admissions; flush e contagem pendente disponíveis.                                                                                    |
| ☑️     | Verify/rebuild da projeção PostgreSQL                 | Reconstroi por scope a partir do SQLite e detecta divergências cobertas por testes. Backup do próprio PostgreSQL segue gerenciado externamente (`pg_dump` ou equivalente).                           |
| 🔍     | Concorrência de projetores PostgreSQL                 | Dois handles foram testados em rodadas service-real; não equivale à cobertura entre processos independentes e de todas as corridas admission/reservation/retry.                                      |
| 🟨     | Equivalência funcional SQLite/PostgreSQL              | PostgreSQL é projeção; não oferece stores genéricos equivalentes de lifecycle/loop/policy nem é autoridade de append. Manter essa assimetria explícita e fechar capacidades comuns ainda aplicáveis. |
| ☐      | Driver/transação genérica SQL                         | Não há uma API genérica SQL abrangente no código; migrations PostgreSQL com rollback também seguem pendentes nos planos.                                                                             |
| 🔍     | Recovery sob indisponibilidade durante verify/rebuild | Rebuild e inconsistências têm cobertura; interrupção remota durante reconstrução e procedimento operacional de consumidor requerem mais validação/documentação.                                      |

### Armazenamento vetorial

| Status | Capacidade / verificação                           | Estado e evidência                                                                                                                                               |
| ------ | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | LanceDB e Qdrant: upsert, busca e remoção          | Adapters implementados em Rust e encaminhados pelos bindings.                                                                                                    |
| ☑️     | Base comum vetorial                                | Escopo/coleção, IDs, validação de dimensão, limites de busca e métrica cosine/similaridade normalizada cobertos em ambos.                                        |
| 🔍     | Integração e falhas/restart nos serviços vetoriais | Testes locais/service-real cobrem casos declarados; matrizes completas de falha, indexação e versões não estão fechadas.                                         |
| ☑️     | Filtros de metadados equivalentes                  | Checkout inclui AST validada e execução antes do top-k em LanceDB/Qdrant; contrato vetorial 3 exige recriação de collections antigas.                            |
| ☐      | Busca híbrida/lexical equivalente                  | Não implementada como capability comum.                                                                                                                          |
| ☐      | Configuração e equivalência de índices ANN         | Não implementada como superfície comum comprovada.                                                                                                               |
| ☐      | Quantização equivalente                            | Não implementada como superfície comum comprovada.                                                                                                               |
| ☐      | Memória, contexto e RAG integrados                 | Vector store e inferência de embeddings/rerank existem; orquestração de ingestão, memória, recuperação e contexto não está implementada como stack RAG completa. |

### KV, filas e pub/sub

| Status | Capacidade / verificação                                                                       | Estado e evidência                                                                                                                                                           |
| ------ | ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | KV Redis e NATS JetStream                                                                      | Get/put, TTL, listagem por prefixo, remoção, revisão/CAS, incremento e watch implementados para os pares cobertos.                                                           |
| 🔍     | TTL equivalente Redis/NATS                                                                     | TTL por chave existe nos dois; NATS exige Server 2.11+. Arredondamento/expiração e versão mínima precisam continuar visíveis e testados em serviço compatível.               |
| ☑️     | Filas duráveis Redis Streams e NATS JetStream                                                  | Enqueue/receive/ack/release, visibilidade/redelivery e limites de retenção configuráveis.                                                                                    |
| ☑️     | Tentativas limitadas e dead-letter queue                                                       | Limite de tentativas e DLQ consumível implementados nos dois, com semântica de entrega descrita como at-least-once onde aplicável.                                           |
| 🟨     | Equivalência de semântica de filas                                                             | Não há exactly-once comum; diferenças de ACK, redelivery, retenção, DLQ e deduplicação precisam ser declaradas. Retenção pode remover mensagens antigas/pendentes no limite. |
| ☐      | Contrato comum completo de atraso/NACK, ordenação, replay, shutdown gracioso e redrive atômico | Permanecem gaps listados no plano/checklist de storage.                                                                                                                      |
| ☑️     | Pub/sub efêmero Redis Pub/Sub e NATS Core                                                      | Publish/subscribe básico; não há promessa de persistência/replay.                                                                                                            |
| 🟨     | Pub/sub: padrões, reconexão e eventos de estado                                                | Semânticas de assinatura e recuperação não atingem o contrato completo previsto nos planos para ambos.                                                                       |
| 🔍     | Matriz completa de testes service-real                                                         | Existem testes condicionais para serviços reais; skips por URL ausente não são evidência de execução. Repetir a matriz com versões corretas, inclusive NATS 2.11+.           |

### Grafos

| Status | Capacidade / verificação                            | Estado e evidência                                                                                             |
| ------ | --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| ☑️     | FalkorDB como capability graph independente         | Seleção, conexão, operações portáveis de nós/relações/traversal e bindings Rust/Python/TypeScript.             |
| ☑️     | Extensão Cypher parametrizada                       | Camada específica do FalkorDB com parâmetros; não é normalizada para os demais stores.                         |
| ☑️     | Limites/timeouts e TLS Rustls                       | Configuração compartilhada nos bindings; testes reais descritos para FalkorDB 6.0.3 e CA confiável.            |
| 🔍     | Cancelamento de query remota FalkorDB               | Client oficial usado não expõe cancelamento remoto; timeout server-side não equivale a cancelar processamento. |
| ☐      | GraphRAG e integração de grafo com memória/contexto | Fora da implementação atual; Graph store não implica GraphRAG.                                                 |

## 5. Bindings, conformidade, distribuição e adoção

| Status | Capacidade / verificação                                             | Estado e evidência                                                                                                                                                                           |
| ------ | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | Binding Python                                                       | Wrapper Rust/PyO3 e testes presentes; API segue o núcleo Rust.                                                                                                                               |
| ☑️     | Binding TypeScript/Node                                              | Wrapper N-API, pacote, tipos, builds e testes presentes; API segue o núcleo Rust.                                                                                                            |
| 🟨     | Conformidade Rust ↔ Python ↔ TypeScript completa                     | Há fixtures compartilhadas e testes por binding, mas não há paridade integral de todas as capabilities, erros, cancelamentos e serviços.                                                     |
| 🔍     | Instalação/publicação do pacote npm                                  | Issue #2 segue aberta. Artefatos/build e API existem; publicação oficial/versionada, matriz completa de plataformas, smoke install e todos os critérios da issue não estão comprovados aqui. |
| ☐      | Binding Go                                                           | Issue #20 é proposta aberta que exige aprovação e decisão ABI C/cgo versus IPC. Não implementar nem declarar suporte aprovado.                                                               |
| ☑️     | Build e manutenção local via SCons                                   | Targets de build/test/lint/update existem; modo simulado e atualização real dos manifests foram exercitados no fluxo local.                                                                  |
| 🔍     | Build limpo, instalação e publicação reproduzíveis em todos os alvos | CI/builds dos bindings existem; cobertura de plataforma, pacote instalado fora do checkout e cadeia de publicação verificável precisam ser consolidadas.                                     |
| 🔍     | Adoção pelo Vectora e GH Bot                                         | Ambos são consumidores previstos; a implementação da Engine não prova integração/adopção em produção. Revisar os repositórios consumidores separadamente antes de anunciar.                  |
| ☐      | Release 1.0 completa da Engine                                       | Não há evidência de release estável/crates.io + PyPI + npm, gates finais, matriz de suporte e checklist de adoção completos. A proposta de escopo está no plano de continuidade.             |
| 🔍     | Segurança operacional do runtime                                     | Segredos/redaction e contratos estão contemplados; sandbox, RBAC, identidade e autorização de produto não são oferecidos pelo core e exigem validação no host.                               |

## 6. Issues abertas e seu estado face ao código

| Issue                                                                                                                 | Estado remoto | Status neste inventário                    | Resumo                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------------------------------------------------------- | ------------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [#2 — Publicar binding TypeScript da Engine Rust como pacote npm](https://github.com/vectora-company/engine/issues/2) | OPEN          | 🟨 Parcialmente implementado               | Binding, pacote e testes existem. Publicação oficial, compatibilidade/versionamento, matriz de plataformas, instalação reproduzível e conformidade integral ainda não estão demonstrados como concluídos.                                 |
| [#20 — Planejar binding Go para a Engine Rust](https://github.com/vectora-company/engine/issues/20)                   | OPEN          | ☐ Não implementado / proposta não aprovada | A issue explicitamente requer aprovação e estudo comparativo ABI C/cgo versus processo/IPC. Não é capacidade comprometida nem existente.                                                                                                  |
| [#58 — Contrato de mídia no chat](https://github.com/vectora-company/engine/issues/58)                                | OPEN          | 🟨 Parcialmente implementado               | Tipos, assets, request efêmero/persistível e serialização foram implementados depois do estado antigo descrito no corpo. Ainda há gaps de matriz/catálogo e de conformidade compartilhada; revisar e atualizar a issue antes de fechá-la. |

## 7. Inventário documental analisado

Foram inventariados os 60 Markdown sob `docs/` e os seis Markdown da raiz;
estrutura, links, estados e critérios foram confrontados com o checkout e os
testes pertinentes. Títulos, corpos e estados das três issues abertas também
foram comparados com código e specs. Esta auditoria documental continua em
andamento para os documentos históricos remanescentes. Inventário atual por
grupo:

- **Índices/arquitetura (2):** `docs/README.md`, `docs/architecture.md`.
- **Specs (21):** `docs/specs/README.md` e os contratos 01–20.
- **Planos (9):** `docs/plans/00-prompt-loop-goal.md` até `docs/plans/08-plano-engine-cli.md`.
- **Checklists (9):** `docs/checklists/00-checklist-de-implementacao.md` até `docs/checklists/08-engine-cli.md`.
- **Análises (9):** `docs/analyses/00-analise-e-referencias-de-desenvolvimento.md` até `docs/analyses/08-auditoria-engine-cli.md`.
- **Auditorias (1):** `docs/audits/01-fotografia-historica-do-bootstrap.md` (fotografia histórica do bootstrap).
- **Goal loop (5):** `docs/goal-loop/README.md`, `docs/goal-loop/01-plano-operacional.md` até `docs/goal-loop/04-criterios-de-encerramento.md`.
- **Templates (4):** `docs/templates/README.md` e `docs/templates/00-plano-de-capability.md` até `docs/templates/02-checklist-de-etapa.md`.
- **Raiz (6):** `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `AGENTS.md`, `CLAUDE.md`.

## 8. Estado da revisão documental

Esta revisão organiza o estado por capability, consumidor, contrato e evidência. Os registros de entrega permanecem no histórico do GitHub e não definem a hierarquia da documentação. Foram atualizados nesta etapa:

- ☑️ `docs/plans/01-plano-portabilidade-llm-rust.md` e `docs/plans/02-plano-loop-agentico.md`: fases e estado organizados por capability.
- ☑️ `docs/plans/03-plano-continuidade.md`: status de providers, filtros vetoriais, mídia e FalkorDB reconciliado com o checkout.
- 🔍 `docs/plans/04-plano-storage-ledger.md`, `docs/checklists/00-checklist-de-implementacao.md`, `docs/checklists/03-auditoria-storage-s0-s5.md`, `docs/analyses/02-matriz-conformidade-capabilities.md` e `docs/analyses/05-auditoria-storage-s0-s5.md`: capacidades recentes estão refletidas; permanecem gaps reais de SQL, shutdown de filas e reconexão pub/sub, além de ANN/quantização/busca híbrida.
- ☑️ Plano/spec/checklist/análise de providers e modalidades foram reestruturados por protocolo e operação; gaps de serviço real e modalidades ausentes estão explícitos.
- ☑️ Plano/spec/checklist/análise de FalkorDB foram reestruturados por contrato, com configuração de teste e limitação de cancelamento remoto.
- ☑️ Checklist/spec de mídia agora refletem requests persistíveis/efêmeros, partes de tools e adapter Responses; catálogo e limites de duração/resolução seguem pendentes.
- 🔍 Revisar os registros históricos em `docs/checklists/01-cliente-openai-compatible.md`, `docs/checklists/02-matriz-conformidade-capabilities.md`, `docs/analyses/01-auditoria-cliente-openai-compatible.md`, `docs/analyses/03-analise-cruzada-akita-engine-vectora-gh-bot.md`, `docs/analyses/04-sabatina-engine-ecossistema.md` e `docs/audits/01-fotografia-historica-do-bootstrap.md`; preservar datas e bases auditadas e deixar explícito quando não descrevem o estado atual.
- ☑️ Validar os links relativos do inventário de 60 Markdown, incluindo os nomes de documentos reorganizados.

## 9. Prioridades abertas derivadas do inventário

1. Concluir a revisão dos registros históricos remanescentes e manter explícitos seus limites temporais.
2. Completar critérios remanescentes de SQL e storage: migrations PostgreSQL, busca híbrida, ANN/quantização, limites de fila/DLQ e reconexão pub/sub.
3. Fechar catálogo e conformidade de mídia/provider por operação; validar integrações reais de llama-server/Ollama/Laya e limites por modelo.
4. Determinar o escopo da primeira release completa e gates de distribuição, instalação limpa, plataformas, segurança, compatibilidade e adoção reversível.
5. Resolver issue #2 com evidência de publicação e conformidade; manter issue #20 como proposta até aprovação; reavaliar e atualizar issue #58 face à implementação real.

Este checklist não autoriza publicar release, afirmar produção, aprovar PRs ou
implementar a proposta Go. Os gaps que dependem de decisão de produto devem ser
apresentados a `brunosrz` pelo chat.

# Checklist completo do estado da Engine

**Fotografia:** 15 de novembro de 2026. **Branch:** `main`.
**SHA conferido:** `bbfdfede188a71e87c8235bdd89e5099ac677cda` (merge do PR #112).
**PRs:** 100 registros consultados; todos merged. Nenhum PR aberto na consulta.
**Issues:** 3 abertas (#2, #20 e #58); títulos, corpos e estado foram lidos.

Este inventário cruza documentação versionada, código existente e os requisitos
das issues abertas. O status descreve o que está no checkout e a evidência
registrada; não representa aprovação de release, adoção por produto, paridade
com SDKs/serviços ou garantia de produção. Itens de uma proposta sem aprovação
continuam sendo backlog, não compromisso aprovado.

## Legenda

| Símbolo | Status                        | Interpretação                                                                                                          |
| ------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| ☑️      | Implementado                  | A capacidade está no `main` para o escopo descrito.                                                                    |
| 🟨      | Parcialmente implementado     | Há uma parte funcional, mas faltam operações, paridade ou critérios declarados.                                        |
| 🔍      | Implementado; precisa revisão | O código existe, porém documentação contraditória, integração real, cobertura ou limites impedem fechar a verificação. |
| ☐       | Não implementado              | Não há implementação identificada no `main`, ou a proposta ainda não foi aprovada.                                     |

## 1. Núcleo, API e harness

| Status | Capacidade / verificação                                                     | Estado e evidência                                                                                                          |
| ------ | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| ☑️     | Core canônico em Rust                                                        | O crate `vectora-engine` reúne chat, LLM, storage e harness; bindings encaminham ao código nativo.                          |
| ☑️     | Erros tipados, validação e configuração por capability                       | Presentes nos módulos públicos; configurações de produto e segredos permanecem responsabilidade do host.                    |
| ☑️     | Lifecycle de run                                                             | Estados e transições do run expostos pelo harness e persistidos em SQLite.                                                  |
| ☑️     | Montagem de turn a partir de eventos                                         | Acumula deltas, tool calls e término em resultado comum.                                                                    |
| ☑️     | Contrato de tools e preparação de dispatch                                   | Valida identificadores/schema, capabilities default-deny, efeitos e decisões de despacho. O host executa a tool.            |
| ☑️     | Loop de modelo e tools                                                       | Coordena iterações, chamadas/resultados, limites e estados; não executa processos ou tools por conta própria.               |
| ☑️     | Policy snapshot e aprovação                                                  | Decisões default-deny e grants vinculados à chamada/policy; persistência local SQLite.                                      |
| ☑️     | Gates e evidências de verificação                                            | Relatórios estruturados e associação a run; a Engine não prova que uma alegação externa é verdadeira.                       |
| ☑️     | Stores SQLite duráveis de lifecycle, loop e policy                           | Implementados com schemas versionados, recuperação e testes; o host fornece identidade e autorização confiáveis.            |
| 🟨     | Resume/recovery ponta a ponta de run                                         | Persistência e APIs existem, mas retomada de efeitos externos exige reconciliação explícita pelo host; não há exactly-once. |
| 🟨     | Extensibilidade uniforme de tools nativas, runtime e externas                | Há contratos de tool/dispatch; registro dinâmico completo, isolamento e adapters externos uniformes não estão fechados.     |
| ☐      | Execução de tools isolada/sandbox, RBAC e identidade de aprovador            | Permanecem no consumidor/infraestrutura e não são fornecidos pelo núcleo.                                                   |
| ☐      | Subagents, scheduler geral de workflows e Hooks/Skills como runtime completo | Não encontrados como capacidades gerais implementadas no código da `main`.                                                  |
| ☐      | Garantia de correção de resultado por verificação automática                 | Gate tipado existe; a semântica e a evidência confiável da verificação pertencem ao host.                                   |

## 2. Chat, providers e inferência

### Chat e protocolos

| Status | Capacidade / verificação                                            | Estado e evidência                                                                                                                                                 |
| ------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ☑️     | Transporte OpenAI-compatible                                        | Completion, streaming SSE, erros, limites, timeout e cancelamento do stream em Rust.                                                                               |
| ☑️     | Perfis OpenAI Chat Completions e OpenAI Responses                   | Perfis separados existem no core e bindings; diferenças de formato não são achatadas numa promessa de equivalência total.                                          |
| ☑️     | OpenRouter Chat Completions                                         | Perfil nomeado, endpoint/autenticação e fixtures locais.                                                                                                           |
| ☑️     | Anthropic Messages, Google Gemini e Cohere Chat v2                  | Adapters nativos com conversão de mensagens, tools, eventos de streaming e usage nos casos cobertos.                                                               |
| ☑️     | Ollama `/api/chat`                                                  | Perfil nativo, JSON/NDJSON, tools, imagens, `think`, `format`, `keep_alive`, `options` e autenticação opcional conforme contrato implementado.                     |
| 🔍     | llama-server v1.0.0: health, models, chat e streaming               | Perfil versionado com fixtures e bindings. Integração real contra servidor/modelo não está demonstrada para todos os cenários.                                     |
| 🔍     | Paridade de streaming, erros e extensões em todos os providers      | Casos cobertos por fixtures, mas não existe prova de paridade integral com todos os SDKs, endpoints, modelos e gateways.                                           |
| 🟨     | 9Router, OmniRouter e Kimi                                          | Podem usar endpoint OpenAI-compatible quando compatível; não há perfil nominal nem conformidade própria por rota/provedor.                                         |
| 🔍     | Descoberta de modalidades e limites por modelo                      | Discovery e overrides são suportados em parte; metadados do servidor não provam capacidade efetiva, e não há catálogo completo e autoritativo por modelo/endpoint. |
| ☑️     | Fallback de chat configurado pelo consumidor                        | Tenta candidatos elegíveis em ordem; evita replay após conteúdo parcial e expõe troca de modelo. Não escolhe credenciais/candidatos automaticamente.               |
| 🟨     | Fallback e compatibilidade multimodal em toda a matriz de providers | Retry textual e classificação existem nos fluxos cobertos; catálogo completo e casos de erro de cada protocolo seguem incompletos.                                 |

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

| Status | Capacidade / verificação                                               | Estado e evidência                                                                                           |
| ------ | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| ☑️     | Embeddings OpenAI-compatible, OpenRouter, Cohere, Voyage e Ollama      | Clientes no núcleo Rust, com validação de dimensão/ordem e usage quando fornecido.                           |
| ☑️     | Rerank Cohere, Voyage e OpenRouter                                     | Clientes no núcleo Rust, índices e scores mantidos no resultado.                                             |
| ☑️     | Fallback explícito de embeddings/rerank                                | Fallback opt-in nos casos de quota definidos; reexecuta a operação inteira, sem persistir resultado parcial. |
| 🔍     | Conformidade completa de erros, limites e modelos de embeddings/rerank | Contratos e fixtures cobrem os casos declarados, não todas as versões/endpoints/modelos dos providers.       |

## 3. Mídia e requests

| Status | Capacidade / verificação                                       | Estado e evidência                                                                                                                                     |
| ------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ☑️     | Partes de conteúdo tipadas                                     | Image, Audio, Video, Document e File aparecem no contrato da Engine.                                                                                   |
| ☑️     | Mapa temporário de assets nos bindings                         | Assets efêmeros são referenciados por ID/hash e encaminhados ao mesmo núcleo Rust.                                                                     |
| ☑️     | Request persistível separado do envio efêmero                  | Bytes inline e URLs `data:` não entram nos requests persistíveis; validação/hash/MIME/limite ocorrem no envio.                                         |
| ☑️     | Três modos de integrar assets                                  | Preparação manual, mídia efêmera por request e contexto padrão, disponíveis na superfície implementada.                                                |
| ☑️     | Resultado de tool multimodal                                   | `ToolResult` usa partes tipadas; serialização de imagem coberta nos providers suportados e fallback textual conforme contrato.                         |
| ☑️     | Reclassificação de MIME por bytes                              | Bytes reidratados são validados antes de montar payload; PDF e previews seguem regras de assinatura/tipo.                                              |
| ☑️     | Limite global e overrides por provider/modelo/modalidade       | Há teto global e configuração específica; limite global continua máximo absoluto.                                                                      |
| 🟨     | Catálogo automático de capabilities/limites multimodais        | Overrides e parte do discovery existem; limites oficiais por modelo/endpoint, limites agregados, duração/resolução/contagem total não estão completos. |
| 🔍     | Matriz de serialização de mídia por provider                   | Adapters cobrem modos selecionados; issue #58 permanece aberta e pede cobertura de conformidade Rust/Python/TypeScript e matriz completa.              |
| 🟨     | Recuperação textual após rejeição de payload multimodal        | Retry textual limitado aos erros e fluxos classificados; não substitui conversão de PDF/vídeo nem processamento local de mídia.                        |
| ☐      | Extração/transcrição local de PDF/áudio/vídeo dentro da Engine | Não faz parte das APIs de geração/inferência atuais; não confundir com passagem de mídia a um provider.                                                |

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
| ☐      | Filtros de metadados equivalentes                  | Gap registrado para LanceDB e Qdrant.                                                                                                                            |
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
| ☑️     | Build e manutenção local via SCons                                   | Targets de build/test/lint/update existem; simulação de update foi adicionada e executada conforme histórico da branch integrada.                                                            |
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

Foram inventariados e cruzados os 55 Markdown sob `docs/`, além de
`README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `AGENTS.md`, `CLAUDE.md` e
`CHANGELOG.md`: estrutura, estados declarados, critérios/checklists, gaps,
links e referências foram confrontados com o código no SHA acima. Os títulos,
corpos e estados das 3 issues abertas também foram comparados com o código e
as specs. Documentos de contrato, plano e auditoria foram usados segundo sua
função; uma fotografia antiga não prevalece sobre implementação mais recente.
Os arquivos ficam listados por grupo para facilitar nova auditoria:

- **Índices/arquitetura (2):** `docs/README.md`, `docs/architecture.md`.
- **Specs (20):** `docs/specs/README.md` e `docs/specs/01-runtime-agentico-e-harness.md` até `docs/specs/19-storage-de-grafos-falkordb.md` (inclui numericamente os contratos 01–19).
- **Planos (8):** `docs/plans/00-prompt-loop-goal.md` até `docs/plans/07-plano-storage-grafos-falkordb.md`.
- **Checklists anteriores (7):** `docs/checklists/00-checklist-de-implementacao.md` até `docs/checklists/06-storage-grafos-falkordb.md`.
- **Análises (8):** `docs/analyses/00-analise-e-referencias-de-desenvolvimento.md` até `docs/analyses/07-auditoria-storage-grafos-falkordb.md`.
- **Auditorias/walkthroughs (2):** `docs/audits/01-auditoria-e-walkthrough-da-main.md`, `docs/audits/02-walkthrough-historico-de-prs-engine.md`.
- **Goal loop (5):** `docs/goal-loop/README.md`, `docs/goal-loop/01-plano-operacional.md` até `docs/goal-loop/04-criterios-de-encerramento.md`.
- **Templates (4):** `docs/templates/README.md` e `docs/templates/00-plano-de-capability.md` até `docs/templates/02-checklist-de-etapa.md`.
- **Raiz (6):** `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `AGENTS.md`, `CLAUDE.md`.

## 8. Documentos cujo estado precisa ser reconciliado

As fontes documentais são extensas, mas várias ainda apresentam como atual uma
fotografia anterior ao merge da stack até #112. Tratar seus detalhes datados
como histórico e atualizar os arquivos em uma revisão documental própria:

- 🔍 `docs/README.md`, `docs/goal-loop/README.md`, `docs/plans/00-prompt-loop-goal.md`, `docs/plans/05-loop-de-desenvolvimento.md`: estado de PRs/branch ainda descreve #112 como aberto ou etapa corrente.
- 🔍 `docs/plans/03-plano-continuidade.md`: estado verificado e tabela ainda param na fotografia da stack anterior; reconciliar com providers, mídia e FalkorDB já integrados.
- 🔍 `docs/plans/04-plano-storage-ledger.md`, `docs/checklists/00-checklist-de-implementacao.md`, `docs/checklists/03-auditoria-storage-s0-s5.md`, `docs/analyses/02-matriz-conformidade-capabilities.md`, `docs/analyses/05-auditoria-storage-s0-s5.md`: itens agora implementados continuam marcados pendentes, e alguns itens de teste/SQL continuam gaps reais.
- 🔍 `docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`, `docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`, `docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`, `docs/analyses/06-auditoria-providers-modalidades.md`: sequência e status ainda descrevem PRs #97–#112 como stack ativa; atualizar cada subetapa sem apagar os gaps de integração real e operações ausentes.
- 🔍 `docs/plans/07-plano-storage-grafos-falkordb.md`, `docs/specs/19-storage-de-grafos-falkordb.md`, `docs/checklists/06-storage-grafos-falkordb.md`, `docs/analyses/07-auditoria-storage-grafos-falkordb.md`: marcar #112 integrado em main, mantendo documentada a ausência de cancelamento remoto.
- 🔍 `docs/checklists/04-midia-no-chat.md` e `docs/specs/17-midia-no-chat.md`: contêm declarações antigas de ausência de OpenAI Responses e status de PR; revalidar contra `main` e preservar as limitações de matriz multimodal.
- 🔍 `docs/checklists/01-review-pr3-openai-compatible.md`, `docs/checklists/02-matriz-conformidade-capabilities.md`, `docs/analyses/01-auditoria-review-pr3.md` e `docs/audits/01-auditoria-e-walkthrough-da-main.md`: manter como auditoria histórica, mas explicitar snapshot/escopo para não induzir uso como estado global atual.
- 🔍 `docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`, `docs/analyses/03-analise-cruzada-akita-engine-vectora-gh-bot.md`, `docs/analyses/04-sabatina-engine-ecossistema.md`, `docs/goal-loop/01-plano-operacional.md` até `docs/goal-loop/04-criterios-de-encerramento.md`: revisar links, branch ativa, próxima etapa e afirmações que envelheceram; não remover decisões aprovadas ou gaps sem evidência.

## 9. Prioridades abertas derivadas do inventário

1. Reconciliar os documentos indicados acima com `main` e com este checklist; atualizar os links/índices para este arquivo.
2. Completar critérios remanescentes da base SQL e storage equivalente: concorrência entre processos/recovery, migrations PostgreSQL, filtros/busca híbrida/índices vetoriais e semânticas de filas/pub-sub.
3. Fechar catálogo e conformidade de mídia/provider por operação; validar integrações reais de llama-server/Ollama e limites por modelo sem inferir suporte por marca.
4. Determinar o escopo da primeira release completa e gates de distribuição, instalação limpa, plataformas, segurança, compatibilidade e adoção reversível.
5. Resolver issue #2 com evidência de publicação e conformidade; manter issue #20 como proposta até aprovação; reavaliar e atualizar issue #58 face à implementação real.

Este checklist não autoriza publicar release, afirmar produção, aprovar PRs ou
implementar a proposta Go. Os gaps que dependem de decisão de produto devem ser
apresentados a `brunosrz` pelo chat.

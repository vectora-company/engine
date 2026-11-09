# Plano do próximo PR: primeira capacidade LLM em Rust

## Objetivo

Iniciar a migração de `../vectora/backend/llm/` para a Vectora Engine, fazendo a primeira chamada de chat e seu streaming serem executados em Rust. Rust é a implementação única e canônica. A engine será distribuída como crate Rust, binding Python no PyPI para o backend Python do Vectora e binding TypeScript/Node no npm para o Vectora GH Bot. As duas linguagens de aplicação chamam a mesma API Rust por bindings; não mantêm cópias da lógica de LLM.

Este PR abre a trilha de migração. Não tenta portar os cerca de 50 módulos de `backend/llm` de uma vez nem declara a pasta Python obsoleta.

## Referências locais

- Contratos e migração por capability: `../issues/08-contratos-backend-e-engine-rust.md`.
- Storage, memória, contexto e RAG: `../issues/02-storage-memoria-contexto-rag.md`.
- Regras deste repositório: `AGENTS.md`.
- Arquitetura da engine: `docs/architecture.md`.
- Implementação de referência: `../vectora/backend/llm/` e `../vectora/backend/vtypes/message.py`.

## Levantamento da implementação atual

A pasta Python tem responsabilidades distintas que precisam permanecer separadas na migração:

| Área                        | Implementação atual                                                                                      | Observações para o port                                                                                                                                                                                                                               |
| --------------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Contratos comuns            | `base.py`, `vtypes/message.py`, `tools/registry.py`                                                      | `VMessage`, blocos multimodais, tool calls/chunks, usage e os protocolos `ChatClient`/`Embeddings` são a fronteira principal com o loop e tools existentes.                                                                                           |
| Transporte/provider clients | `openai`, `anthropic`, `google`, `cohere`, `ollama`, `openrouter`                                        | Clientes assíncronos, status HTTP tipados, corpos JSON/bytes/multipart, SSE e NDJSON. Cada protocolo tem serialização própria.                                                                                                                        |
| Protocolo OpenAI compatível | `openrouter/client.py` reutilizado por `nine_router`, `llamacpp` e `dmr`; `ollama` tem protocolo próprio | Configuração de `base_url`, autenticação opcional, cabeçalhos, caminhos e diferenças de protocolo variam. OmniRouter e 9Router devem ser cobertos como endpoints compatíveis com OpenAI quando o contrato configurado confirmar essa compatibilidade. |
| Adaptação de chat           | `*/chat_client.py`                                                                                       | Conversão de histórico, imagens, schemas de tools, respostas, usage, razões de término e fragmentos de streaming. Não presumir equivalência entre providers.                                                                                          |
| Fallback                    | `fallback_chat_client.py`, `provider_fallback.py`                                                        | Fallback depende de configuração explícita, disponibilidade de credenciais, tipo de erro, estado multimodal e se algum chunk já foi entregue. Não pode repetir uma geração após iniciar o stream.                                                     |
| Outras capacidades          | embeddings, rerank, transcription/STT, imagem, TTS, vídeo, busca Ollama                                  | São superfícies separadas, algumas síncronas na API externa e outras assíncronas; ficam fora do primeiro corte de chat.                                                                                                                               |
| Decision Models             | `systemone/`                                                                                             | Harness, política, budget, privacidade, circuit breaker e providers próprios. Apesar de estar sob `backend/llm`, não é simplesmente um adaptador de chat e não entra neste PR.                                                                        |
| Integrações consumidoras    | chat/engine, RAG/context graph, tools, usage, CLI e handlers                                             | Consumidores importam classes Python concretas e dependem de callbacks/eventos. A integração deve começar por um adaptador compatível e preservar esses consumidores.                                                                                 |

O primeiro PR deve confirmar os contratos exatos dos modelos Python usados pelo caminho de chat antes de codificar os tipos Rust, incluindo campos opcionais e serialização canônica. A tabela acima é um inventário inicial, não uma declaração de equivalência já comprovada.

## Decisões de fronteira

- **Engine Rust:** tipos comuns de chat, trait de provider, transporte HTTP assíncrono, parsing e eventos de stream. Não lê `.env`, `settings.json`, banco de dados nem credenciais por conta própria.
- **Vectora Backend (Python):** seleciona provider/modelo, resolve e valida credenciais, aplica configurações do produto e constrói a requisição tipada. Passa segredos ao binding Python somente para a chamada solicitada; eles não entram em logs, eventos ou mensagens de erro.
- **Binding Python/PyPI:** converte os tipos aprovados e expõe streaming consumível em Python sem bloquear o event loop. Encerrar/descartar o iterador cancela a requisição Rust e libera socket e buffers. Não implementa transporte ou política de provider em Python.
- **Vectora GH Bot (TypeScript):** consome o pacote npm da engine diretamente, sem depender do Vectora App ou do backend Python. O Bot seleciona/configura a capacidade e credenciais segundo seu próprio contrato de produto.
- **Binding TypeScript/Node/npm:** expõe a mesma capacidade Rust por uma fronteira nativa Node, com tipos TypeScript, streaming assíncrono, cancelamento, erros e shutdown coerentes com o contrato canônico. Não implementa um segundo motor TypeScript.
- **Distribuição Rust:** crate `vectora-engine` no crates.io é a interface direta para consumidores Rust; PyPI/npm são fachadas sobre a implementação Rust, não forks da engine.
- **Loop atual:** segue sendo dono de tools, persistência, autorização, HITL, orçamento de produto, workspace e composição da conversa. A engine LLM não executa tools recebidas do modelo; apenas devolve tool calls tipadas.
- **Fallback:** permanece coordenado pelo consumidor Python nesta primeira etapa. O adapter Rust retorna erros estáveis e classificados; só o Python percorre a cadeia existente. Não habilitar fallback em dois lugares.
- **Compatibilidade:** o cliente Python atual continua como caminho de referência e fallback operacional. A integração nativa é opt-in, desligável e reversível por configuração de runtime.

## Escopo proposto do PR 1

Entregar uma fatia vertical estreita, mas utilizável:

1. Contrato versionado de chat: roles, mensagens, blocos de conteúdo, tool definitions/calls, parâmetros de geração, usage e eventos de streaming.
2. Tipos Rust com erros discriminados: configuração inválida, autenticação, rate limit/quota, provider/server, timeout, conexão, resposta inválida, protocolo incompatível e cancelamento.
3. Transporte HTTP assíncrono com connect/read/total timeout definidos, limites de resposta, encerramento explícito e política de retry restrita à conexão antes da entrega do primeiro evento.
4. Adaptador do protocolo Chat Completions compatível com OpenAI, escolhido como primeiro alvo por fornecer a menor fatia que cobre gateway e runtime local sem confundir isso com suporte universal a todos os providers.
5. Cobertura de conformidade usando respostas e streams sintéticos para: texto, tool calls fragmentadas, usage, erro HTTP, SSE malformado, timeout, disconnect e cancelamento pelo consumidor.
6. Bindings Python e TypeScript/Node compiláveis fazem parte do bootstrap da engine, não de fases futuras. Neste primeiro PR, cada binding expõe ao menos a versão do contrato e a capacidade de chat/stream implementada aqui, com chamadas encaminhadas ao mesmo núcleo Rust. Os consumidores mantêm somente sua camada de adaptação local.
7. Fixture de compatibilidade por endpoint/base URL para testar OpenAI-compatible. O PR 1 estabelece transporte genérico e configuração de prefixo; a matriz de perfis observados e suas diferenças verificáveis fica no PR 2. Ollama permanece no adaptador/protocolo Ollama próprio.
8. Harness de comparação local com respostas determinísticas e gravação de metadados (SHA, Rust/Python, configuração e duração), sem chamar serviços reais nem executar efeitos externos duplicados.

A estratégia de FFI/empacotamento deve ser fechada no bootstrap, incluindo PyO3/maturin para wheels Python e N-API/napi-rs para o pacote Node, com justificativa de suporte, manutenção, cancelamento assíncrono e matriz de artefatos. O CI deve compilar e testar crate e bindings desde o bootstrap; a publicação em crates.io, PyPI e npm permanece em workflow de release separado, versionado e baseado no mesmo SHA/tag, com credenciais e publicação condicionadas a release aprovada. O PR de capacidade não publica automaticamente nem inclui builds dos produtos consumidores.

## Fora do PR 1

- Remover ou substituir os clientes Python existentes.
- Migrar o loop de conversa, fallback, autenticação, settings ou telemetria de produto para Rust.
- Afirmar paridade geral ou ganho de performance sem medições comparáveis.
- Portar de uma vez Anthropic, Google, Cohere, Ollama e todos os formatos OpenRouter.
- Portar embeddings, rerank, STT, transcription, imagem, TTS, vídeo ou web search.
- Portar `systemone/` ou o seu harness/políticas para o provider genérico.
- Criar chamadas reais em CI que gastem quota ou dependam de contas externas.

## Sequência de PRs da migração

A stack de LLM depende do contrato Rust introduzido no PR 1. Os bindings e sua infraestrutura de build/conformidade fazem parte do bootstrap; cada capability nova deve atravessar as três entradas na mesma stack, sem recriar a FFI. Em paralelo como uma trilha da engine, haverá uma migração dedicada de storage baseada na issue local `02-storage-memoria-contexto-rag.md`. Essa issue foi escrita para o backend Python atual; ela é fonte de comportamento e requisitos, não uma especificação Rust pronta. Antes do primeiro PR de storage, sua intenção deve ser adaptada para um contrato da engine em `docs/`, mantendo a cópia original em `../issues/` intacta.

### Trilha de LLM

1. **PR 1 — contrato, bindings bootstrap e chat base:** API canônica, transporte, primeiro protocolo OpenAI-compatible, bindings Python/TypeScript compiláveis e conformidade comum nas três entradas. Não declarar adoção por produto ainda.
2. **PR 2 — completar o perfil OpenAI-compatible:** registrar diferenças observadas de endpoint, autenticação, headers, catálogo e erros para OpenRouter, 9Router/OmniRouter e backends locais; preservar reasoning e custo de usage, permitir extensões de payload sem sobrescrever campos canônicos e testar prefixos como `/v1` e `/engines/v1`. Rodar fixtures locais de completion/stream/erro pelas três entradas (Rust, Python e TypeScript). Isso confirma o contrato/configuração, sem alegar paridade com serviços remotos. Ollama permanece no seu protocolo próprio.
3. **PR 3 — providers com protocolos nativos:** Anthropic e Google, incluindo conversão de mensagens multimodais, tools, safety/configuração e seus formatos de stream, expostos pelos bindings existentes. A implementação fica em `docs/specs/05-native-chat-providers.md`; os bindings do bootstrap encaminham a nova capacidade sem duplicar a lógica.
4. **PR 4 — Cohere chat e normalização de compatibilidade:** serialização de histórico/tool plan, respostas e erros específicos, mantendo paridade de contrato nos bindings.
5. **PR 5 — fallback de chat:** migrar a política apenas após os erros tipados estarem estáveis; preservar ordem explícita, elegibilidade por credencial, limites de retry e a regra de não trocar provider depois de emitir chunks.
6. **PR 6 — embeddings e rerank:** contratos próprios, diferenças query/document, dimensões, ordenação, scores e fallback; cada consumidor mantém seu RAG e decide adoção pela binding correspondente.
7. **PR 7 — mídia e capacidades auxiliares:** STT/transcription, geração de imagem, TTS, vídeo e web search, agrupados por contrato/protocolo comum, não apenas por diretório.
8. **PRs posteriores — adoção por consumidor:** trocar gradualmente chamadas selecionadas no Vectora Backend/Python e no GH Bot/TypeScript para suas bindings, medir operação e rollback, e só então discutir desativar caminhos antigos.

### Trilha de storage da engine

O objetivo desta trilha é oferecer adapters Rust reutilizáveis para **SQLite, LanceDB, NATS JetStream, PostgreSQL, Qdrant e Redis**, selecionados por capabilities explícitas. Não é um port textual de `backend/storage/`: o Vectora mantém settings, credenciais, RBAC, contas e regras de produto no backend. A engine assume os contratos genéricos de persistência que seus consumidores aprovarem.

| PR da trilha                           | Entrega planejada                                                                                                                                                                                                                                    | Fronteira e validação                                                                                                                                                                                                                |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **S0 — contrato Rust de storage**      | Criar a spec da engine baseada em `../issues/02-storage-memoria-contexto-rag.md`; inventariar consumidores e definir interfaces SQL, vector store, KV e fila, health/capabilities, timeout, retry, ciclo de vida, erros, isolamento e versionamento. | Documentar onde a engine termina e onde ficam dados de controle do produto. Matriz semântica compara backends; nenhum fallback para memória pode fingir durabilidade.                                                                |
| **S1 — SQL local/remoto**              | Implementar interfaces e adapters SQLite e PostgreSQL para os contratos duráveis aprovados: transação, query, migrações, pool, concorrência e recuperação.                                                                                           | Suíte comum + casos próprios de locks, JSON/datas, rollback, restart e migração. Não mover automaticamente tabelas de usuário/auth/settings do Vectora.                                                                              |
| **S2 — vector stores**                 | Implementar LanceDB local e Qdrant remoto: upsert/delete, busca vetorial e textual quando suportada, filtros, metadata, coleções e health.                                                                                                           | Comparar dimensão, métrica/direção do score, ordenação, filtros e consistência. Capacidades não suportadas retornam explicitamente; não simular equivalência.                                                                        |
| **S3 — KV, eventos e filas**           | Implementar Redis e NATS JetStream conforme os usos identificados: KV/TTL, pub-sub/invalidação, streams/consumidores ou fila durável e ACK.                                                                                                          | Contratos separados para Redis Streams e NATS JetStream: consumer/group, ACK/redelivery, retenção, replay, DLQ, limites, desconexão e shutdown. Não introduzir Celery por inferência.                                                |
| **S4 — bindings e adoção incremental** | Expor os mesmos contratos por Rust, Python/PyPI e TypeScript/npm. Integrar o Vectora Backend via Python e o GH Bot via TypeScript, cada um com seleção reversível e sua implementação de referência.                                                 | Sem dual-write nem execução duplicada de efeitos. Rodar a suíte comum contra a API Rust e ambas as bindings. Migração de dados precisa de contagens/hashes, divergências, rollback e origem de verdade definida antes de cada troca. |

S1–S3 podem ser empilhados sobre S0; dependências entre APIs de storage e consumidores devem ser explícitas. O objetivo é cobrir as seis tecnologias citadas, mas a ordem de adapters não implica que todos tenham a mesma garantia ou sejam obrigatórios em toda instalação. Os modos atuais do Vectora servem como evidência inicial — lite usa SQLite/LanceDB; complete usa PostgreSQL/Qdrant/Redis — e a participação de NATS precisa ser classificada por uso concreto (sidecar, KV, fila/eventos) antes de consolidar interfaces.

### Outras trilhas

- **SystemOne/Decision Models:** tratar contratos de decisão, privacidade, budgets, políticas e circuit breakers sem misturar com providers genéricos de chat.

## Plano de execução dentro do PR 1

1. **Caracterizar contratos.** Mapear `VMessage`, `VMessageChunk`, `ToolSpec`, usage, callbacks, headers e parâmetros; registrar campos obrigatórios, opcionais e invariantes. Extrair casos observáveis dos testes Python, sem copiar detalhes internos sem necessidade.
2. **Fechar matriz de compatibilidade.** Registrar método, rota, auth, headers, formato request/response, stream protocol, timeout, retry e classificação de erro por perfil OpenAI-compatible que será aceito no PR.
3. **Definir schema canônico e FFI.** Preferir structs versionados e explícitos a mapas sem tipo; garantir que chamadas/tool args continuem objetos JSON e que dados desconhecidos não desapareçam silenciosamente. Definir conversões, ownership, cancelamento e erros para Python e TypeScript junto com a implementação dos bindings mínimos.
4. **Implementar transporte e parsers.** Separar SSE framing, JSON decoding, provider error mapping e mapeamento de eventos de chat; impor limites e cancelamento propagado.
5. **Implementar a API Rust.** Async end-to-end, backpressure, ownership claro do cliente e do stream, fechamento determinístico e mensagens de erro sem segredo.
6. **Adicionar conformidade nas três entradas.** Fixtures determinísticas verificam eventos/resultados do núcleo Rust e os mesmos vetores exercitam os bindings Python e TypeScript. Nenhum chat de produção muda de implementação por padrão neste PR.
7. **Documentar e medir.** Publicar tabela de cobertura, gaps por provider e baseline reproduzível de latência/CPU/memória do transporte sintético; não usar resultado sintético para afirmar vantagem em chamadas remotas.

## Critérios de aceite do PR 1

- Contrato de entrada, saída, eventos e erros tem versão e exemplos JSON registrados.
- Testes comprovam ordem e conteúdo do stream, tool calls fragmentadas, usage, erro, timeout e cancelamento.
- Cancelar o consumidor interrompe o HTTP stream e libera recursos; nenhuma tarefa/socket permanece ativa.
- Retry só ocorre antes do primeiro evento entregue; após a primeira entrega, a falha é propagada sem recomeçar a geração.
- Chaves e headers sensíveis não aparecem em logs, exceptions, snapshots ou fixtures.
- Configuração/provider inválidos falham antes de enviar a requisição.
- A API Rust fornece streaming cancelável e os tipos necessários para os bindings preservarem o contrato atual de `ChatClient`.
- Os bindings Python e TypeScript são compilados no bootstrap, expõem completion e streaming sobre o mesmo núcleo e passam por testes próprios de contrato/erros; nenhum binding contém transporte ou política de provider duplicada.
- Fallback continua em um único lugar no consumidor Vectora e o caminho Python segue utilizável sem alteração de comportamento quando Rust estiver desligado.
- Casos comuns e de falha OpenAI-compatible têm paridade definida para os perfis incluídos; gaps conhecidos aparecem explicitamente na matriz.
- CI roda formatação, Clippy, testes Rust e conformidade sem credenciais ou APIs pagas.
- Nenhuma afirmação de performance/paridade ampla sem relatório reproduzível por SHA, ambiente e configuração.

## Riscos que o PR deve resolver explicitamente

- Diferenças de stream entre SSE e NDJSON e eventos terminais que contêm usage/tool calls.
- Compatibilidade de esquemas de tools e JSON inválido nos argumentos; a chamada deve sobreviver como erro tipado, não sumir silenciosamente.
- Semântica de retry e fallback com entrega parcial de texto.
- Cancelamento Python → binding → HTTP e descarte do iterador durante shutdown.
- Cancelamento TypeScript → binding → HTTP e descarte do `AsyncIterator` durante shutdown.
- Proteção de credenciais, logs de endpoint e cabeçalhos de atribuição de gateway.
- IPv6, TLS/proxies configurados pelo ambiente, endpoints customizados e limites de payload.
- ABI/empacotamento da extensão Python em Linux, Windows e macOS, inclusive arquiteturas suportadas.
- Testes de conformidade precisam separar comportamento protocolar compartilhado de extensões específicas de cada gateway.

## Decisões abertas antes de iniciar implementação

- Versões de Python e sistemas/arquiteturas mínimas suportadas pelo binding.
- Matriz de wheels e empacotamento da extensão Python para os sistemas e arquiteturas suportados.
- Versões de Node e sistemas/arquiteturas mínimas suportadas pelo pacote npm e matriz de publicação dos binários N-API.
- Política de versão entre crate Rust, pacote PyPI e pacote npm, com release rastreável ao mesmo SHA/tag e compatibilidade explícita.
- Forma de selecionar o backend Rust em runtime: configuração local de desenvolvimento, feature flag ou ambas; default permanece Python.
- Perfis exatos de endpoints compatíveis que entrarão no primeiro PR e suas diferenças observadas.
- Limites padrão de conexão/leitura/stream e quais configurações existentes serão passadas pelo consumidor.

## Preparação da trilha de storage

Antes de iniciar S0, levantar e classificar no Vectora:

- Cada uso efetivo dos seis serviços, incluindo qual dado é durável, transitório, reconstruível ou derivado; o processo que o possui; e quais consumidores dependem dele.
- Schema e contrato de sessão/transcript, eventos/ledger, checkpoints, memória/skills, filas de embedding, cache, rate limit, invalidação, pub-sub, vetores e índices.
- Modos lite/complete, defaults, credenciais/endpoints e ciclos de startup/shutdown sem transportar secrets ou defaults de produto para a engine.
- Migrations existentes, backup/restore, reconciliação, idempotência, TTL, ACK/redelivery, retenção, paginação, ordenação e garantias de transação.
- Gaps entre adapters atuais e o contrato comum proposto. Marcar cada comportamento como implementado, inferido, ausente ou específico do backend.

O texto adaptado em `docs/` deve reescrever a issue de origem para os contratos Rust e APIs de binding: responsabilidades, schemas, erros, capabilities, compatibilidade, suíte de conformidade, matriz de durabilidade e plano de migração/rollback. O plano deve preservar os requisitos úteis da issue local, sem trazer settings, tabelas, paths ou decisões proprietárias do Vectora como API da engine.

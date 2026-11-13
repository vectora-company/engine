# Plano de implementação: storage, ledger e mensageria

## Objetivo e fontes

Criar capacidades reutilizáveis para persistência, ledger, busca vetorial, KV,
filas e pub/sub na Engine Rust. O GH Bot TypeScript é o primeiro consumidor;
o Vectora Python é referência de comportamento e consumidor posterior. Cada
capacidade pública entrega Rust, bindings Python e TypeScript e conformidade no
mesmo PR. A publicação em crates.io, PyPI e npm tem seus próprios gates.

Fontes: `/media/brunosrz/externo/vectora-repos/issues/01-loop-agentico-e-harness.md`,
`/media/brunosrz/externo/vectora-repos/issues/02-storage-memoria-contexto-rag.md`,
`/media/brunosrz/externo/vectora-repos/issues/04-chat-streaming-e-threads.md`,
`/media/brunosrz/externo/vectora-repos/issues/08-contratos-backend-e-engine-rust.md` e
`/media/brunosrz/externo/vectora-repos/issues/10-avaliacao-benchmarks-e-estudos.md`; consulte também
`/media/brunosrz/externo/vectora-repos/vectora/backend/storage/`, `/media/brunosrz/externo/vectora-repos/vectora/backend/persistence/native/` e
`/media/brunosrz/externo/vectora-repos/vectora/backend/scheduling/mq.py`. As issues contêm requisitos de produto:
extraia contratos genéricos sem incorporar UI, workbenches ou RBAC do Vectora.

## Princípios e fronteiras

- Configurar SQL, vetores, KV, filas e pub/sub independentemente, permitindo
  SQLite + LanceDB + NATS, PostgreSQL + Qdrant + Redis e combinações mistas.
  Nenhum backend é obrigatório para todos os produtos.
- O host fornece identidade e escopo confiáveis, política, credenciais,
  configuração, autorização e ciclo de vida; a Engine valida e aplica o escopo
  às operações. Dados de produto e billing não viram schema universal.
- O ledger tem uma autoridade local SQLite obrigatória, mesmo quando PostgreSQL
  estiver selecionado para consultas ou projeção remota. Cada append confirma
  primeiro no SQLite e grava na mesma transação um item de outbox; PostgreSQL
  recebe a projeção de forma recuperável. Indisponibilidade ou conflito remoto
  nunca apaga nem desfaz o evento local. Índices, filas e notificações são
  projeções ou transporte. Pub/sub efêmero e filas duráveis são contratos
  distintos.
- Equivalência por par primeiro: o que existe nos dois lados de SQLite/PostgreSQL,
  LanceDB/Qdrant, Redis/NATS KV, Redis Streams/JetStream e Redis Pub/Sub/NATS
  Core é implementado nos dois, com a mesma semântica observável (a política
  está na spec 02). Só depois vem a especialização do que for incompatível.
  Especificar capabilities e as diferenças reais de consistência, ordenação,
  concorrência, retenção, TTL, ACK e replay que restarem, cada uma como lacuna a
  fechar ou especialização com motivo. Doubles em memória são internos
  aos testes; a configuração de produto não os seleciona. Indisponibilidade
  nunca troca silenciosamente o backend.
- Erros tipados, timeouts, cancelamento, limites e observabilidade sem segredos
  fazem parte de cada API. Sem dual-write entre implementações nem replay
  automático de efeito externo cujo resultado é desconhecido.

## Matriz de capacidades

| Capacidade          | Adapters                                                 | Base equivalente exigida nos dois lados                                           | Diferença que só a configuração do serviço explica                 |
| ------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Relacional e ledger | SQLite autoritativo; PostgreSQL selecionável e espelhado | Transações, conflitos, migrations, paginação, stores de run/loop/policy, backup.  | Isolamento e locks do motor; durabilidade conforme a configuração. |
| Vetorial            | LanceDB, Qdrant                                          | Métrica e score alinhados, filtros, índices, quantização, busca híbrida, limites. | Consistência e durabilidade conforme o deployment.                 |
| KV/cache            | Redis, NATS JetStream KV                                 | TTL, revisão/CAS, incremento atômico, prefixo, watch, limites.                    | Eviction e persistência conforme a configuração.                   |
| Fila/stream durável | NATS JetStream, Redis Streams                            | Grupos, ACK/NACK, redelivery com teto, retenção, replay, ordenação e DLQ.         | Retenção física conforme a configuração do serviço.                |
| Pub/sub             | NATS Core, Redis Pub/Sub                                 | Entrega efêmera, reconexão, assinaturas, comportamento sem assinante.             | Limites de buffer do serviço.                                      |

Adapters em memória podem apoiar testes internos, mas não são backends públicos
nem uma opção de configuração de produto. A configuração pública oferece os
seis backends concretos desta spec e permite combiná-los por capability.
Garantias dependem da configuração do serviço; não inferir durabilidade apenas
pelo nome do backend. A coluna "base equivalente" não é uma lista de
diferenças aceitas: cada item ausente em um lado é lacuna na matriz abaixo.

## Matriz de equivalência por par (verificada em 2026-11-13)

Fonte: código dos adapters em `main` (`dc1aa5e`) e nos PRs de storage já
integrados (#45–#72), com validação da dimensão no PR #77, ainda aberto na stack
atual sobre #86. As etapas de mídia #79–#86 e os PRs de mídia/documentação #87–#88 não alteram adapters de storage.
Consulte também os testes citados na
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md). "Lacuna"
significa que um lado tem e o outro não; "base comum ausente" significa que
nenhum dos dois lados expõe a capacidade.

| Par                       | Base equivalente já entregue                                                                                                                                                                           | Lacuna (um lado tem, o outro não)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Base comum ainda ausente nos dois lados                                                                                                                                                                   |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SQLite × PostgreSQL       | Ledger com idempotência, conflito de versão, paginação, reserva/admissão e checkpoints; projeção SQLite-first com outbox (`resilient_ledger.rs`).                                                      | PostgreSQL recebe evento + checkpoint dos stores SQLite lifecycle/loop/policy por outbox transacional opt-in (PR #92; schema v6); `ResilientLedger` habilita projeção e faz backfill. SQLite-only não acumula cópias. A branch filha `feat/postgres-admission-projection` adiciona outbox de reserva e APIs de leitura de reservation/admission; os testes service-real passaram; PR [#93](https://github.com/vectora-company/engine/pull/93), integrado na stack sobre #92. A decisão foi respondida no chat; o PR #94 implementa rebuild idempotente e comparação de contagens de agregados/versões por scope a partir do SQLite, com writers pausados. Backup integral usa `pg_dump`/backup gerenciado. SQLite é a cópia local e autoridade. | Interface SQL transacional tipada e limitada; testes concorrentes do PostgreSQL com serviço real; migrations PostgreSQL além da v1 com rollback documentado.                                              |
| LanceDB × Qdrant          | Cosseno normalizado em `[-1, 1]` (#57); dimensões de 1 a 65536 (#65); dimensão divergente da coleção retorna `InvalidInput` (#77); coleção ausente e reinício (#51).                                   | Nenhuma conhecida no contrato atual.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | Filtro por metadado além do escopo, índice ANN parametrizado, quantização, busca lexical/híbrida, falhas de indexação visíveis, backup/snapshot e health.                                                 |
| Redis × NATS JetStream KV | TTL por chave (NATS 2.11+, #59), listagem por prefixo (#67), remoção nos bindings (#68), validação de chaves (#69), CAS por revisão (#70), incremento (#71) e watch por prefixo (#72).                 | Nenhuma de contrato. O watch do Redis depende de `notify-keyspace-events` no servidor e retorna `Unsupported` sem ele; isso precisa constar em `capabilities()`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Testes Rust e Python de CAS, incremento, prefixo, watch e remoção (hoje só Node).                                                                                                                         |
| Redis Streams × JetStream | Grupos, ACK, retenção finita (100000, #60), cinco tentativas e DLQ consumível (#61), recuperação da transferência para a DLQ (#62), uma entrega por leitura (#63) e preservação da configuração (#64). | Deduplicação nativa existe só no JetStream e não é usada no `enqueue`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | NACK e reentrega com atraso, deduplicação na publicação, replay e ordenação com contrato, shutdown gracioso, redrive atômico da DLQ, envelope de job versionado e validação real dos cenários de #62–#64. |
| Redis Pub/Sub × NATS Core | Entrega efêmera; mensagem sem assinante é descartada (teste `pubsub_drops_messages_published_without_a_subscriber`).                                                                                   | Nenhuma conhecida.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Reconexão observável e assinatura por padrão/curinga.                                                                                                                                                     |

`capabilities()` enumera implementado, não implementado e notas, mas não tem
campo estruturado para separar lacuna, emulação e especialização com motivo de
incompatibilidade. Também omite o backup do SQLite e as ausências reais das
filas e do vetorial. Corrigir isso faz parte da Fase 0 e da Fase 2 do
[plano ativo](03-plano-continuidade.md).

O contrato vetorial compartilhado usa vetores não unitários, de modo que L2 e
cosseno ordenariam diferente; os dois adapters ordenam por cosseno.

## Estado das fases S0–S5 (verificado em 2026-11-13; atualizado nesta revisão)

As fases abaixo foram escritas como plano de implementação original. A tabela
separa o que existe do que falta; "implementada" não significa pronta para
produção.

| Fase                                | Estado atual                                                                                                                                                                                                        | Trabalho que continua pendente                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S0 — contratos                      | **Parcial.** Seleção versionada com os seis backends (`memory` rejeitado), contratos de ledger, vetor e mensageria, relatório estático `capabilities()` (#46) e fixture comum `tests/fixtures/conformance-v1.json`. | Fixtures comuns de KV, filas e vetores; `capabilities()` estruturado; health/readiness; contratos de shutdown e cancelamento de filas e pub/sub.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| S1 — SQLite e ledger                | **Implementada experimentalmente.** Ledger, admissão, checkpoints, stores de lifecycle/loop/policy, backup online/offline (#53) e concorrência entre processos (#49).                                               | Procedimento operacional de recovery documentado para o consumidor.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| S2 — PostgreSQL                     | **Parcial.** `PostgresLedger` e projeção via `ResilientLedger`, com queda e retorno do serviço testados.                                                                                                            | O PR #92 (filho do #91, integrado) enfileira eventos e checkpoints dos stores lifecycle/loop/policy no SQLite e projeta os dois atomicamente no PostgreSQL. O PR [#93](https://github.com/vectora-company/engine/pull/93), filho do #92, projeta reservas/admissions consultáveis e adiciona APIs de leitura. O PR [#94](https://github.com/vectora-company/engine/pull/94), integrado em `main`, implementa rebuild idempotente do PostgreSQL a partir do SQLite, compara contagens por scope e exige writers pausados durante a conferência; backup completo continua com `pg_dump`/backup gerenciado. O PR [#95](https://github.com/vectora-company/engine/pull/95), integrado em `main`, adiciona testes de rejeição de linhas extras e conteúdo divergente. Esta etapa adiciona teste service-real com dois projetores independentes; migrations PostgreSQL com rollback continuam pendentes. SQL genérico segue pendente. `flush_outbox`/`pending_count` estão no PR #91, filho do #90. |
| S3 — Redis/NATS                     | **Implementada experimentalmente.** KV completo (TTL, CAS, incremento, prefixo, watch), filas com retenção, tentativas e DLQ, pub/sub efêmero, reinício dos serviços testado.                                       | NACK/atraso, deduplicação, replay, shutdown, redrive atômico, reconexão de pub/sub e validação real de #62–#64.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| S4 — LanceDB/Qdrant                 | **Implementada experimentalmente.** Score cosseno alinhado, limites de dimensão, dimensão divergente, coleção ausente e reinício do Qdrant.                                                                         | Filtros de metadado, índice ANN, quantização, busca híbrida, falhas de indexação e backup/snapshot.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| S5 — recuperação, contexto e adoção | **Parcial no recovery; planejada em contexto e adoção.** Run e loop têm persistência e reconciliação SQLite.                                                                                                        | Memória, Context Planner e RAG não existem; nenhum consumidor adotou storage da Engine.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

A matriz por contrato e binding está em
[`02-matriz-conformidade-capabilities.md`](../analyses/02-matriz-conformidade-capabilities.md).

Os contratos de vetores (`VectorStore`) e KV (`KvStore`) têm adapters que
selecionam uma implementação por configuração. Filas duráveis e pub/sub têm
APIs distintas (`DurableQueue` e `EphemeralPubSub`): a fila entrega um handle
que deve receber ACK após o processamento; liberar o handle sem ACK mantém a
possibilidade de redelivery conforme o visibility timeout, até o teto de
tentativas, quando a mensagem vai para a DLQ. O binding retorna bytes sem
reinterpretar o payload. Handles de entrega e assinaturas são locais ao
processo e não devem ser persistidos em configuração.

Redis Streams recupera entregas pendentes com `XAUTOCLAIM`; NATS JetStream usa
consumidores duráveis e `ack_wait`. O TTL por chave existe nos dois KV (no
NATS via TTL de mensagem do JetStream, que exige NATS 2.11+). Os dois adapters
vetoriais reportam similaridade cosseno em `[-1, 1]`, maior é mais próximo.

No ledger, `ResilientLedger` grava evento e outbox atomicamente em SQLite antes
de tentar a projeção PostgreSQL. O resultado informa `synchronized`, `pending`
ou `conflict`; um conflito de projeção não desfaz o evento local. Retentativas
devem reutilizar a mesma chave de idempotência. O append do binding usa esse
contrato SQLite-first; não abrir um ledger PostgreSQL isolado para substituir a
autoridade local. O PR #91 expõe `flush_outbox` e `pending_count` em Python e
TypeScript; contagem é `int` em Python e `bigint` em TypeScript. Sem PostgreSQL,
um flush válido não remove itens locais. Limite permitido: 1–1000. O PR #92
adiciona checkpoints à outbox SQLite (schema v6), com projeção opt-in e backfill, e projeta evento/checkpoint
na mesma transação remota para run lifecycle, model/tool loop e policy. A branch
filha `feat/postgres-admission-projection` migra o SQLite ao schema v8, adiciona
outbox de reservas standalone, projeta reservation + admission + event +
checkpoint atomicamente e acrescenta APIs de leitura. Verificação service-real
passou; a decisão de `brunosrz` no chat adota backup nativo/gerenciado mais verify/rebuild da projeção a partir do SQLite. O PR [#94](https://github.com/vectora-company/engine/pull/94), head `984e2d066dcc05b0a98e076361ada632751fc61a`, implementa rebuild por scope e verificação de contagens, com writers pausados. Os testes service-real no PR [#95](https://github.com/vectora-company/engine/pull/95), integrado em `main`, cobrem linhas extras e conteúdo divergente. Esta etapa valida projetores independentes concorrendo sobre a mesma outbox; migrations continuam pendentes. SQLite continua autoridade local.

## Contrato do ledger

Separar eventos append-only, projeções, checkpoints, referências de artifacts
e cursores. O envelope versionado identifica evento, agregado, sequência,
tipo, escopo, chave de idempotência e, quando aplicável, run/turno/thread.
O host autoriza acesso; argumentos originados no modelo não são autoridade.

Admitir execução atomicamente por chave e identidade de operação: repetição com
mesmo conteúdo recupera resultado; chave reutilizada com conteúdo incompatível
retorna conflito. Definir ordem por agregado, concorrência, expected version,
paginação, retenção e tratamento de lacunas. Gravações relacionadas que exigem
atomicidade devem compartilhar uma transação SQL; efeitos entre SQL, broker e
vetores exigem recuperação e reconciliação explícitas, sem prometer transação
distribuída. Projetar publicação recuperável após commit SQL com outbox quando
o fluxo exigir essa garantia.

Checkpoints são versionados e rejeitam formato incompatível. Recovery distingue
efeito confirmado, não aplicado e desconhecido; o último exige reconciliação.
Definir limites de payload, redação antes da persistência e referência com hash,
tamanho e política de acesso para artifacts grandes. O ledger não armazena
segredos ou raciocínio interno bruto por padrão.

## PRs propostos

Cada PR parte do anterior quando houver dependência e deve registrar contrato,
limites, critérios de aceite, conformidade e interfaces nas três linguagens.
Os números abaixo representam entregas propostas, não issues já publicadas.

### 1. S0 — contratos e conformidade inicial

Inventariar operações e consumidores no backend Python e no GH Bot; definir
tipos versionados, escopo, erros, health/capabilities, cancelamento, timeout,
shutdown, cursor e limites. Especificar separadamente ledger, SQL, vetores,
KV, filas e pub/sub; criar matriz de garantias e fixtures de conformidade.
Documentar as decisões de API e dependências Rust antes dos adapters. Expor os
contratos utilizáveis pelos bindings Python e TypeScript no mesmo PR.

**Aceite:** contratos reproduzíveis e versionados, seleção independente,
erros e capabilities observáveis; nenhuma capacidade não implementada é
anunciada como disponível. Identificar explicitamente transações exigidas pelo
ledger e a fronteira entre estado persistido e transporte de eventos.

### 2. S1 — SQLite e ledger vertical

Implementar migrations próprias da Engine, conexão/pool e lifecycle; admissão
idempotente, append com versão esperada e sequência por agregado, leitura por
cursor e checkpoints. Expor o mesmo núcleo pelos dois bindings e exercitar um
fluxo mínimo do GH Bot. Avaliar cancelamento, concorrência e recuperação com
processos independentes; não migrar dados de produto neste PR.

**Aceite:** reinício preserva dados; retries não duplicam eventos; conflito de
idempotência é detectado; falha transacional não deixa estado parcial; cursores
e checkpoints inválidos falham explicitamente. Testar locks, migrations,
backup/restore quando suportados, redação e isolamento de escopo.

### 3. S2 — PostgreSQL como adapter selecionável e projeção

Implementar o contrato relacional com transações, pool e migrations próprios
de PostgreSQL, mantendo SQLite como autoridade local obrigatória do ledger.
Adicionar uma outbox local transacional e projeção idempotente, retomável após
restart. Reutilizar fixtures de contrato e acrescentar testes concorrentes com
serviço real, inclusive conflito de versão, locks, falha e restart. Expor
configuração e lifecycle nos dois bindings.

**Aceite:** append continua disponível com PostgreSQL fora do ar e é confirmado
no SQLite; a projeção pode ser retomada sem duplicar eventos após restart;
divergências de isolamento e operação documentadas como lacunas ou como
diferença de configuração do serviço. Equivalência é afirmada só para os casos
medidos.

### 4. S3 — Redis e NATS: KV, filas e pub/sub

Implementar KV Redis e JetStream KV, filas Redis Streams e JetStream e pub/sub
Redis Pub/Sub e NATS Core em interfaces separadas. Incluir envelopes de job,
ACK/NACK ou equivalente, redelivery, deadlines, cancelamento, shutdown,
backpressure, retenção, replay e capabilities de DLQ conforme o serviço.
Documentar como o host faz idempotência e reconciliação com o ledger; comparar
as chaves NATS e os consumidores existentes no Vectora antes do port.

**Aceite:** serviços reais cobrem desconexão/reconexão, restart, mensagem
duplicada, consumidor abandonado, shutdown, TTL e limites. Pub/sub não é
tratado como confirmação durável. Bindings Python e TypeScript acompanham as
operações entregues.

### 5. S4 — LanceDB e Qdrant

Implementar coleções, upsert/delete, filtros de escopo, dimensões e busca
vetorial, com métricas e direção de score declaradas. Expor busca lexical ou
híbrida apenas quando suportada. Integrar fixtures de embeddings existentes
sem exigir provider externo e disponibilizar ambos os bindings.

**Aceite:** testes com backends reais verificam filtros antes da recuperação,
dimensão inválida, coleção ausente, ordenação/score, concorrência e falhas de
indexação. O contrato exige métrica e score alinhados entre os backends; a
equivalência numérica só é afirmada para os casos medidos.

### 6. S5 — recuperação, contexto e adoção progressiva

Integrar ledger, checkpoints e política do harness; tratar reconciliação de
efeitos desconhecidos e projeções pendentes. Acrescentar contratos genéricos
de memória/contexto/RAG, proveniência e orçamento conforme necessidades reais
do GH Bot. Medir com fixtures isoladas; só depois planejar integração reversível
no Vectora Python por capability, com migração, contagens, hashes e rollback.

**Aceite:** cenários de restart, replay, cancelamento e falha parcial sem
duplicar efeitos; índices incompletos são observáveis. GH Bot usa o contrato
da Engine sem implementação paralela do núcleo. Nenhuma troca ampla no Vectora
ocorre sem conformidade, evidência operacional e rollback.

## Gates transversais

- Testar o contrato Rust e os bindings Python/TypeScript no mesmo PR; fixtures
  comuns não substituem testes de serviços reais e falhas de rede/processo.
- Verificar migrations, compatibilidade, concorrência, limites, redaction,
  isolamento, observabilidade, shutdown e recuperação aplicáveis a cada PR.
- Medir latência, throughput e memória por SHA, ambiente, versão do backend,
  configuração e carga. Não alegar paridade ou ganho sem evidência.
- Revisar instalação, empacotamento e plataformas suportadas antes de publicar;
  segredo e credenciais nunca entram em logs, fixtures ou exportações.

## Histórico das entregas de storage

As etapas S0–S5 entregues pela stack #45–#72 estão integradas em `main`
(`dc1aa5e`) e listadas na tabela de fases acima. A execução combinada mais
recente dos testes service-real está registrada na
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md), com a
contagem e os serviços usados. Resultado local; CI remoto não é usado como
evidência.

## Decisões

Já decididas por `brunosrz`: SQLite como autoridade local e PostgreSQL como
projeção; backup online e offline coordenado; sem limite novo de payload; TTL
com NATS 2.11+; filas com retenção finita (100000), cinco tentativas e DLQ;
equivalência por par antes da especialização.

Ainda abertas: granularidade e versão dos crates; política de compatibilidade
de schemas e bindings; retenção e compactação do ledger; envelope de job;
mapeamento comum de índices vetoriais; formato estruturado de
`capabilities()`; perfil de deployment do primeiro fluxo do GH Bot. Elas
dependem de inventário e de provas de conformidade e são perguntadas no chat
quando a etapa correspondente começar.

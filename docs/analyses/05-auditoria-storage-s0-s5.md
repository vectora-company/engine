# Auditoria de storage e ledger S0–S5

**Estado:** correção de contrato implementada; validação runtime pendente
**Branch:** `fix/storage-selection-contract`
**Escopo desta etapa:** alinhar o contrato público de seleção às implementações
e registrar a cobertura que ainda precisa ser demonstrada.

## Decisão desta etapa

O Vectora define seis backends de produto combináveis por capability:

- SQL: SQLite e PostgreSQL.
- Vetores: LanceDB e Qdrant.
- Cache/KV e mensageria: NATS e Redis, com filas e pub/sub usando contratos
  distintos.

`memory` não é um sétimo backend. Antes desta etapa, o enum Rust e o tipo
TypeScript aceitavam `memory` para KV, filas e pub/sub, mas os três adapters
retornavam `Unsupported` ao conectar. A configuração permitia, portanto, um
estado impossível. A correção remove a variante dos contratos públicos e
verifica que Rust, Python e TypeScript rejeitam o valor. Doubles em memória
podem continuar privados aos testes.

## Fotografia do código

| Fase                          | Implementação encontrada                                                                                                              | Cobertura/limite observado                                                                                                                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S0 — contratos                | `StorageSelection` versionada seleciona SQL, vector, KV, queue e pub/sub independentemente.                                           | A seleção indica intenção; não comprova conexão, health ou garantias equivalentes. Esta etapa fecha a opção pública sem adapter.                                                                           |
| S1 — SQLite/ledger            | Ledger append-only, checkpoints e stores SQLite para run, loop e policy; `ResilientLedger` grava localmente antes da projeção remota. | Ainda faltam provas operacionais de backup/restore, concorrência entre processos e recovery. SQLite permanece a autoridade local do append resiliente.                                                     |
| S2 — PostgreSQL               | `PostgresLedger` e projeção a partir do ledger SQLite.                                                                                | Não há SQL genérico nem stores Postgres para lifecycle, loop e policy. Serviço real, conflitos, restart e recovery requerem cobertura própria.                                                             |
| S3 — Redis/NATS               | Redis e NATS JetStream para KV; Redis Streams e JetStream para filas; Redis Pub/Sub e NATS Core para pub/sub.                         | TTL não é idêntico no KV; ACK, redelivery, retenção, reconexão e shutdown diferem. Integração service-real não é uma suíte comum completa.                                                                 |
| S4 — LanceDB/Qdrant           | Adapters de upsert, busca e delete nos bindings.                                                                                      | Filtros, score, isolamento, limites de payload e falhas precisam de testes service-real específicos; a diferença semântica entre os adapters é lacuna de equivalência a fechar, não característica aceita. |
| S5 — recuperação/consumidores | Recovery e reconciliação do runtime são SQLite-first.                                                                                 | Memory, Context Planner, RAG e adoção no Vectora/GH Bot não foram comprovados como capacidades integradas da engine.                                                                                       |

## Verificações desta correção

Critérios locais para a etapa:

1. Os enums Rust e tipos TypeScript só aceitam os backends com adapters
   concretos.
2. Rust e ambos os bindings rejeitam `memory` antes de tentar abrir uma
   conexão.
3. A seleção válida continua independente: cada família pode ser combinada
   sem obrigar um perfil “nativo” ou “servidor”.
4. A documentação e a sabatina deixam claro que doubles em memória são apenas
   detalhes de teste.

### Concluídas localmente

- `cargo fmt --all -- --check` — passou.
- `cargo check -p vectora-engine --locked` — passou.
- `uv run ruff check utils bindings/python` — passou.
- `uv run ruff format --check utils bindings/python` — passou.
- `uv run ty check utils bindings/python` — passou.
- `pnpm typecheck` em `bindings/typescript` — passou.
- Hooks estáticos do `pre-commit` nos arquivos alterados — passaram, incluindo
  Prettier, Oxlint, Ruff, ty, Bandit, Clippy e rustfmt.
- Hook `Rust tests` do `pre-commit` — interrompido; não foi contado como
  aprovado. `git diff --check` deve ser repetido após a inclusão final.

### Pendentes nesta máquina

- O teste `cargo test -p vectora-engine storage::tests:: --locked` começou a
  compilar, mas foi interrompido após mais de dez minutos sob pressão alta de
  swap e I/O. Não foi contado como aprovado.
- A execução de testes Rust iniciada pelo `pre-commit` também foi interrompida;
  não foi contado como aprovada.
- Os testes runtime Python/Node não foram executados: os addons nativos locais
  estão desatualizados e exigiriam recompilação. O typecheck TypeScript e os
  checks estáticos Python acima não substituem esses testes.
- Testes contra serviços reais e a matriz S0–S5 continuam pendentes conforme
  as fases abaixo.

Esta auditoria não declara paridade operacional dos adapters nem substitui os
testes com serviços reais. A identificação exata da revisão publicada fica no
histórico Git e no pull request desta correção.

## Etapa S0: relatório de capabilities (branch `feat/storage-readiness-matrix`)

Decisão de contrato: `StorageSelection::capabilities()` devolve um relatório
estático e versionado (`STORAGE_CONTRACT_VERSION`, aditivo) com, por backend
selecionado, o que o adapter implementa, o que não implementa, suporte a TTL,
direção do score vetorial e durabilidade. Os fatos vêm do código dos adapters
(`messaging.rs`: TTL do JetStream KV retorna `Unsupported`; `vector.rs`:
LanceDB devolve distância e Qdrant similaridade cosseno; `storage/postgres.rs`:
somente projeção de ledger). O relatório não contata serviços, não é health
check e não prova garantias operacionais. Alternativa descartada: probe ativo
de serviço, por exigir credenciais/rede e misturar readiness com configuração.
Exposto em Rust, Python (`storage_capabilities`) e TypeScript
(`storageCapabilities`). Estado: ver checklist; testes ainda dependem de
compilação nesta máquina.

## Etapa S1: concorrência e restart do ledger SQLite

Testes adicionados em `storage/sqlite.rs` (conexões independentes em threads
sobre o mesmo arquivo): dois appends com a mesma `expected_version` confirmam
exatamente um evento e o outro recebe `VersionConflict`; retries idênticos
concorrentes devolvem o mesmo evento sem duplicar; eventos e idempotência
sobrevivem à reabertura do arquivo; cursor/limite inválidos falham com
`InvalidInput`. Resultado local: 12 testes de `storage::sqlite::tests` passam;
nenhum defeito reproduzido. Também há um teste com 3 processos independentes
(o binário de teste reexecuta a si mesmo como escritor): com a mesma versão
esperada, exatamente um confirma e os outros dois recebem `VersionConflict`.
Limite: backup/restore e recovery operacional não foram cobertos; o mecanismo
de backup (cópia online do arquivo SQLite) afeta o contrato e fica como decisão
a registrar antes de implementar.

## Matriz service-real

Os serviços de teste (Redis, NATS com JetStream, Qdrant e PostgreSQL) existem
como contêineres Podman locais (`vectora-engine-*`) e precisam de
`podman start`. Os testes condicionais leem `VECTORA_TEST_POSTGRES_URL`,
`VECTORA_TEST_REDIS_URL`, `VECTORA_TEST_NATS_URL` e `VECTORA_TEST_QDRANT_URL`;
sem elas retornam sem validar o serviço.

Resultado local (2026-11-12, serviços reais ligados, `--test-threads=1`):
`cargo test -p vectora-engine --lib --locked storage::` — 58 passaram, 0
falharam, incluindo os testes opt-in de reinício Redis, NATS, PostgreSQL e
Qdrant. Testes adicionados nas etapas anteriores:

- fila vazia expira sem entrega; job confirmado (ACK) não é reentregue
  (Redis Streams e NATS JetStream);
- mensagem publicada sem assinante é descartada (Redis Pub/Sub e NATS Core),
  como declarado em `capabilities()`;
- namespaces de KV são isolados e `delete` remove só a chave do namespace;
- contrato vetorial comum a LanceDB e Qdrant: ordenação, `limit`, upsert
  substitui, mesmo id em escopos distintos, delete por escopo, dimensão
  inválida falha, e direção do score (LanceDB distância/menor é mais próximo;
  Qdrant similaridade/maior é mais próximo), coerente com `capabilities()`.

Reinício de serviços (`VECTORA_TEST_PODMAN_RESTART=1`): Redis e NATS — KV/jobs
sobreviveram a `podman restart`, os adapters voltaram a operar e o ACK
funcionou; PostgreSQL — o append SQLite permaneceu `Pending` durante a queda e
a outbox drenou em ordem; Qdrant — coleção e ponto continuaram disponíveis e a
busca recuperou pelo mesmo adapter. O Qdrant foi reiniciado sem volume montado,
no mesmo container; isso comprova o comportamento nessa configuração, não
durabilidade após remoção ou recriação do container.

Execução integrada inicial: 54 passaram e uma falhou porque `podman stop`
removeu o container PostgreSQL original, configurado com `AutoRemove=true`.
Após recriá-lo sem remoção automática e proteger o teste com verificação de
`AutoRemove=false`, a execução combinada com os quatro serviços e todos os
reinícios passou: 58/58. A durabilidade observada depende da configuração de
teste e não é garantia geral dos adapters.

Incidente: o contêiner `vectora-engine-pg-test` havia sido criado com remoção
automática; `podman stop` o apagou. Foi recriado (mesma imagem, porta, usuário
e banco; senha de teste nova, sem remoção automática). O teste confere a
configuração antes de parar o serviço.

Limites de entrada (testes sem serviço, `storage::vector` e `storage::messaging`):
os casos limite aceitos passam a validação e só falham ao alcançar um serviço
inexistente (`Storage`), enquanto os inválidos retornam `InvalidInput`.
Observação por leitura de código: `metadata` vetorial, valores de KV e payloads
de fila não têm limite de tamanho, e a busca do Qdrant não limita a dimensão da
consulta (o upsert limita a 65536). Definir limites muda o contrato público e
depende de decisão do mantenedor; não foi implementado.

Não coberto: shutdown gracioso do adapter, retenção/limites JetStream, limites
de payload e falhas de indexação. Permanecem `não executado`.

## Estado do ledger multiprocesso (PR #49)

`cargo test -p vectora-engine --lib --locked storage::sqlite::tests --
--test-threads=1` passou com 14 testes. O novo teste executa três processos
independentes do próprio binário de teste contra o mesmo arquivo: um append
com `expected_version=0` confirma, dois recebem `VersionConflict`, e uma
reabertura lê um único evento. Isso fecha a cobertura de concorrência entre
processos, ainda sujeito à revisão/integração do PR #49.

## Estado vetorial (PR #51)

A suíte comum agora exige que uma coleção ausente resulte em
`VectorError::Storage` em LanceDB e Qdrant. O teste opt-in de reinício do
Qdrant cria uma coleção e um ponto, reinicia o contêiner explicitamente
configurado e consulta o mesmo ponto com o mesmo adapter. A suíte vetorial
passou com 6 testes, incluindo LanceDB local e Qdrant real. A execução
integrada com os quatro serviços, `VECTORA_TEST_PODMAN_RESTART=1` e
`--test-threads=1` passou com 58/58. Isso valida a retenção no reinício do
mesmo contêiner de teste, sem volume montado, não após recriação do contêiner.

## Trabalho que permanece

- Criar uma matriz de conformidade por backend com o mesmo corpus e as mesmas
  asserções onde as semânticas forem comuns.
- Exercitar serviço real para desconexão, restart, falha parcial, retry,
  redelivery, ACK, TTL/retention, isolamento e shutdown; Redis, NATS,
  PostgreSQL e Qdrant já têm cobertura local de reinício, com limites detalhados.
- Falhas de indexação de LanceDB/Qdrant; filtros, escopo, dimensão, score,
  ordenação, coleção ausente e limites de entrada têm cobertura. Sem limite de
  tamanho de payload por decisão do mantenedor.
- Retenção/limites e shutdown gracioso do JetStream. Backup/restore do ledger
  está implementado (ver decisões).
- Manter interfaces de Memory/Context/RAG e integração dos consumidores em
  etapas separadas, com contratos e fallback próprios.

## Decisões do mantenedor (2026-11-12)

**Backup/restore do ledger SQLite: implementar as duas opções.** Contrato
implementado em `storage/sqlite/backup.rs` e exposto em Rust, Python e
TypeScript (`sqlite_ledger_backup_online`, `sqlite_ledger_backup_offline`,
`sqlite_ledger_verify_backup`, `sqlite_ledger_restore`):

- **Online** (`backup_online`): snapshot consistente via API de backup do SQLite
  (feature `backup` do `rusqlite`, sem crate nova) enquanto outros escritores
  continuam; grava `<destino>.partial`, verifica e só então renomeia.
- **Offline coordenado pelo host** (`backup_offline`): toma `BEGIN EXCLUSIVE`
  sem espera; se algum escritor ainda estiver ativo retorna `Busy`, o que
  prova que o host não parou os escritores. Só então copia o arquivo.
- **Verificação** (`verify_backup`): abre somente leitura, `integrity_check`,
  versão do esquema (`SchemaVersion` se mais nova que a suportada) e contagem
  de eventos; arquivo que não é ledger retorna `BackupInvalid`.
- **Restore** (`restore`): verifica o backup, recusa destino existente sem
  `replace`, recusa destino com escritor ativo (`Busy`) e, com `replace`, guarda
  o arquivo anterior em `<destino>.replaced` (não sobrescreve um `.replaced`
  existente). O host continua responsável por parar os escritores e reabrir.
- Nenhum modo sobrescreve arquivo existente; falhas removem o `.partial`.
- Limite: `backup_online` abre o ledger com `open_persistent`, que executa as
  migrações de esquema pendentes no arquivo de origem.

**Limites de tamanho: sem limite novo.** Um banco vetorial é grande; `metadata`
vetorial, valores de KV e payloads de fila permanecem sem limite de tamanho
(comportamento atual, sem mudança de contrato). O limite de 1 MiB por payload
de evento do ledger e as demais validações de entrada existentes continuam.

Evidência local: 5 testes Rust em `sqlite::backup::tests` (snapshot consistente
com escritor ativo, `Busy`, restore com `.replaced`, destino em uso, arquivos
corrompidos/estrangeiros/esquema novo) e testes equivalentes em Python e Node.

## Decisão vital aguardando `brunosrz`: retenção e redelivery das filas duráveis

Evidência (testes de caracterização contra Redis e NATS reais, 2026-11-12):

- **Redis Streams:** depois do ACK a entrada continua no stream (`XLEN` não
  diminui); não há `XTRIM`/`MAXLEN`. O stream cresce sem limite.
- **NATS JetStream:** o stream é criado com retenção `Limits`, `max_messages`
  e `max_age` ilimitados; jobs confirmados permanecem (`messages` não diminui).
  O consumidor tem `max_deliver` ilimitado: uma mensagem que nunca é confirmada
  é reentregue indefinidamente, e `dead_letter_queue` consta como não
  implementado em `capabilities()`.

Pergunta: qual política de retenção e de redelivery as filas devem adotar?

1. Remover o job ao confirmar (WorkQueue no NATS; `XDEL`/`XACKDEL` no Redis):
   sem crescimento, mas sem replay após o ACK.
2. Manter com limite configurável pelo host (idade e/ou número máximo de
   entradas, mais `max_deliver`): preserva replay limitado; o host escolhe
   valores e responde por perda de dados ao ultrapassá-los.
3. Manter como está: sem mudança de contrato, com crescimento ilimitado e
   redelivery sem teto como limitação documentada.

A escolha afeta perda de dados, replay e configuração pública. Trabalho
dependente: implementar a política escolhida e os testes service-real
correspondentes. Perguntado a `brunosrz` no chat; aguardar a resposta.

## Política de equivalência por par (2026-11-12)

O mantenedor definiu a regra: a base funcional dos pares de storage é
equivalente (tudo que existe nos dois lados é implementado nos dois), e a
especialização vem depois, apenas para o que for incompatível com o outro lado.
Os documentos foram revisados nesse sentido (spec 02, planos 01, 03 e 04,
análises 03 e 04, `AGENTS.md`). Isto corrige afirmações anteriores desta
auditoria e dos PRs #46 e #47: o teste vetorial compartilhado usa vetores
unitários, onde L2 e cosseno ordenam igual, e portanto não prova equivalência
de ranking; o relatório `capabilities()` declara a diferença sem corrigi-la.

A matriz de lacunas por par está no plano de storage. Uma pergunta de
arquitetura depende do mantenedor: a autoridade local obrigatória em SQLite do
ledger resiliente (PostgreSQL apenas como projeção) é compatível com a
equivalência entre os dois, ou o PostgreSQL deve poder ser autoridade única
quando selecionado? A pergunta está no chat; nada foi alterado no contrato.

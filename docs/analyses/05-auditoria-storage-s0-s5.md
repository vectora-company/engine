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

| Fase                          | Implementação encontrada                                                                                                              | Cobertura/limite observado                                                                                                                             |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| S0 — contratos                | `StorageSelection` versionada seleciona SQL, vector, KV, queue e pub/sub independentemente.                                           | A seleção indica intenção; não comprova conexão, health ou garantias equivalentes. Esta etapa fecha a opção pública sem adapter.                       |
| S1 — SQLite/ledger            | Ledger append-only, checkpoints e stores SQLite para run, loop e policy; `ResilientLedger` grava localmente antes da projeção remota. | Ainda faltam provas operacionais de backup/restore, concorrência entre processos e recovery. SQLite permanece a autoridade local do append resiliente. |
| S2 — PostgreSQL               | `PostgresLedger` e projeção a partir do ledger SQLite.                                                                                | Não há SQL genérico nem stores Postgres para lifecycle, loop e policy. Serviço real, conflitos, restart e recovery requerem cobertura própria.         |
| S3 — Redis/NATS               | Redis e NATS JetStream para KV; Redis Streams e JetStream para filas; Redis Pub/Sub e NATS Core para pub/sub.                         | TTL não é idêntico no KV; ACK, redelivery, retenção, reconexão e shutdown diferem. Integração service-real não é uma suíte comum completa.             |
| S4 — LanceDB/Qdrant           | Adapters de upsert, busca e delete nos bindings.                                                                                      | Filtros, score, isolamento, limites de payload e falhas precisam de testes service-real específicos; os adapters não são semanticamente idênticos.     |
| S5 — recuperação/consumidores | Recovery e reconciliação do runtime são SQLite-first.                                                                                 | Memory, Context Planner, RAG e adoção no Vectora/GH Bot não foram comprovados como capacidades integradas da engine.                                   |

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

## Trabalho que permanece

- Criar uma matriz de conformidade por backend com o mesmo corpus e as mesmas
  asserções onde as semânticas forem comuns.
- Exercitar serviço real para desconexão, restart, falha parcial, retry,
  redelivery, ACK, TTL/retention, isolamento e shutdown.
- Definir e testar limites de filtros, scores e payloads de LanceDB/Qdrant.
- Provar concorrência, backup, restore, projeção e recovery para ledger SQLite
  e PostgreSQL, sem enfraquecer o SQLite como autoridade local resiliente.
- Manter interfaces de Memory/Context/RAG e integração dos consumidores em
  etapas separadas, com contratos e fallback próprios.

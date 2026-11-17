# Plano de implementação: storage, ledger e mensageria

## Objetivo e fontes

Criar capacidades reutilizáveis para persistência, ledger, busca vetorial, KV,
filas e pub/sub na Engine Rust. O GH Bot TypeScript é o primeiro consumidor;
o Vectora Python é referência de comportamento e consumidor posterior. Cada
capacidade pública entrega Rust, bindings Python e TypeScript e conformidade na
mesma etapa. A publicação em crates.io, PyPI e npm tem seus próprios gates.

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
nem uma opção de configuração de produto. A configuração pública atualmente
oferece os seis backends concretos desta spec e permite combiná-los por
capability. FalkorDB graph é uma capability adicional independente, implementada e fora dos cinco pares de equivalência; veja [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md).
Garantias dependem da configuração do serviço; não inferir durabilidade apenas
pelo nome do backend. A coluna "base equivalente" não é uma lista de
diferenças aceitas: cada item ausente em um lado é lacuna na matriz abaixo.

## Matriz de equivalência por par (estado verificado em 2026-11-17)

Fonte: `main` no GitHub em `4140ddeffa646c4dfd2f0e6a7402a1b00e337099` e
código/testes da stack local. A posição publicada e a base da stack são
consultadas dinamicamente no GitHub; esta matriz permanece organizada por
capability e não por entrega ou pull request. Consulte também a
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md).

| Par                       | Base equivalente entregue                                                                                                                                                                                                             | Lacunas verificadas                                                                                                                                                                                                                                           | Base comum ainda ausente                                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| SQLite × PostgreSQL       | Ledger SQLite-first, stores SQLite de lifecycle/loop/policy, backup/restore SQLite, projeção PostgreSQL de eventos/checkpoints/reservations/admissions, flush, rebuild/verify e testes de divergência/concorrência no mesmo processo. | PostgreSQL não oferece APIs locais equivalentes aos stores especializados SQLite; a cópia SQLite segue autoridade. Backup PostgreSQL é gerenciado pelo operador (`pg_dump` ou serviço equivalente); Engine verifica/reconstrói a projeção a partir do SQLite. | SQL genérico tipado, migrations PostgreSQL com rollback e concorrência multiprocesso dos projetores/admissions.                     |
| LanceDB × Qdrant          | Upsert/delete por escopo, coleção, cosseno normalizado em `[-1, 1]`, dimensões 1–65536 e validação de coleção/dimensão divergente.                                                                                                    | Nenhuma diferença deliberada aceita; recursos presentes só no backend e viáveis no outro continuam lacunas a avaliar.                                                                                                                                         | Filtro por metadado, ANN configurável, quantização, busca lexical/híbrida, falha de indexação observável, backup/snapshot e health. |
| Redis × NATS JetStream KV | get/put/delete, TTL, CAS por revisão, incremento, listagem por prefixo e watch; suíte real Rust/Python/TypeScript cobre os dois backends.                                                                                             | NATS requer Server 2.11+ para TTL por mensagem. Watch Redis exige keyspace notifications `K$gxe` ou `KA`; a Engine não altera configuração do servidor.                                                                                                       | Nenhuma operação básica conhecida; TTL e watch dependem dos pré-requisitos descritos.                                               |
| Redis Streams × JetStream | Enqueue/receive/ACK, redelivery, retenção limitada, teto de tentativas e DLQ consumível; testes reais cobrem corrida ACK×DLQ, retomada NATS, pull único, timeout e preservação de configuração.                                       | Redis transfere para DLQ e confirma origem atomicamente. JetStream publica com deduplicação por sequência antes do ACK; a semântica é at-least-once e pode duplicar se ACK se perder.                                                                         | NACK, retry agendado, deduplicação pública de enqueue, replay por cursor, shutdown gracioso e redrive atômico da DLQ.               |
| Redis Pub/Sub × NATS Core | Publicação/assinatura efêmera; mensagem sem assinante é descartada.                                                                                                                                                                   | Sem diferença deliberada aceita.                                                                                                                                                                                                                              | Reconexão observável e assinaturas por padrão/curinga.                                                                              |

`capabilities()` declara adapters implementados, lacunas comuns e requisitos
operacionais, incluindo modos de backup, watch Redis e requisitos de versão do
NATS. A API ainda não representa lacuna, emulação e especialização como
categorias estruturadas; esse refinamento segue como trabalho. O relatório é
estático e não testa disponibilidade do serviço.

O contrato vetorial compartilhado usa vetores não unitários, de modo que L2 e
cosseno ordenariam diferente; os dois adapters ordenam por cosseno.

## Estado das fases S0–S5 (verificado em 2026-11-17)

| Fase                                | Estado atual                                                                                                                                                                           | Trabalho que continua pendente                                                                                                             |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| S0 — contratos                      | **Parcial.** Seleção versionada, contratos Rust e bindings, fixture inicial e relatório `capabilities()` coerente com os adapters.                                                     | Fixtures comuns de KV, filas e vetores; categorias estruturadas de capability; health/readiness; shutdown e cancelamento de filas/pub-sub. |
| S1 — SQLite e ledger                | **Implementada experimentalmente.** Ledger, admissão, checkpoints, stores de lifecycle/loop/policy, backup online/offline/verify/restore e concorrência entre processos.               | Documentar procedimento operacional de recovery para consumidores.                                                                         |
| S2 — PostgreSQL                     | **Parcial.** Projeção SQLite-first com outbox, stores e admissions projetados, flush, rebuild/verify, rejeição de divergências e concorrência entre dois projetores no mesmo processo. | Migrations PostgreSQL com rollback; concorrência multiprocesso/admissions; SQL genérico tipado.                                            |
| S3 — Redis/NATS                     | **Parcial.** KV, filas com retenção/tentativas/DLQ e pub/sub efêmero; Redis 8.10.3 e NATS 2.15.3 foram testados em serviço real.                                                       | NACK/retry agendado, deduplicação pública, replay por cursor, shutdown, redrive atômico e reconexão pub/sub.                               |
| S4 — LanceDB/Qdrant                 | **Parcial.** Cosseno alinhado, limites/dimensões, coleção ausente e reinício Qdrant.                                                                                                   | Filtros, parâmetros ANN, quantização, busca híbrida, falhas de indexação, backup/snapshot e health.                                        |
| S5 — recuperação, contexto e adoção | **Parcial no recovery; planejada em contexto e adoção.** SQLite mantém recovery e a projeção PostgreSQL pode ser reconstruída.                                                         | Memória, Context Planner, RAG e adoção gradual por consumidores ainda não estão implementados.                                             |

A matriz por contrato e binding está em
[`02-matriz-conformidade-capabilities.md`](../analyses/02-matriz-conformidade-capabilities.md).

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

## Escopo e critérios de aceitação por fase

As fases são ordenadas por dependências técnicas. Os critérios abaixo
orientam a conclusão das capabilities; a posição da stack de entrega é dinâmica
e consultada separadamente no GitHub.

### 1. S0 — contratos e conformidade inicial

Inventariar operações e consumidores no backend Python e no GH Bot; definir
tipos versionados, escopo, erros, health/capabilities, cancelamento, timeout,
shutdown, cursor e limites. Especificar separadamente ledger, SQL, vetores,
KV, filas e pub/sub; criar matriz de garantias e fixtures de conformidade.
Documentar as decisões de API e dependências Rust antes dos adapters. Expor os
contratos utilizáveis pelos bindings Python e TypeScript na mesma etapa.

**Aceite:** contratos reproduzíveis e versionados, seleção independente,
erros e capabilities observáveis; nenhuma capacidade não implementada é
anunciada como disponível. Identificar explicitamente transações exigidas pelo
ledger e a fronteira entre estado persistido e transporte de eventos.

### 2. S1 — SQLite e ledger vertical

Implementar migrations próprias da Engine, conexão/pool e lifecycle; admissão
idempotente, append com versão esperada e sequência por agregado, leitura por
cursor e checkpoints. Expor o mesmo núcleo pelos dois bindings e exercitar um
fluxo mínimo do GH Bot. Avaliar cancelamento, concorrência e recuperação com
processos independentes; não migrar dados de produto nesta fase.

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

- Testar o contrato Rust e os bindings Python/TypeScript na mesma etapa; fixtures
  comuns não substituem testes de serviços reais e falhas de rede/processo.
- Verificar migrations, compatibilidade, concorrência, limites, redaction,
  isolamento, observabilidade, shutdown e recuperação aplicáveis a cada capability.
- Medir latência, throughput e memória por SHA, ambiente, versão do backend,
  configuração e carga. Não alegar paridade ou ganho sem evidência.
- Revisar instalação, empacotamento e plataformas suportadas antes de publicar;
  segredo e credenciais nunca entram em logs, fixtures ou exportações.

## Evidência e operação

Graph storage FalkorDB é uma capability adicional fora dos cinco pares de
equivalência; não deve ser contabilizado como paridade de storage. O contrato e
seus critérios estão em [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md).

A execução mais recente dos testes service-real e as versões usadas constam na
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md). Resultados
locais são registrados por comando e ambiente; CI remoto não os substitui.

## Decisões

Já decididas por `brunosrz`: SQLite como autoridade local e PostgreSQL como
projeção; backup online e offline coordenado; sem limite novo de payload; TTL
com NATS 2.11+; filas com retenção finita (100000), cinco tentativas e DLQ;
equivalência por par antes da especialização.

Ainda abertas: granularidade e versão dos crates; política de compatibilidade
de schemas e bindings; retenção e compactação do ledger; envelope de job;
mapeamento comum de índices vetoriais; formato estruturado de
`capabilities()`; perfil de deployment do primeiro fluxo do GH Bot.

**Decisão vital antes de filtros vetoriais:** LanceDB 0.22.3 armazena hoje
`metadata` como JSON em coluna `Utf8`; os filtros JSON documentados pelo Lance
operam sobre a coluna de extensão JSONB. Qdrant já aceita filtros sobre payload.
Para implementar o mesmo filtro antes de limitar os resultados, a Engine precisa
mudar o formato persistido da metadata LanceDB. Proposta de API comum: expressão
booleana tipada sobre caminhos de campos e valores escalares JSON; escopo
continua sempre aplicado pela Engine. Opções para coleções LanceDB existentes:

1. migrar automaticamente cada coleção ao abrir, reescrevendo e verificando
   todas as linhas;
2. expor uma operação explícita de migração, com verificação e relatório, e
   rejeitar busca filtrada até a coleção ser migrada;
3. declarar o formato anterior incompatível e exigir recriação da coleção.

A opção 2 preserva controle operacional e evita reescrita inesperada de grandes
coleções. A decisão de compatibilidade e impacto sobre dados pertence ao
mantenedor; não iniciar a migração persistente antes de resposta no chat.

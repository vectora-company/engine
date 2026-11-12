# Plano de implementação: storage, ledger e mensageria

## Objetivo e fontes

Criar capacidades reutilizáveis para persistência, ledger, busca vetorial, KV,
filas e pub/sub na Engine Rust. O GH Bot TypeScript é o primeiro consumidor;
o Vectora Python é referência de comportamento e consumidor posterior. Cada
capacidade pública entrega Rust, bindings Python e TypeScript e conformidade no
mesmo PR. A publicação em crates.io, PyPI e npm tem seus próprios gates.

Fontes: `../issues/01-loop-agentico-e-harness.md`,
`../issues/02-storage-memoria-contexto-rag.md`,
`../issues/04-chat-streaming-e-threads.md`,
`../issues/08-contratos-backend-e-engine-rust.md` e
`../issues/10-avaliacao-benchmarks-e-estudos.md`; consulte também
`../vectora/backend/storage/`, `../vectora/backend/persistence/native/` e
`../vectora/backend/scheduling/mq.py`. As issues contêm requisitos de produto:
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

## Matriz de equivalência por par (estado verificado em 2026-11-12)

Fonte: código dos adapters nas branches da stack e testes locais. "Lacuna"
significa que o outro lado consegue e a Engine ainda não entrega; "base comum
ausente" significa que nenhum dos dois lados expõe a capacidade hoje.

| Par                       | Lacuna (um lado tem, o outro não)                                                                                                                                                                                                                                                                                    | Base comum ainda ausente nos dois lados                                                                                                                                       |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SQLite × PostgreSQL       | PostgreSQL não tem os stores de lifecycle, loop e policy nem backup/restore/verify equivalentes. Decisão do mantenedor: SQLite é sempre a cópia local obrigatória e autoridade de recuperação do ledger e demais estados críticos internos; PostgreSQL é projeção/replicação retomável e nunca substitui essa cópia. | Interface SQL transacional tipada e limitada (spec 02) em ambos; completar stores PostgreSQL sem retirar a escrita SQLite-first.                                              |
| LanceDB × Qdrant          | A branch do PR #57 fixa `Cosine` no LanceDB e normaliza os dois resultados para similaridade `[-1, 1]` (maior é melhor); testes com vetores não unitários passam nos dois adapters. O PR #65 aplica o intervalo comum de 1 a 65536 dimensões em gravação e busca, antes de chamar cada backend.                      | Filtro por metadado além do escopo, criação/parâmetros de índice ANN, quantização, busca lexical/híbrida, backup/snapshot, health.                                            |
| Redis × NATS JetStream KV | A branch empilhada sobre #57 implementa TTL por mensagem em NATS e marca o bucket com `AllowMsgTTL`; exige NATS 2.11+. Teste real de create/update/expiração passou com NATS 2.15.3 e Redis 8.10.3. As imagens de teste do workspace foram atualizadas.                                                              | CAS por revisão, incremento atômico, listagem por prefixo e observação de mudanças.                                                                                           |
| Redis Streams × JetStream | PR #60 configura retenção por contagem (padrão 100000); a branch `feat/storage-queue-retries-dlq` limita a cinco entregas e move falhas à DLQ consumível nos dois. Rust service-real 14/14; bindings Python 50 e Node 35 passam. ACK confirma sucesso do job.                                                        | Shutdown gracioso e operação de redrive; o consumidor pode reenfileirar a mensagem via `enqueue`, sem atomicidade com o ACK da DLQ. NATS DLQ é at-least-once após publicação. |
| Redis Pub/Sub × NATS Core | Nenhuma lacuna de contrato conhecida (ambos efêmeros; mensagem sem assinante é descartada, provado por teste).                                                                                                                                                                                                       | Assinatura por padrão/curinga e reconexão observável.                                                                                                                         |

O relatório atual `capabilities()` enumera implementado/não implementado e
notas, mas não tem um campo estruturado para diferenciar lacuna, emulação e
especialização com justificativa de incompatibilidade. Essa parte do contrato
de descoberta também precisa ser definida e implementada antes de anunciar
especializações como suportadas.

O contrato vetorial compartilhado usa vetores não unitários para distinguir
Cosine de L2: com os mesmos dados, o ranking por norma L2 difere, enquanto os
dois adapters devem ordenar por similaridade cosseno. A implementação e os
testes estão na branch do PR #57, ainda não integrada em `main`. O PR #59 atualiza
dependências lockadas e adiciona TTL nativo de NATS KV. PR #60 adiciona retenção
finita comum por contagem às filas Redis/NATS. A branch empilhada atual limita
tentativas a cinco, envia falhas à DLQ consumível e expõe o recebimento da DLQ
nos bindings; shutdown gracioso permanece pendente.

## Estado revalidado contra `main` (`22c861f`)

As fases abaixo foram escritas como plano de implementação original. A tabela
separa o que já existe do que ainda precisa ser implementado ou provado; não
interprete uma fase marcada como implementada como pronta para produção.

| Fase                             | Estado atual                                                                                                                                                                                      | Trabalho que continua pendente                                                                                                         |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| S0 — contratos                   | **Parcial.** Seleção versionada independente e contratos de ledger/vector/messaging existem; seleções públicas rejeitam `memory`, que não tem adapter nem faz parte dos seis backends do produto. | Fixtures comuns entre bindings; capability/readiness observável; revisar diferenças e erros por adapter.                               |
| S1 — SQLite e ledger             | **Implementada experimentalmente.** Ledger, reserva/admissão, checkpoints, stores de lifecycle, loop e policy expostos em Rust/Python/TS.                                                         | Backup/restore, concorrência multi-processo e recovery operacional com evidência; completar conformidade compartilhada.                |
| S2 — PostgreSQL                  | **Parcial.** `PostgresLedger` e projeção via `ResilientLedger` existem; SQLite continua autoridade local do append resiliente.                                                                    | Não existe SQL genérico nem store Postgres para lifecycle/loop/policy; provar serviço real, conflitos, restart e recovery.             |
| S3 — Redis/NATS                  | **Implementada experimentalmente.** KV, Redis Streams/NATS JetStream queues e Redis/NATS Core pub/sub existem.                                                                                    | Testes integrados comuns de desconexão, restart, redelivery, ACK, retention, TTL e shutdown.                                           |
| S4 — LanceDB/Qdrant              | **Implementada experimentalmente.** `VectorStore` tem adapters de LanceDB e Qdrant e bindings.                                                                                                    | Cobrir filtros/isolamento e falhas contra serviços reais; explicitar diferenças de score e limites sem alegar equivalência.            |
| S5 — recuperação/contexto/adoção | **Parcial no recovery; planejada em contexto/adoção.** Run/loop têm persistência/reconciliação SQLite.                                                                                            | Efeitos externos seguem sob o host; Memory/Context Planner/RAG e integração com GH Bot/Vectora não existem como capabilities adotadas. |

A matriz detalhada por contrato e binding está em
[`02-matriz-conformidade-capabilities.md`](../analyses/02-matriz-conformidade-capabilities.md).

Os contratos de vetores (`VectorStore`) e KV (`KvStore`) têm adapters que
selecionam uma implementação por configuração. Filas duráveis e pub/sub também
possuem APIs distintas (`DurableQueue` e `EphemeralPubSub`): a fila entrega um
handle que deve receber ACK após o processamento; liberar o handle sem ACK
mantém a possibilidade de redelivery conforme o visibility timeout. O binding
retorna bytes sem reinterpretar o payload. Handles de entrega e assinaturas são
locais ao processo e não devem ser persistidos em configuração.

Redis Streams recupera entregas pendentes com `XAUTOCLAIM`; NATS JetStream usa
consumidores duráveis e `ack_wait`. KV Redis aceita TTL por chave; JetStream KV
não implementa o mesmo contrato de TTL e retorna erro `Unsupported` quando ele
é solicitado em `main` (`22c861f`), o que é uma lacuna de equivalência corrigida
na branch empilhada atual. LanceDB reporta distância
(_menor é mais próximo_, métrica padrão L2); Qdrant reporta similaridade cosseno
(_maior é mais próximo_). Esse é o estado atual, não o contrato desejado: a
meta é alinhar a métrica e normalizar o score para um contrato único. Até lá,
não comparar os scores entre adapters.

No ledger, `ResilientLedger` grava evento e outbox atomicamente em SQLite antes
de tentar a projeção PostgreSQL. O resultado informa `synchronized`, `pending`
ou `conflict`; um conflito de projeção não desfaz o evento local. Retentativas
devem reutilizar a mesma chave de idempotência. O append do binding usa esse
contrato SQLite-first; não abrir um ledger PostgreSQL isolado para substituir a
autoridade local.

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

## Revalidação do loop em 2026-11-12

PRs empilhados ainda sujeitos a revisão, não incorporados em `main`:

- #46: relatório estático de capabilities e binding TypeScript.
- #47: matriz service-real de Redis, NATS, Qdrant e PostgreSQL.
- #48: reinício de Redis/NATS e queda/retorno PostgreSQL.
- #49: concorrência do ledger entre processos independentes.
- #51: erro para coleção vetorial ausente e reinício do Qdrant, com resultado
  atualizado na auditoria.

Os itens de capabilities, testes de storage service-real, restart de Redis/NATS,
recovery da projeção PostgreSQL (teste isolado) e concorrência do ledger estão
implementados nas branches da stack e têm os resultados registrados em
[`05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).
Não são ainda capacidades integradas em `main`. Backup/restore do ledger (online e offline coordenado) está implementado após decisão do mantenedor; restam shutdown gracioso, retenção/limites JetStream e limites/falhas de
indexação vetorial. A execução combinada inicial de 55 testes teve uma falha
porque `podman stop` removeu o PostgreSQL auto-removível; após recriar o
container sem remoção automática e proteger o teste, a execução mais recente
passou 58/58 com os quatro serviços e reinícios habilitados.

## Decisões a fechar durante S0

Definir APIs assíncronas e formato de configuração sem impor framework; versão
e granularidade dos crates; política de compatibilidade dos schemas e bindings;
versão esperada/escopo dos agregados; retenção, compactação e cursores; limites
e estratégia de migração; requisitos reais de entrega, outbox e DLQ; e perfil
de deployment do primeiro fluxo do GH Bot. Essas decisões dependem do inventário
e de provas de conformidade, não devem ser presumidas por similaridade de API.

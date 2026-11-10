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
- Ledger é a autoridade persistida do runtime quando configurado; índices,
  filas e notificações podem ser projeções ou transporte. Pub/sub efêmero e
  filas duráveis são contratos distintos.
- Especificar capabilities e diferenças reais de consistência, ordenação,
  concorrência, retenção, TTL, ACK e replay. Memória é opt-in, identificada
  como não durável; indisponibilidade nunca troca silenciosamente o backend.
- Erros tipados, timeouts, cancelamento, limites e observabilidade sem segredos
  fazem parte de cada API. Sem dual-write entre implementações nem replay
  automático de efeito externo cujo resultado é desconhecido.

## Matriz de capacidades

| Capacidade | Adapters | Diferenças a declarar |
| --- | --- | --- |
| Relacional e ledger | SQLite, PostgreSQL | Transações, isolamento, locks, conflitos, migrations, tipos e paginação. |
| Vetorial | LanceDB, Qdrant | Filtros, distância/score, índices, busca lexical/híbrida, consistência e limites. |
| KV/cache | Redis, NATS JetStream KV | TTL, atomicidade, revisões, eviction, limites e persistência configurada. |
| Fila/stream durável | NATS JetStream, Redis Streams | Grupos/consumidores, ACK, redelivery, retenção, replay, ordenação e DLQ. |
| Pub/sub | NATS Core, Redis Pub/Sub | Entrega efêmera, reconexão, assinaturas e comportamento sem assinante. |

Adapters em memória podem apoiar testes e modo local explicitamente escolhido,
sem reivindicar durabilidade ou semântica distribuída. Garantias dependem da
configuração do serviço; não inferir durabilidade apenas pelo nome do backend.

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

### 3. S2 — PostgreSQL e conformidade SQL

Implementar o contrato de S1 com transações, pool e migrations próprios de
PostgreSQL. Reutilizar fixtures de contrato e acrescentar testes concorrentes
com serviço real, inclusive conflito de versão, locks, falha e restart. Expor
configuração e lifecycle nos dois bindings.

**Aceite:** mesmas garantias obrigatórias de ledger que SQLite; divergências
de isolamento e operação documentadas. Paridade é limitada aos casos medidos.

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
indexação. Resultados não reivindicam equivalência numérica entre backends.

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

## Decisões a fechar durante S0

Definir APIs assíncronas e formato de configuração sem impor framework; versão
e granularidade dos crates; política de compatibilidade dos schemas e bindings;
versão esperada/escopo dos agregados; retenção, compactação e cursores; limites
e estratégia de migração; requisitos reais de entrega, outbox e DLQ; e perfil
de deployment do primeiro fluxo do GH Bot. Essas decisões dependem do inventário
e de provas de conformidade, não devem ser presumidas por similaridade de API.

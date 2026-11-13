# Plano de continuidade da Vectora Engine

**Estado revalidado:** 12 de novembro de 2026
**Referência remota:** branch `main`, commit `a13140b9f85ba8c06a7412a7110a5292b8220e56`
**Objetivo:** transformar as capabilities experimentais já implementadas numa
engine distribuível e adotável com segurança, sem substituir o backend Python
nem presumir paridade.

## Resumo executivo

A engine já contém adapters de chat, embeddings/rerank, storage/mensageria e um
Harness experimental. Desde a fotografia anterior, o main recebeu storage e
ledger, lifecycle e loop SQLite duráveis, gates de verificação, cancelamento com
reconciliação e policy/aprovação persistidas. Os bindings Python e
TypeScript/Node expõem essas capacidades do mesmo core.

O bloqueio para adoção é agora a maturidade operacional e a comprovação de
conformidade: efeitos externos continuam sob execução do host, o SQLite não
torna esses efeitos atômicos, e não há integração comprovada no Vectora App nem
no GH Bot. A maior parte das APIs continua experimental. A revisão dos cinco
findings históricos do PR #3 foi revalidada; veja
[`01-auditoria-review-pr3.md`](../analyses/01-auditoria-review-pr3.md).
O inventário de APIs, bindings, testes e gaps está na
[`matriz de conformidade`](../analyses/02-matriz-conformidade-capabilities.md);
seu checklist específico está em
[`02-matriz-conformidade-capabilities.md`](../checklists/02-matriz-conformidade-capabilities.md).

O app consumidor segue no backend Python e não declara dependência nem uso de
`vectora-engine` nos manifests ou caminhos de runtime examinados. Portanto, o
status atual é **capability implementada na engine, ainda sem adoção comprovada
no Vectora App**. A continuidade deve priorizar contratos conformes,
durabilidade/autorização do Harness e um piloto estreito e reversível; não uma
migração ampla nem mais superfície de provider.

## Estado observado

| Área                       | Estado em `main`                                                                                           | Limite relevante                                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Core e distribuição local  | Workspace Rust `0.1.0`; `scons install`, builds focados e cache Cargo separados.                           | Publicação versionada dos crates/bindings não está comprovada.                                                                |
| Chat e modelos             | OpenAI-compatible, Anthropic, Google, Cohere, fallback, embeddings e rerank nos bindings.                  | Contratos experimentais; seleção, credenciais e fallback de produto pertencem ao consumer.                                    |
| Storage e mensageria       | SQLite/PostgreSQL, LanceDB/Qdrant, NATS/Redis; ledger SQLite-first e fila/pub-sub com contratos distintos. | Os pares devem ter a mesma base funcional; as lacunas atuais estão na matriz de equivalência de `04-plano-storage-ledger.md`. |
| Lifecycle e loop           | Stores SQLite persistem admissão, transições, eventos/checkpoints do loop e recuperação.                   | Não dão exactly-once a efeitos externos nem autenticam o host.                                                                |
| Verificação e cancelamento | Reports/gates e pedido de cancelamento são duráveis; resultados incertos exigem reconciliação.             | O host executa checks, interrompe adapters e comprova seu término.                                                            |
| Policy e aprovação         | `SqliteToolPolicyStore` persiste snapshots, pedidos e recibos, com verificação de estado/binding.          | A autenticação do aprovador e a execução da tool permanecem no host.                                                          |
| Consumidores               | Backend Python do Vectora e GH Bot seguem como projetos consumidores.                                      | Não foi encontrado uso de `vectora-engine` nos manifests/caminhos atuais examinados.                                          |

## Fotografia de entregas

O histórico em `main` inclui o bootstrap e bindings (#1–#14), storage e ledger
(#15), policy/aprovação duráveis (#16) e as etapas posteriores de lifecycle,
verificação, cancelamento, build e persistência do Harness até o PR #30. A lista
e os detalhes por capability estão no walkthrough e nos planos específicos; o
número do PR não substitui evidência de código nem de conformidade.

O PR #3 teve cinco observações de review que permaneceram sem resolução formal.
Elas foram revalidadas contra o `main` atual e classificadas em
[`01-auditoria-review-pr3.md`](../analyses/01-auditoria-review-pr3.md). Três
foram reproduzidas e corrigidas nesta etapa; duas já estavam corrigidas em
`main`. A ordem S0-S5 de storage em `04-plano-storage-ledger.md` precisa ser
reconciliada com os adapters já presentes antes de qualquer etapa ser descrita
como pendente.

## Plano priorizado

### 0. Fechar a fotografia de conformidade e review

1. **Review do PR #3:** findings classificados; três correções e cobertura
   local estão nesta branch, duas correções já estavam em `main`. Resultado
   final e comandos ficam na análise/checklist desta etapa.
2. **Matriz:** inventário por capability criado; fixtures de run, verificação,
   storage, fallback, policy, loop e ledger implementadas e executadas nos três
   bindings. Cancelamento explícito da iteração de stream também está coberto.
   O teste de pacote Node passou com addon gerado pelo comando padrão
   `napi build --platform --esm --manifest-path native/Cargo.toml`; a tentativa
   anterior com `--profile dev` apontava para o diretório de saída incorreto.
   A análise de cobertura
   service-real identificou testes condicionais e ausência de matriz integrada
   de falha/restart nos quatro serviços. A auditoria de fallback não encontrou
   divergência; a matriz não presume paridade.
3. **Estado:** etapa da matriz concluída com os limites acima documentados.
   Não foram usados serviços remotos nem CI como evidência.
4. Semântica de fallback foi comparada com implementação e testes locais sem
   divergência reproduzida; manter gaps de cobertura explícitos e não atribuir
   alterações desta branch ao comportamento de `main`.

**Saída:** gaps atuais reproduzíveis, feedback de review triado, fixtures
compartilhadas e uma linha de base de conformidade para cada binding. Concluída
nesta branch, com instalação limpa Node e integração service-real permanecendo
sem comprovação.

### 1. Auditar maturidade de storage e ledger

O PR #15 implementou contratos e adapters SQLite/PostgreSQL, LanceDB/Qdrant,
NATS/Redis, filas/pub-sub e ledger SQLite-first. O plano
[`04-plano-storage-ledger.md`](04-plano-storage-ledger.md) ainda conserva S0-S5
como propostas e precisa registrar, item a item, o que está implementado e o
que falta provar. Confrontar testes e adapters reais com garantias de
consistência, concorrência, restart, retenção, redaction, limites e falhas.
Não reimplementar adapters existentes nem assumir que sua presença comprova
paridade operacional.

**Saída:** matriz de cobertura por adapter/contrato e lista de gaps reproduzidos
que orienta o próximo trabalho de storage.

### 1b. Equivalência dos pares de storage (base antes da especialização)

Política em [`docs/specs/02-storage-memoria-contexto-rag.md`](../specs/02-storage-memoria-contexto-rag.md):
tudo que existe nos dois lados de SQLite/PostgreSQL, LanceDB/Qdrant, Redis/NATS
KV, Redis Streams/JetStream e Redis Pub/Sub/NATS Core é implementado nos dois,
com a mesma semântica observável e a mesma suíte de conformidade; só depois cada
backend se especializa no que o outro não consegue por incompatibilidade
verificada. As lacunas atuais estão na matriz do plano de storage. Ordem
sugerida, cada item em PRs empilhados: (1) métrica e score alinhados no par
vetorial; (2) TTL no NATS KV; (3) `max_deliver`/DLQ nas duas filas; (4) stores de
lifecycle, loop e policy e backup no PostgreSQL; (5) base comum ainda ausente
nos dois lados (filtros de metadado, índices/quantização, CAS, prefixo).

**Saída:** matriz de equivalência sem lacunas na base e com cada especialização
justificada por incompatibilidade.

### 2. Completar a integração host/runtime de policy e aprovação

Os contratos e store SQLite existem experimentalmente; o passo restante é
provar a integração do host que recompila policy, autentica cada recibo,
recarrega estado atual após restart e revalida imediatamente antes de efeitos.
Separar estado do run, resultado da verificação e veredito do gate.

O core não deve autenticar usuários nem importar RBAC do Vectora. O consumer
continua responsável por identidade, policy de produto e adapters. Capability
ausente, policy indisponível e contexto alterado devem falhar fechado segundo
contratos testáveis.

**Saída:** nenhuma tool com efeito externo é executável com base apenas em
capabilities do modelo ou em snapshot não confiável.

### 3. Completar cancelamento e recovery no host antes de efeitos

O core persiste pedido e estado de reconciliação, mas o host precisa propagar
cancelamento a providers/processos/adapters, confirmar término ou registrar
resultado incerto e retomar sem repetir efeito. Separar cancelamento de run e
cancelamento de assinante/stream; impor timeout no ponto de execução.

**Saída:** testes com falha parcial, restart, timeout, cancelamento, resultado
`unknown` e reconciliação sem duplicação de efeitos.

### 4. Integrar um piloto reversível no Vectora App

Primeiro, sincronizar a branch de integração do app. Adotar uma única capability
por vez e manter Python como caminho padrão/fallback até o gate de operação.
Um piloto de chat pode usar o binding Python com a montagem de turno, sem
executar tools: flag opt-in, fixtures locais compartilhadas, decisão de
provider/credenciais no consumer e rollback direto para o client atual.

Não chamar Python e Rust em paralelo contra providers reais para “comparar”:
isso duplica custo/efeitos. Comparar por fixtures/cassettes redigidos e
workloads sintéticos isolados. Não remover o runtime Python nem mover settings,
fallback de produto, telemetria, RAG ou regras de negócio para a engine.

Depois que storage, policy e recovery passarem seus gates, avaliar uma
capability de Harness com tools read-only. Tools com efeitos destrutivos ficam
fora do piloto inicial.

**Saída:** consumidor identificado, comportamento compatível nos casos
aprovados, métricas e rollback demonstrados, sem declarar paridade global.

### 5. Integrar o GH Bot pela binding local; publicar depois

O GH Bot é o primeiro ambiente de validação experimental da Engine e deve ser
desenvolvido em conjunto com ela, não aguardar a publicação de uma versão npm.
Durante o desenvolvimento, consome `../engine/bindings/typescript` pelo
workspace local `link:`. Para cada capability necessária ao Bot, validar o
carregamento do addon Rust local, implementar lacunas no core/binding e ligar o
fluxo no Bot dentro da mesma stack. O Bot permanece independente do Vectora
App e mantém a integração GitHub e a lógica de produto; não cria um runtime
paralelo em TypeScript.

Os testes experimentais devem usar uma conta GitHub nova e repositórios de
teste isolados. Credenciais e repositórios de produção ficam fora do ambiente.
Publicação em crates.io, PyPI e npm é uma etapa de distribuição separada e não
bloqueia o desenvolvimento conjunto. Não anunciar capacidades sem contratos e
cobertura correspondentes.

**Saída:** fluxos do GH Bot exercitam a binding local da Engine com erros,
cancelamento, persistência e efeitos GitHub cobertos conforme cada capability;
as falhas são atribuídas ao core, binding, configuração do Bot ou serviço
externo, sem duplicar efeitos.

### 6. Medir, amadurecer e expandir

Após o piloto, criar evidência reproduzível por SHA, ambiente, configuração,
dataset/fixture e comando. Medir latência, memória, throughput, custo,
qualidade, recuperação e falhas por capability. Separar `passed`, `failed`,
`blocked`, `skipped` e `not_run` da maturidade da API. Usar essa evidência para
decidir se amplia adoção, começa adapters de storage adicionais ou trabalha em
mídia/STT/TTS; não antecipar esses ports por semelhança de diretório.

## Gates de adoção e release

Uma capability só avança de experimental quando:

- contratos e erros são versionados, documentados e testados nas entradas
  aplicáveis;
- timeout, cancelamento, redaction e falhas/degradação têm resultado observável;
- concorrência/restart e idempotência são testados quando houver efeitos;
- o fallback e rollback do consumidor são testados sem duplicar efeitos;
- métricas são reproduzíveis e limitadas ao workload que realmente mediram;
- os consumidores e limites de compatibilidade estão identificados.

## Próxima entrega recomendada

O PR #65 unifica a validação de dimensões de busca e gravação em LanceDB e
Qdrant. O core compilou. Ainda falta verificar os extremos em execução.
As próximas lacunas independentes incluem CAS por revisão, incremento,
prefixo e watch nos dois KV stores, além de filtros e índices vetoriais.
Em filas, faltam shutdown gracioso e redrive atômico. Os cenários de falha
da DLQ, recebimento e preservação de configuração dos PRs #62–#64 ainda
precisam de validação com serviço real.

Veja [`docs/plans/04-plano-storage-ledger.md`](04-plano-storage-ledger.md),
[`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md)
e [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).

## Nota de proveniência local

O walkthrough histórico registra entregas até o commit `43a2655`. A fotografia
de `main` nesta revisão está em `a13140b`; as branches dos PRs abertos são
estado de trabalho separado. Consulte a auditoria do PR #3 e os planos
específicos para evidência posterior. Nenhum estado de CI remoto é usado como
evidência nesta atualização.

## Revalidação do loop em 2026-11-12

A referência de `main` foi consultada no remoto em 2026-11-12 (`a13140b`); as
alterações abaixo estão em PRs abertos e não são atribuídas a `main`. A stack
ativa, da base ao topo, é #45 → #46 → #47 → #48 → #49 → #51 → #52 → #53 → #54
→ #55 → #56 → #57 → #59 → #60 → #61 → #62 → #63 → #64 → #65.

| PR                                                       | Entrega na branch do PR                                                             |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [#45](https://github.com/vectora-company/engine/pull/45) | Snapshot do runtime TypeScript.                                                     |
| [#46](https://github.com/vectora-company/engine/pull/46) | Relatório estático de capabilities de storage e testes do ledger.                   |
| [#47](https://github.com/vectora-company/engine/pull/47) | Matriz de integração service-real para storage.                                     |
| [#48](https://github.com/vectora-company/engine/pull/48) | Recuperação de Redis/NATS após restart e projeção PostgreSQL durante queda/retorno. |
| [#49](https://github.com/vectora-company/engine/pull/49) | Concorrência do ledger SQLite entre processos independentes.                        |
| [#51](https://github.com/vectora-company/engine/pull/51) | Restart do Qdrant e erro explícito quando a coleção não existe.                     |
| [#52](https://github.com/vectora-company/engine/pull/52) | Limites de entrada dos stores de storage.                                           |
| [#53](https://github.com/vectora-company/engine/pull/53) | Backup/restore do ledger SQLite: cópia online e backup coordenado pelo host.        |
| [#54](https://github.com/vectora-company/engine/pull/54) | Caracterização da retenção e redelivery atuais das filas.                           |
| [#55](https://github.com/vectora-company/engine/pull/55) | Política de equivalência funcional dos pares e matriz de lacunas.                   |
| [#56](https://github.com/vectora-company/engine/pull/56) | Loop operacional, stack de PRs e regras de decisão/evidência.                       |
| [#57](https://github.com/vectora-company/engine/pull/57) | Alinhamento do ranking vetorial por cosseno e normalização do score.                |
| [#59](https://github.com/vectora-company/engine/pull/59) | Atualização das dependências e TTL nativo por chave no NATS JetStream KV.           |
| [#60](https://github.com/vectora-company/engine/pull/60) | Retenção finita configurável por quantidade em Redis Streams e NATS JetStream.      |
| [#61](https://github.com/vectora-company/engine/pull/61) | Teto de tentativas, DLQ consumível e bindings Python/TypeScript para filas.         |
| [#62](https://github.com/vectora-company/engine/pull/62) | Recuperação de falha na transferência para a DLQ em Redis Streams e NATS JetStream. |
| [#63](https://github.com/vectora-company/engine/pull/63) | Recebimento de uma mensagem por chamada no JetStream, com expiração no broker.      |
| [#64](https://github.com/vectora-company/engine/pull/64) | Preservação da configuração de streams e consumidores JetStream existentes.         |
| [#65](https://github.com/vectora-company/engine/pull/65) | Limite comum de dimensões para gravar e buscar vetores em LanceDB e Qdrant.         |

O teste service-real combinado de storage passou 58/58 com Redis, NATS,
Qdrant e PostgreSQL locais e os reinícios habilitados, conforme os registros
em `04-plano-storage-ledger.md` e `05-auditoria-storage-s0-s5.md`. Isso é
resultado local das branches, não integração em `main` nem aprovação de CI.
Backup/restore SQLite (online e offline coordenado) e ausência de limite novo
para payload são decisões de `brunosrz` já implementadas nos PRs indicados.

O GH Bot ainda monta manualmente em
[`app/src/server/engine.ts`](../../../gh-bot/app/src/server/engine.ts) os
contract versions que lê da binding local. A branch local da Engine expõe
`storageCapabilities()`, mas o Bot ainda não usa um snapshot agregado como
`engineContractSnapshot()`; a substituição pertence a um PR próprio no
repositório do Bot e fica fora desta stack da Engine.

As decisões sobre TTL, política de filas e autoridade do ledger foram recebidas
no chat em 2026-11-12. PR #59 implementa atualização de dependências e TTL NATS.
PR #60 implementa limite finito por quantidade para Redis/NATS (100000 por
padrão). O PR #61 implementa cinco entregas por padrão, DLQ consumível e
APIs nos bindings. Shutdown gracioso e redrive atômico continuam como etapas
seguintes aprovadas, sem depender de nova resposta.

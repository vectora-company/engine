# Auditoria: storage de grafos FalkorDB

## Evidência local

Verificação inicial em 2026-11-14 no checkout `/home/brunosrz/Área de trabalho/engine`, então em `feat/llama-server-bindings`. A implementação está em `feat/falkordb-graph-storage`, filha do PR #101 (`docs/falkordb-graph-contract`); o stack atual é #97 → #98 → #100 → #101 → #102.

- `crates/vectora-engine/src/storage.rs`: enumera `SqlBackend`, `VectorBackend`,
  `KvBackend`, `QueueBackend` e `PubSubBackend`; não existe backend graph.
- `crates/vectora-engine/src/storage/capabilities.rs`: relatório estático dos
  cinco tipos citados.
- bindings têm validação e reporte de seleção encaminhados ao Rust, mas não
  expõem graph.
- Spec 02 define cinco pares de equivalência; FalkorDB não é membro de nenhum.
- Na verificação inicial, #100 era o topo da stack. O contrato FalkorDB foi registrado no #101 e a implementação está no #102, filho do #101.
- Working tree contém mudança anterior do usuário em
  `bindings/typescript/native/index.d.ts`; não deve ser incorporada.

## Evidência upstream

- `GRAPH.QUERY` recebe nome do grafo, query e timeout opcional; parâmetros
  nomeados são suportados e recomendados. O protocolo é Redis e a linguagem é
  baseada em OpenCypher, com suporte parcial/extensões; não se deve presumir
  compatibilidade total com Neo4j.
  Fontes: [comando oficial](https://falkordb.github.io/docs-staging/commands/graph.query/)
  e [restrições listadas no repositório oficial de docs](https://github.com/FalkorDB/docs/blob/main/docs.json).
- Cliente Rust oficial `falkordb` versão 0.11.2, consultada em 2026-11-14,
  licença MIT e MSRV 1.95, acima do `rust-version = 1.89` declarado pela Engine. Recursos `tokio`, `serde` e TLS são opt-in; embedded
  é separado e `embedded-bundle` inclui módulo SSPL, portanto nenhum modo
  embedded deve ser ativado automaticamente.
  Fontes: [README oficial](https://github.com/FalkorDB/falkordb-rs),
  [crate metadata](https://crates.io/crates/falkordb).
- Persistência/recovery dependem da configuração do servidor e volume. O teste
  upstream de restart não prova durabilidade para qualquer configuração.
  [Guia de persistência](https://falkordb.github.io/docs-staging/operations/persistence/).

## Decisão de API e MSRV respondidas

Em 2026-11-14, `brunosrz` escolheu as duas camadas: API portátil de property
graph mais extensão Cypher específica de FalkorDB. Também aprovou o cliente
oficial stable `falkordb` 0.11.2 e elevar o MSRV da Engine para Rust 1.95. O
ambiente local usa Rust 1.99.1. A implementação ativa está no PR
[#102](https://github.com/vectora-company/engine/pull/102), filho do #101; usa
Tokio, Serde e TLS Rustls sem habilitar embedded/SSPL.

## Riscos e controles propostos

- Queries arbitrárias podem ser caras: exigir timeout e limite de resposta;
  query parametrizada e sem retry cego em write de resultado desconhecido.
- Nome do grafo e credenciais não são identidade ou autorização: validar nome
  sintaticamente; consumer autentica/autoriza e guarda secrets.
- O dialeto não é todo OpenCypher/Neo4j: fixar versão de server, fixtures e
  operações suportadas; não alegar portabilidade sem conformidade.
- FalkorDB falar Redis não torna o backend KV equivalente ao Redis KV; separar
  seleção/capabilities.
- GraphRAG não é apenas armazenamento: ingestão, embeddings e extração por LLM
  ficam fora desta primeira etapa.

## Integração existente no consumer Vectora

PR [vectora#578](https://github.com/vectora-company/vectora/pull/578), head
`036c8b8feb0d0919f76187518bb90971660a09a4`, adiciona um destino opcional de
exportação do Context Graph do Workbench. A implementação foi conferida em
`backend/api/handlers/context_graph.py::push_graph_to_falkordb` e
`backend/context_graph/export.py::push_to_falkordb`; ela não adiciona uma
capability de storage à Engine.

### O que o PR já protege

- Exige confirmação explícita, limita o arquivo a 32 MiB, a 10 mil nós e 50
  mil relações e retorna erro sanitizado para falhas remotas.
- Guarda host, porta e nome do grafo nas configurações locais do usuário; as
  credenciais passam pelo cofre e a API devolve apenas indicadores de presença.
- Revalida o destino na gravação usando a política de egress/SSRF do workspace.
  O caminho de envio fixa o IP validado no socket e mantém o hostname para
  validação TLS.
- Usa `MERGE` e parâmetros para valores; labels e tipos dinâmicos de nó/relação
  são normalizados antes de compor o texto Cypher.
- Executa o cliente síncrono fora do event loop com `asyncio.to_thread` e fecha
  o cliente em `finally`.

Essas são defesas úteis para a ação de exportar dados do workspace a um serviço
configurado pelo usuário. Não fazem do exporter um adapter genérico de storage.

### Limites observados no diff

- O exporter envia uma query por nó e uma por relação. No teto documentado pela
  rota, isso pode significar até 60 mil chamadas sequenciais ao servidor; o PR
  não mede esse custo nem usa batch/pipeline.
- As gravações são independentes. Se a conexão cair no meio, o grafo remoto pode
  conter somente parte do snapshot; o endpoint não mantém checkpoint ou marca de
  sincronização para retomar ou detectar esse estado.
- `MERGE` faz upsert dos elementos recebidos, mas o exporter não remove nós e
  relações que desapareceram do snapshot nem apaga propriedades que deixaram de
  existir na origem (`SET +=`). Portanto, o comportamento observado é uma
  mesclagem aditiva, não espelhamento/sincronização integral do Context Graph.
- Os contadores retornados são incrementados após cada chamada individual; o
  diff não consulta o grafo remoto para confirmar contagens finais. Os testes
  adicionados usam clientes e conexões simulados, sem iniciar um serviço real.
- O SDK é importado sob demanda; a rota retorna `501` quando a dependência não
  está instalada. Não encontrei mudança de dependências ou evidência de teste
  com uma versão/imagem real do servidor neste PR.

O cliente oficial `falkordb-py` não repete writes por padrão. A documentação
alerta que um timeout pode ocorrer depois de a escrita executar; habilitar retry
de writes pode duplicar efeitos e só é seguro para queries idempotentes. O
exporter atual não configura política de retry, o que evita repetição cega, mas
não resolve a recuperação do envio parcial. Ver
[cliente oficial](https://github.com/FalkorDB/falkordb-py#sentinel-failover).

### Paridade com o storage da Engine

Não: a integração do Vectora não está no mesmo nível contratual dos adapters
SQLite, LanceDB e NATS da Engine, porque cumpre outra função e vive em outra
camada. É uma exportação de produto para FalkorDB, não uma store selecionável
por Rust/Python/TypeScript, não participa da suite de conformidade e não expõe
health, recuperação, isolamento de escopo ou capabilities no core. O facto de
o destino ser uma base de grafos não permite inferir paridade operacional.

Isso não significa que os adapters atuais da Engine tenham paridade operacional
completa ou maturidade de produção: as auditorias da própria Engine mantêm gaps
de integração com serviços reais, reinício, reconexão e recuperação. A diferença
observável é de estágio e contrato: SQLite, LanceDB e NATS já têm adapters Rust,
seleção/capabilities e testes de adapter; FalkorDB ainda é somente um exporter
específico do Vectora.

Também não se deve encaixar o FalkorDB no contrato vetorial atual só porque ele
tem índice de embeddings. A documentação do servidor descreve dimensões de
1–4096 e distância em que score menor significa mais próximo; o contrato da
Engine para LanceDB/Qdrant aceita até 65.536 componentes e normaliza similaridade
cosseno para `[-1, 1]`, maior sendo melhor. Se a Engine expuser busca vetorial
via FalkorDB, essas diferenças precisam de contrato, validação e testes próprios;
elas não substituem o papel de property graph.

Referências primárias: [índice vetorial FalkorDB](https://docs.falkordb.com/cypher/indexing/vector-index),
[GRAPH.QUERY](https://docs.falkordb.com/commands/graph.query),
[GRAPH.RO_QUERY](https://docs.falkordb.com/commands/graph.ro-query) e
[contrato vetorial da Engine](../specs/02-storage-memoria-contexto-rag.md).

### Conclusão sobre implementação

No Vectora, FalkorDB está implementado como destino opcional de exportação do
Context Graph, com os controles e limites descritos acima. O exporter é uma
integração de produto, não uma store selecionável nem prova de sincronização
completa. A Engine agora tem uma capability graph independente no PR #102:
core Rust, seleção/capabilities, API portátil de property graph, extensão Cypher
parametrizada e bindings Python/TypeScript. Não se declara paridade com os
adapters SQL, vector, KV, queue ou pubsub, nem suporte GraphRAG completo. O
FalkorDB vector index continua fora desse contrato devido às diferenças de
dimensão e ordenação de score descritas acima.

## Estado

O PR de implementação [#102](https://github.com/vectora-company/engine/pull/102)
foi atualizado sobre o head mais recente do #101 para resolver conflito neste
arquivo, preservando a análise do exporter do Vectora e incorporando o estado
da Engine. A implementação FalkorDB 6.0.3 foi testada antes da última atualização
de dependências: autenticação, Cypher parametrizado, Unicode, propriedades,
traversal, limites, timeout e restart/AOF síncrono. Na rodada posterior ao
update, `scons tests` passou com 196 testes Rust + 3 fixtures, 87 testes Python
(1 ignorado) e 40 testes Node (11 ignorados sem endpoints, incluindo FalkorDB).
`scons lint` também passou.

Em 2026-11-15, `scons update` atualizou 51 pacotes Cargo compatíveis com Rust
1.95 e atualizou Ruff 0.17.4→0.17.5 e ty 0.0.101→0.0.104; pnpm v11.5.0 já
estava atualizado. O `node_modules` foi reinstalado usando o store padrão local
`/home/brunosrz/.local/share/pnpm/store/v11`; SCons não consulta `.modules.yaml`
nem força o store externo. Após rebase no head atual do #101, `scons update`
também atualizou `xxhash-rust` 0.8.19→0.8.20. A simulação pós-update não encontrou atualizações aplicáveis;
15 versões mais novas permanecem fora da seleção compatível com o MSRV 1.95.

Naquele ponto da auditoria, TLS e cancelamento remoto ainda não tinham sido
testados. O follow-up abaixo cobriu TLS e investigou a API de cancelamento. A
mudança preexistente do usuário em `bindings/typescript/native/index.d.ts` foi
mantida fora dos PRs.

## Auditoria posterior: configuração de timeouts e limites nos bindings

Em 15/11/2026, a revisão do código no checkout da Área de trabalho encontrou
uma divergência real: `GraphStoreConfig` do core aceitava timeout de query e
limites de linhas/bytes, mas as funções NAPI/PyO3 reconstruíam a configuração
com apenas `endpoint` e `graph_name`. Também não se definiam deadlines de
conexão nem de resposta client-side no builder oficial.

A branch filha do PR #110 introduz `GraphStoreSettings`, usada diretamente por
Python e Node, com validação no core e defaults explícitos: connection 10 s,
query server-side 10 s, response client-side 30 s, 1.000 linhas e 4 MiB. Testes
Rust verificam defaults, overrides, opções desconhecidas e limites inválidos.
Com `falkordb/falkordb:6.0.3` em Podman, Rust, Python e Node aceitaram opções
customizadas; roundtrip e limite de linhas passaram. Python e Node rejeitaram
configuração inválida antes de abrir conexão. O teste TLS seguinte revelou que a dependência FalkorDB acessa Rustls sem selecionar provider: Lance/AWS ativa `aws-lc-rs` e outras dependências ativam `ring`, fazendo seleção implícita entrar em panic. A correção seleciona `ring` somente se o host ainda não instalou provider. FalkorDB 6.0.3 com certificado SAN assinado por CA de teste passou roundtrip, limites e timeout em Rust, Python e Node via `falkors://`; `SSL_CERT_FILE` forneceu a CA ao trust store. O client oficial 0.11.2 não expõe API/handle de cancelamento remoto; cancelar a future não garante parar o comando. O timeout server-side é exercitado separadamente. Essa correção está no PR [#112](https://github.com/vectora-company/engine/pull/112), filho do #111; o GitHub reportou `MERGEABLE` na abertura.

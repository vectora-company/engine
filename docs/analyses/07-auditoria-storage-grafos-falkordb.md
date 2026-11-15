# Auditoria: storage de grafos FalkorDB

## Evidência local

Verificação em 2026-11-14 no checkout
`/home/brunosrz/Área de trabalho/engine`, branch
`feat/llama-server-bindings`:

- `crates/vectora-engine/src/storage.rs`: enumera `SqlBackend`, `VectorBackend`,
  `KvBackend`, `QueueBackend` e `PubSubBackend`; não existe backend graph.
- `crates/vectora-engine/src/storage/capabilities.rs`: relatório estático dos
  cinco tipos citados.
- bindings têm validação e reporte de seleção encaminhados ao Rust, mas não
  expõem graph.
- Spec 02 define cinco pares de equivalência; FalkorDB não é membro de nenhum.
- PRs abertos no momento: #97 em `main`, #98 filho do #97, #100 filho do #98.
  #100 é o topo; a etapa FalkorDB deve nascer como filho dele após resolver o
  contrato.
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

## Decisão de API respondida e decisão de MSRV pendente

Em 2026-11-14, `brunosrz` escolheu as duas camadas: API portátil de property
graph mais extensão Cypher específica de FalkorDB. A decisão sobre client e MSRV
ainda altera compatibilidade: o client oficial 0.11.2 requer Rust 1.95, enquanto
a Engine declara 1.89; 0.10.4 não declara MSRV e usa Redis client 1.2.2. O
toolchain 1.89 não foi instalado: o download iniciado ficou sem progresso e foi
interrompido. Decisão registrada em
[`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md);
perguntar no chat se elevar MSRV, validar 0.10.4 no 1.89, ou implementar o
protocolo com redis já presente (desviando da recomendação upstream). Até a
resposta, não adicionar dependência nem alterar MSRV.

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
Context Graph, com controles relevantes de destino, credenciais e confirmação.
O PR não comprova sincronização completa nem operação resiliente contra um
serviço real. Na Engine, FalkorDB ainda não está implementado: a seleção, o
adapter, os bindings e os testes de conformidade não existem em `main`. A
capability graph deve ser avaliada separadamente das stores SQLite, LanceDB e
NATS e não deve herdar suas garantias por analogia.

## Estado

Pesquisa factual, comparação do exporter do Vectora e proposta documental
registradas. Na Engine, nenhuma dependência foi adicionada, nenhum serviço foi
iniciado e nenhum teste de integração foi executado. A etapa de código continua
dependente da decisão vital descrita na spec; este documento não a decide.

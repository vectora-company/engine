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

## Estado

Pesquisa factual e proposta documental feitas. Dependência não adicionada,
serviço não iniciado e nenhum teste de integração executado. A etapa de código
está bloqueada pela decisão vital; não é um bloqueio externo.

# Storage de grafos com FalkorDB

## Estado

Implementação publicada no PR [#102](https://github.com/vectora-company/engine/pull/102), filho do #101. Uma auditoria posterior encontrou que os bindings descartavam os timeouts/limites do core; a etapa filha #110 expõe esses controles e configura os deadlines de conexão/resposta do client oficial. `brunosrz` escolheu as duas camadas e aprovou `falkordb` 0.11.2 com Rust 1.95. Core e bindings foram testados contra FalkorDB 6.0.3; TLS e cancelamento remoto seguem limitações documentadas. Não declarar paridade com SQL/vetores.

## Objetivo e fronteira

Adicionar à Engine uma capability selecionável de property graph usando
FalkorDB, para consumidores que precisam consultar relações e caminhos. É uma
capability `graph` independente: não substitui SQL, vetores, KV ou ledger, e
pode ser composta com vector store para recuperação/RAG. Ingestão de documentos,
extração de entidades por LLM, ontologias de produto, identidade/RBAC, UI e
visualização continuam no consumidor.

SQLite continua sendo a autoridade/cópia local obrigatória para ledger e estado
interno crítico da Engine. Dados de domínio armazenados pelo consumer em
FalkorDB não são automaticamente dual-written em SQLite; backup e persistência
do serviço são responsabilidade operacional do consumer.

## Fatos upstream verificados

- FalkorDB recebe comandos pelo protocolo Redis e executa OpenCypher por
  `GRAPH.QUERY`/`GRAPH.RO_QUERY`; não é Bolt nem SQL. O dialeto é um subconjunto
  com extensões próprias. [Documentação oficial do comando](https://falkordb.github.io/docs-staging/commands/graph.query/)
  e [instruções oficiais da documentação](https://github.com/FalkorDB/docs/blob/main/docs.json).
- Queries aceitam parâmetros e timeout de execução; parâmetros são preferíveis
  a interpolação textual. O client oficial mostra `query_with_params`.
- Existe crate Rust oficial `falkordb` 0.11.2 (verificado em 2026-11-14), com
  API Tokio, Serde e TLS opcionais; `rust-version = 1.95`. A Engine declara
  `rust-version = 1.95`; essa decisão de MSRV foi aprovada por `brunosrz`. Recursos embedded
  trazem implicações de download/binário e, no `embedded-bundle`, licença SSPL;
  a Engine não deve habilitar embedded por padrão. [Client e features](https://github.com/FalkorDB/falkordb-rs)
  e [crate](https://crates.io/crates/falkordb).
- A persistência depende da configuração/volume do deployment; o nome
  FalkorDB sozinho não prova que snapshots/AOF estão habilitados.
  [Guia oficial de persistência](https://falkordb.github.io/docs-staging/operations/persistence/).

## Contrato aprovado

`brunosrz` escolheu as duas camadas: contrato portátil de property graph mais
extensão Cypher específica de FalkorDB. Alternativas consideradas:

1. **API Cypher específica de FalkorDB:** cliente envia query OpenCypher
   parametrizada, nome de grafo, timeout e limites de resposta; máxima
   fidelidade, mas acopla consumidor ao dialeto e não oferece equivalência
   portável.
2. **API genérica de property graph:** operações tipadas para nós, relações,
   propriedades e traversal; mais portável, mas requer definir a semântica
   comum e não expõe todo o poder do FalkorDB.
3. **Duas camadas:** contrato portátil mínimo mais extensão explícita de query
   FalkorDB; mais flexível, com superfície e conformidade maiores.

O contrato portátil expõe operações tipadas de nós, relações e traversal; a
extensão FalkorDB expõe queries Cypher parametrizadas e delimitadas. A
implementação deve definir autenticação/TLS, validação do nome do grafo,
timeout, cancelamento, limite de linhas/bytes, erros, retry (sem retry cego de
writes), conexão/reconexão, readiness, observabilidade/redaction e suporte nos
bindings. Parâmetros são dados tipados, nunca interpolados pela Engine.

Bindings aceitam `endpoint`, `graph_name`, `connect_timeout_ms` (10.000 ms),
`query_timeout_ms` (10.000 ms, limite enviado ao servidor),
`response_timeout_ms` (30.000 ms, deadline client-side), `max_rows` (1.000) e
`max_result_bytes` (4 MiB). As opções são validadas no core; campos
desconhecidos são rejeitados. Descartar/cancelar a future interrompe a espera
local, mas não garante que FalkorDB interrompa um comando já recebido; o
timeout de execução do servidor limita esse trabalho quando respeitado pela
versão/configuração do serviço. O timeout client-side limita espera por
resposta e não deve ser interpretado como confirmação de rollback de escrita.

### Dependência e MSRV aprovados

`brunosrz` aprovou o client oficial stable `falkordb` 0.11.2 e elevação do MSRV global para Rust 1.95. O ambiente local usa Rust 1.99.1. A dependência foi adicionada com Tokio, Serde e TLS Rustls, sem embedded/SSPL. Revalidar versões stable antes de futuras atualizações.

## Equivalência e especialização

FalkorDB é um novo tipo de storage; não é um dos cinco pares existentes da
spec 02. Nenhum peer de graph foi identificado na Engine, logo não se declara
paridade nem especialização por incompatibilidade. Redis subjacente não torna
FalkorDB equivalente ao KV Redis: comandos e modelo de dados de grafo são
distintos. Qualquer backend de graph futuro deverá primeiro compartilhar a
base comum aprovada.

## Fora de escopo inicial

GraphRAG SDK, parsing/ingestão de corpus, geração de grafo por LLM, algoritmos
de domínio, embeddings, execução de código/procedures não aprovados e gestão
automática de serviço. Recursos vector/full-text nativos do FalkorDB não
substituem os adapters vectoriais existentes sem contrato de equivalência.

## Aceite após decisão

- Contrato Rust versionado com conexão e operações, erros tipados, timeout,
  cancelamento, limites e redaction.
- Seleção/capabilities e bindings Python/TypeScript coerentes com o core; testes passaram em Python (55) e Node (41; 10 testes de outros backends ignorados por falta de endpoints).
- Testes unitários e integração contra FalkorDB real. Cobertura executada em FalkorDB 6.0.3: auth via URL, parâmetros, dados não ASCII, substituição de propriedades, isolamento por nome de grafo, traversal nas três direções, limites, timeout, restart/recovery com AOF síncrono, delete e roundtrip nas três linguagens. TLS e cancelamento remoto não foram verificados; não há garantia de que descartar o future interrompa execução no servidor.
- Documentar versão/imagem, configuração de persistência testada e limites sem
  afirmar durabilidade operacional universal.

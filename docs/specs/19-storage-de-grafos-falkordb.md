# Storage de grafos com FalkorDB

## Estado

Proposta em análise; nenhum adapter de grafos existe na Engine. A escolha da
superfície pública está aguardando decisão de `brunosrz` no chat. Esta spec não
declara suporte implementado nem paridade com SQL/vetores.

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
  `rust-version = 1.89`; adotar o client exige elevar MSRV global, encontrar
  client compatível ou deixar a feature inacessível aos toolchains antigos.
  Esse impacto precisa ser decidido antes de incluir a dependência. Recursos embedded
  trazem implicações de download/binário e, no `embedded-bundle`, licença SSPL;
  a Engine não deve habilitar embedded por padrão. [Client e features](https://github.com/FalkorDB/falkordb-rs)
  e [crate](https://crates.io/crates/falkordb).
- A persistência depende da configuração/volume do deployment; o nome
  FalkorDB sozinho não prova que snapshots/AOF estão habilitados.
  [Guia oficial de persistência](https://falkordb.github.io/docs-staging/operations/persistence/).

## Contrato a decidir

A API ainda não está aprovada. Alternativas:

1. **API Cypher específica de FalkorDB:** cliente envia query OpenCypher
   parametrizada, nome de grafo, timeout e limites de resposta; máxima
   fidelidade, mas acopla consumidor ao dialeto e não oferece equivalência
   portável.
2. **API genérica de property graph:** operações tipadas para nós, relações,
   propriedades e traversal; mais portável, mas requer definir a semântica
   comum e não expõe todo o poder do FalkorDB.
3. **Duas camadas:** contrato portátil mínimo mais extensão explícita de query
   FalkorDB; mais flexível, com superfície e conformidade maiores.

Decisão bloqueante para a API pública: qual alternativa adotar? A implementação
deve também definir autenticação/TLS, validação do nome do grafo, timeout,
cancelamento, limite de linhas/bytes, erros, retry (sem retry cego de writes),
conexão/reconexão, readiness, observabilidade/redaction e suporte nos bindings.
Parâmetros de query devem ser dados tipados, nunca interpolados pela Engine.

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
- Seleção/capabilities e bindings Python/TypeScript coerentes com o core.
- Testes unitários e integração contra FalkorDB real, incluindo autenticação,
  parâmetros, dados não ASCII, timeout/cancelamento, erros, restart e isolamento
  entre nomes de grafo; sem service endpoint, teste real é registrado como não
  executado.
- Documentar versão/imagem, configuração de persistência testada e limites sem
  afirmar durabilidade operacional universal.

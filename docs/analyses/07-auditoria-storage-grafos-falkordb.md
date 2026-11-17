# Análise: storage de grafos FalkorDB

## Modelo upstream

FalkorDB recebe comandos pelo protocolo Redis e executa OpenCypher por
`GRAPH.QUERY`/`GRAPH.RO_QUERY`; não é Bolt nem SQL. O dialeto implementa um
subconjunto com extensões. Queries aceitam parâmetros nomeados e timeout. Fontes
primárias: [GRAPH.QUERY](https://docs.falkordb.com/commands/graph.query),
[GRAPH.RO_QUERY](https://docs.falkordb.com/commands/graph.ro-query) e
[documentação do cliente Rust](https://github.com/FalkorDB/falkordb-rs).

O client Rust oficial `falkordb` 0.11.2 foi escolhido com Tokio, Serde e TLS;
MSRV Rust 1.95 foi aprovado, embedded não é habilitado. Persistência depende de
configuração e volume; ver [guia upstream](https://docs.falkordb.com/operations/persistence/).

## Contrato observado na Engine

A Engine expõe uma capability graph independente com duas camadas: operações
portáteis de nodes, relations e traversal, além de Cypher parametrizada. Rust é
canônico; Python e TypeScript usam as configurações/validações do core. São
expostos connect/query/response timeout, limite de linhas e bytes, TLS Rustls,
redaction e erros tipados.

Testes reais contra FalkorDB 6.0.3 cobrem auth via URL, parâmetros, Unicode,
substituição de propriedades, isolamento por grafo, traversal, limites,
timeout, restart/AOF síncrono e TLS com CA confiável nos bindings exercitados.
Isso documenta uma configuração testada, não durabilidade universal. O client
não oferece API de cancelamento remoto: cancelar uma future interrompe a espera
local, mas não confirma que a query parou no servidor. Timeout server-side
também não prova rollback de uma escrita já recebida.

## Relação com o exporter do Vectora

O backend do Vectora contém um destino de exportação do Context Graph em
`backend/api/handlers/context_graph.py` e `backend/context_graph/export.py`.
Esse fluxo é específico do produto; não constitui adapter de storage genérico
da Engine.

O exporter exige confirmação explícita, valida limites de arquivo/nós/relações,
guarda credenciais no cofre e revalida o destino com política de egress. Usa
parâmetros para valores e executa chamadas bloqueantes fora do event loop.
Porém, envia uma query individual por nó/relação, sem batch medido; as escritas
são independentes e falha intermediária pode deixar snapshot parcial. `MERGE`
é aditivo: não remove dados ausentes na origem nem propriedades removidas. Os
contadores refletem chamadas emitidas, não reconciliação do grafo remoto. Logo,
não se deve descrever export como sincronização completa ou retomável.

As defesas são úteis para envio pontual do workspace a um destino do usuário,
mas não substituem o contrato da Engine com seleção, operações portáveis,
capabilities, health e conformidade.

## Equivalência e limites

FalkorDB não é um dos pares da spec de storage e não há peer graph atualmente
definido. Redis subjacente não torna property graph equivalente ao KV Redis.
Também não se deve usar o índice vetorial de FalkorDB como substituto direto de
LanceDB/Qdrant: dimensões, score e operações são diferentes; qualquer adapter
vetorial exigiria contrato e testes próprios.

GraphRAG não é apenas armazenamento. Ingestão, extração de entidades por LLM,
ontologias, identidade/RBAC, políticas de egress, UI, visualização, backup e
retenção do serviço pertencem ao consumidor ou a capabilities separadas.

## Riscos operacionais

- Limitar custo de query com parâmetros, timeouts e máximo de linhas/bytes; não
  repetir cegamente writes de resultado ambíguo.
- Validar nome do grafo e manter autenticação/autorização no consumidor.
- Registrar versão do serviço e operações suportadas; não alegar compatibilidade
  total com OpenCypher/Neo4j.
- Documentar separadamente timeout local, timeout servidor e confirmação de
  cancelamento; o último não está disponível no client usado.
- Configurar backup, volumes e persistência no deployment; os testes de restart
  só sustentam o AOF síncrono exercitado.

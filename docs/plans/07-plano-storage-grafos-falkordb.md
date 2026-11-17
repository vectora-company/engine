# Plano: capability de storage graph com FalkorDB

Plano por contrato e capacidade. Consulte dinamicamente GitHub apenas para
estado de entrega; branches e PRs não definem as fases deste documento.

## Objetivo e fronteira

Oferecer storage de property graph selecionável pela Engine, com contrato
portátil para nós/relações/traversal e extensão Cypher parametrizada de
FalkorDB. Não substitui SQL, vetores, KV, filas, pub/sub ou ledger. SQLite
continua autoridade/cópia local obrigatória do ledger e estado crítico; os
dados graph do consumidor não são automaticamente duplicados para SQLite.

## Estado no código

Implementados em Rust, Python e TypeScript: seleção/configuração, relatório de
capabilities, operações tipadas de graph, Cypher parametrizada, validação,
timeouts, limites de resultado, TLS e erros. Dependência é client oficial
`falkordb` 0.11.2 com Rust 1.95, sem recursos embedded.

Testes service-real foram executados contra FalkorDB 6.0.3, incluindo auth,
parâmetros, Unicode, propriedades, traversal, limites, timeout, TLS com CA
confiável e restart/AOF síncrono. A configuração testada não prova durabilidade
universal. Cancelar a future local não confirma cancelamento de uma query já
recebida pelo servidor.

Referências: [spec](../specs/19-storage-de-grafos-falkordb.md),
[checklist](../checklists/06-storage-grafos-falkordb.md) e
[análise](../analyses/07-auditoria-storage-grafos-falkordb.md).

## Critérios funcionais

- [x] Duas camadas: API portátil de property graph e extensão Cypher específica.
- [x] Configuração validada no core e encaminhada sem perda aos bindings.
- [x] Queries parametrizadas; nunca interpolar valores não confiáveis.
- [x] Timeouts separados de conexão, query no servidor e resposta local.
- [x] Limites de linhas/bytes, redaction de credenciais e errors tipados.
- [x] TLS Rustls sem sobrescrever provider já escolhido pelo host.
- [x] Testar serviço real com versão fixada e registrar setup de persistência.
- [ ] Adicionar teste automatizado de reconexão/recovery para política de
      persistência de serviço configurada, além do perfil AOF coberto.
- [ ] Revisar compatibilidade do dialeto conforme operações adicionadas;
      não anunciar compatibilidade total com OpenCypher/Neo4j.

## Limites e não objetivos

- Sem GraphRAG integrado, ingestão, extração de entidades por LLM, ontologias,
  identidade/RBAC ou UI.
- Sem promessa de exactly-once nem retry cego em writes.
- O client oficial não expõe confirmação de cancelamento remoto.
- Não declara equivalência com outro backend graph enquanto não houver um par
  e uma suíte comum definidos.
- O índice vetorial FalkorDB não substitui os contratos LanceDB/Qdrant: dimensão,
  ordenação de score e operações diferem.

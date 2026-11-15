# Plano: storage de grafos FalkorDB

## Objetivo

Adicionar uma capability graph à Engine em uma etapa empilhada após o PR #100,
sem alterar o escopo do adapter llama-server. Implementação Rust canônica,
bindings Python e TypeScript, testes contra FalkorDB real e documentação
operacional, após aprovação do contrato público.

## Estado inicial verificado em 2026-11-14

- Checkout `/home/brunosrz/Área de trabalho/engine`, branch
  `feat/llama-server-bindings`; PRs abertos #97 → #98 → #100, com #100 no topo.
- `crates/vectora-engine/src/storage.rs` seleciona SQL, vector, KV, queue e
  pubsub; não há seleção, trait ou adapter graph.
- `storage/capabilities.rs` reporta apenas esses cinco tipos.
- `docs/specs/02-storage-memoria-contexto-rag.md` não define property graph.
- Existe alteração preexistente do usuário em
  `bindings/typescript/native/index.d.ts`; deve ser preservada e excluída deste
  trabalho.
- Crate oficial `falkordb` 0.11.2 está publicado e requer Rust 1.95; seu client
  assíncrono usa Tokio. A Engine declara Rust 1.89. Não foi adicionada dependência;
  decisão sobre compatibilidade/MSRV precisa acompanhar a escolha da API.

## Etapas

1. **Contrato e pesquisa — em andamento.** Registrar fatos, escopo, limites,
   opções de API e decisão vital em
   [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md)
   e [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md).
   Gate: resposta de `brunosrz` sobre API Cypher específica, property graph
   portável ou duas camadas.
2. **Core Rust.** Definir seleção `graph`, configuração segura, trait/adapter
   de acordo com a decisão, erros, conexão, execução parametrizada e limites;
   adicionar testes unitários e de serviço.
3. **Bindings.** Expor o mesmo contrato nas bindings Python e TypeScript, sem
   lógica de domínio duplicada; snapshots/stubs e fixtures compartilhadas.
4. **Conformidade operacional.** Testar FalkorDB local via Podman/Docker:
   conexão/TLS conforme suportado, query parametrizada, isolamento, falhas,
   timeout, cancelamento, restart e persistência sob a configuração registrada.
5. **Entrega empilhada.** Atualizar checklist/análise/plano, verificar diff,
   formatar Markdown, commit, push e abrir PR filho do topo atual da stack.

## Critérios de aceite

- Contrato responde às perguntas de ownership, auth/TLS, timeout/cancelamento,
  limites, erro/retry, isolamento e observabilidade.
- Seleção/capabilities Rust/Python/TypeScript serializam de forma consistente.
- Query parameters não são concatenados em texto; erro de escrita não é
  repetido automaticamente após resultado de rede incerto.
- Integração real tem evidência reproduzível com versão de servidor e cliente.
- SQLite permanece autoridade local para estado interno crítico/ledger; não
  há dual-write implícito dos dados de domínio do consumer.
- Sem alegar equivalência com os pares existentes ou suporte completo a
  GraphRAG.

## Fora de escopo

GraphRAG SDK, extração/ingestão por modelo, schemas de produto, RBAC, visualização,
orquestração de containers e features exclusivas do server sem contrato e
conformidade próprios.

## Bloqueio atual

A decisão sobre superfície pública é vital e aguarda resposta no chat. A pesquisa
e a documentação são trabalho independente concluído nesta etapa; código que
fixaria uma das alternativas não deve avançar antes da decisão.

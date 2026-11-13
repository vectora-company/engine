# Instruções para agentes: Vectora Engine

Este repositório é o núcleo Rust planejado para capacidades reutilizáveis de
agentes. A spec de referência está em `../issues/08-contratos-backend-e-engine-rust.md`;
as cópias de issues relevantes ficam em `../issues/`. O Vectora consumidor está
no diretório irmão `../vectora/`. Consulte esses materiais antes de propor ou
alterar contratos. Não use issues abertas no GitHub como substitutas dessas
specs locais.

## Fonte de verdade

- Implementação Rust: `crates/` e, quando aprovados os contratos,
  `bindings/python/` e `bindings/typescript/`.
- O backend atual do Vectora continua sendo a referência de comportamento; a
  engine não autoriza reescrita ampla nem remoção de caminhos existentes.
- `utils/` contém tooling compartilhado de repositório. Não o misture à API da
  engine.
- Não trate código de Services ou de outro produto como implementação da
  engine.

## Regras de engenharia

1. Contratos primeiro: identifique consumidores, dados, erros, timeout, retry,
   cancelamento, ownership, observabilidade e compatibilidade antes de criar
   APIs públicas.
2. Migração incremental por capability; preserve fallback e rollback. Não faça
   dual-write nem duplique efeitos externos em shadow mode.
3. APIs públicas pequenas, dependências justificadas, `unsafe` proibido sem
   revisão explícita, erros tipados e comportamento determinístico onde o
   contrato permitir.
4. Não afirme paridade nem ganho de performance sem casos de conformidade e
   medições reproduzíveis por SHA, ambiente e configuração.
5. Storage tem duas camadas, nesta ordem. Base: tudo que existe nos dois lados
   de um par (SQLite/PostgreSQL, LanceDB/Qdrant, Redis/NATS KV, Redis
   Streams/JetStream, Redis Pub/Sub/NATS Core) é implementado nos dois, com a
   mesma API, semântica observável e suíte de conformidade; um lado sem a
   capacidade é lacuna, não escolha. Especialização: só o que o outro lado
   não pode fazer por incompatibilidade técnica verificada e registrada em
   `capabilities()`. Veja `docs/specs/02-storage-memoria-contexto-rag.md`.
6. MCP é uma integração cliente. A engine não converte o produto Vectora em
   servidor MCP.
7. Comentários explicam invariantes e comportamento presente; não guardam
   histórico de implementação, menções a ferramentas de IA ou etapas de
   planejamento.
8. Use Conventional Commits. Títulos e descrições de PR em português; para
   mudanças ligadas a uma spec, inclua uma única linha `Refs: #N` ou
   `Closes: #N` quando houver issue pública correspondente.

## Fluxo de trabalho do mantenedor

- Commit, push e PR são obrigatórios em toda etapa, sem esperar pedido. Aprovar
  PR e criar issue só quando `brunosrz` pedir.
- PRs são sempre empilhados: cada PR novo tem como base a branch do PR anterior
  aberto. A próxima etapa começa sem esperar CI nem aprovação. Um PR aberto é o
  trabalho ativo: liste os PRs antes de escolher o escopo.
- Decisão vital (contrato, dados, segurança, compatibilidade, escopo) é
  perguntada a `brunosrz` no chat, depois de concluído o trabalho independente;
  não use issues nem comentários de PR para isso.
- Storage: SQLite é a cópia local obrigatória e a autoridade de recuperação; o
  PostgreSQL recebe projeção. TTL por chave no NATS exige NATS Server 2.11+.
- O plano ativo e o backlog estão em `docs/plans/03-plano-continuidade.md`; o
  processo completo está em `docs/goal-loop/README.md`.

## Checks

```bash
scons install
scons lint
scons tests
```

O CI executa formatação, `check`, Clippy, testes e documentação Rust; build,
lint, type check, Bandit e testes do binding Python e do tooling; e build,
lint, typecheck e testes do binding TypeScript. Resultado de CI remoto não
substitui a verificação local registrada. Não baixe browsers Playwright para a
engine; o Chromium do app pertence ao Electron no repositório consumidor.

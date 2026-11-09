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
5. MCP é uma integração cliente. A engine não converte o produto Vectora em
   servidor MCP.
6. Comentários explicam invariantes e comportamento presente; não guardam
   histórico de implementação, menções a ferramentas de IA ou etapas de
   planejamento.
7. Use Conventional Commits. Títulos e descrições de PR em português; para
   mudanças ligadas a uma spec, inclua uma única linha `Refs: #N` ou
   `Closes: #N` quando houver issue pública correspondente.

## Checks

```bash
scons install
scons lint
scons tests
```

O CI executa os checks Rust e os testes do tooling compartilhado. Não baixe
browsers Playwright para a engine; o Chromium do app pertence ao Electron no
repositório consumidor.

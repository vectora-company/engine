# Análise e referências para desenvolvimento

Este documento define como investigar uma capability da Engine antes de
implementar ou alterar seus contratos. Ele complementa `AGENTS.md`; não substitui
as regras específicas de uma spec ou de um plano.

## Ordem das fontes

1. `AGENTS.md` e instruções locais dos diretórios que serão alterados.
2. A spec da Engine em `docs/specs/` para a capability em questão.
3. O plano ativo em `docs/plans/` e seu checklist de execução.
4. As referências citadas pela spec, incluindo as cópias locais em
   `../issues/` quando estiverem disponíveis.
5. Implementação atual da Engine e seus testes para verificar o comportamento
   realmente existente.
6. Consumidores relevantes, em especial `../vectora/`, para descobrir contratos
   e comportamento que precisam continuar compatíveis.
7. Documentação primária das bibliotecas e protocolos externos citados.

Issues locais e auditorias são material de requisitos e contexto. Não são prova
de que algo já foi implementado, nem autorização para copiar comportamento de
produto para a Engine. Issues abertas no GitHub não substituem os documentos
locais definidos como referência.

## Perguntas obrigatórias da investigação

Antes de abrir uma API ou modificar persistência, registrar respostas para os
itens aplicáveis:

- Qual capability está em escopo e qual explicitamente fica fora?
- Quais consumidores existem hoje e quais estão planejados? Quais linguagens e
  versões dos bindings precisam acompanhar o contrato?
- Quais são os tipos de entrada/saída, invariantes, versões e regras de
  compatibilidade?
- Como erros, timeout, retry, cancelamento, concorrência, ownership e
  idempotência se comportam?
- Quem autentica e autoriza? O que pertence ao host consumidor e o que a Engine
  valida ou persiste?
- Quais dados são duráveis, qual é a fonte da verdade e como funcionam recovery,
  replay e migração?
- Quais logs, eventos, métricas e evidências tornam o comportamento
  diagnosticável sem registrar segredos?
- Qual fallback e rollback mantêm a capability segura durante uma migração
  incremental?
- Quais cenários de conformidade, teste e benchmark sustentam cada afirmação de
  comportamento ou desempenho?

Quando um item não se aplica, anotar por que. Não preencher lacunas com
suposições silenciosas.

## Registro por etapa

Cada etapa de implementação de um plano deve apontar para:

1. spec e requisitos de origem;
2. análise do estado atual, com caminhos e símbolos relevantes;
3. decisão de contrato e alternativas descartadas, quando houver;
4. checklist de arquivos/camadas a alterar e critérios de aceite;
5. testes/checks e evidências obtidas;
6. riscos, limitações, rollback e itens ainda pendentes.

Atualize os documentos junto com o código. O checklist precisa corresponder ao
estado real: diferencie `feito`, `pendente`, `bloqueado` e `não aplicável`, sempre
com uma nota curta para itens bloqueados ou não aplicáveis.

## Decisões que exigem o mantenedor

Pare antes de implementar quando uma resposta alterar de modo material:

- contratos públicos, formato de dados ou estratégia de migração;
- segurança, trust boundary, permissões ou execução de efeitos externos;
- suporte anunciado, dependências obrigatórias ou compatibilidade;
- semântica de consistência, retenção, recuperação ou perda de dados;
- escopo do produto, responsabilidade do host ou dependência entre produtos.

Documente a pergunta, opções, impactos e trabalho bloqueado em `docs/`; peça a
decisão de `brunosrz` e encerre o loop até receber resposta. Não transforme uma
preferência presumida em decisão arquitetural.

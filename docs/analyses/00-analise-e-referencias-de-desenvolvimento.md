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
6. Consumidores relevantes: `../gh-bot/` como primeiro ambiente experimental
   integrado pela binding TypeScript local e `../vectora/` como referência de
   comportamento e consumidor Python posterior. Consulte
   `../gh-bot/docs/engine-integration.md` para o fluxo de desenvolvimento
   conjunto; não condicione esse fluxo à publicação npm.
7. Documentação primária das bibliotecas e protocolos externos citados.

## Revalidação de sessão e PR stack

Antes de escolher escopo, liste os PRs abertos com `gh pr list`; o PR no topo é
o trabalho ativo e base do próximo PR.
Confira `git branch --show-current`, `git status` e o SHA local porque o
checkout pode ser compartilhado. Faça commit, push e PR em cada etapa; PRs
novos são empilhados sobre a branch do PR anterior e a próxima etapa começa
sem aguardar aprovação ou CI. Decisões vitais são perguntadas a `brunosrz` aqui
no chat, nunca por marcação em GitHub.

Para storage, inspecione os serviços já instalados com `podman ps -a`, confirme
imagem/versão em execução e configure as variáveis `VECTORA_TEST_*`. Teste
condicional sem variável de serviço não executa integração e deve ser anotado
como `não executado`. Antes de parar/reiniciar um contêiner, confira
`AutoRemove`; não registre credenciais. CI remoto não é evidência desta etapa.

Em storage, aplique primeiro a base funcional equivalente entre os pares
definidos na spec 02. Só registre especialização se a incompatibilidade estiver
verificada para as versões concretas dos backends e sua justificativa estiver
visível em `capabilities()`; uma diferença observada hoje pode ser lacuna, não
uma decisão de produto.

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

Atualize plano, checklist e análise da capability em cada etapa, com o PR,
a evidência obtida e o trabalho pendente. Ao criar um Markdown, no mesmo PR
vincule-o no índice `docs/README.md`, no prompt
`docs/plans/00-prompt-loop-goal.md`, nos documentos do goal que o usam e nos
artefatos da capability que dependem dele. Use o caminho completo a partir
da raiz do repositório no texto do link e um destino relativo válido.

## Decisões que exigem o mantenedor

Pare antes de implementar a parte dependente quando uma resposta alterar de
modo material:

- contratos públicos, formato de dados ou estratégia de migração;
- segurança, trust boundary, permissões ou execução de efeitos externos;
- suporte anunciado, dependências obrigatórias ou compatibilidade;
- semântica de consistência, retenção, recuperação ou perda de dados;
- escopo do produto, responsabilidade do host ou dependência entre produtos.

Documente a pergunta, opções, impactos e trabalho dependente em `docs/`; peça a
decisão de `brunosrz` aqui no chat. Conclua primeiro o trabalho independente e
encerre o loop somente se não houver outra etapa independente aprovada. Não
transforme uma preferência presumida em decisão arquitetural.

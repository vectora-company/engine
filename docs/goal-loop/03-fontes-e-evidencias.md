# Fontes, referências e evidências

## Ordem de consulta

1. `AGENTS.md` e instruções locais dos diretórios afetados.
2. Spec da capability em `docs/specs/`.
3. Plano ativo em `docs/plans/` e checklist específico em `docs/checklists/`.
4. Análise específica em `docs/analyses/` e referências citadas, incluindo
   cópias locais pertinentes em `../issues/`.
5. Código e testes atuais da Engine.
6. Consumidores relevantes, especialmente `../vectora/`, para verificar
   contratos e comportamento que precisam continuar compatíveis.
7. Documentação primária das dependências e protocolos externos.

Issues abertas no GitHub não substituem specs e materiais locais. Documentação,
auditorias e planos registram intenção ou observações; isoladamente não provam
que o comportamento existe.

## Conteúdo obrigatório dos artefatos por capability

O plano deve registrar objetivo, escopo, fora de escopo, consumidores,
dependências, ordem das entregas, critérios de aceite, gates, rollback e estado
de cada etapa.

A análise deve registrar fontes, caminhos/símbolos examinados, comportamento
observado, fatos versus hipóteses, contratos e alternativas, riscos, decisões
pendentes e as evidências que sustentam cada conclusão.

O checklist deve decompor os critérios em tarefas verificáveis de preparação,
contratos, implementação, testes, revisão e entrega. Cada item usa um estado
permitido e aponta evidência ou justificativa quando necessário.

Os três arquivos precisam referenciar uns aos outros e estar ligados a partir
do plano ativo e do índice `docs/README.md`. Ao criar Markdown, atualize também
`docs/plans/00-prompt-loop-goal.md` e os documentos do goal que o consultam.
O texto de cada link mostra o caminho completo a partir da raiz da Engine.

## Registro de evidência

Para cada resultado relevante, anote comando, resultado, SHA e limitações do
ambiente. Diferencie `passou`, `falhou`, `bloqueado`, `ignorado` e `não
executado`. Não use check remoto de CI como evidência neste loop; não afirme
paridade, suporte ou ganho de desempenho além do que os testes/medições realmente
cobrem.

Evidência de serviço real: registre as variáveis usadas (`VECTORA_TEST_REDIS_URL`,
`VECTORA_TEST_NATS_URL`, `VECTORA_TEST_QDRANT_URL`, `VECTORA_TEST_POSTGRES_URL`
e, para reinício de serviços, `VECTORA_TEST_PODMAN_RESTART`), a contagem de
testes executados e o ambiente. Sem a variável, os testes retornam sem validar o
serviço: classifique como `não executado`, nunca como `passou`. Não registre
senhas nem URLs com credenciais.

Ao revisitar um finding, classifique-o como `confirmado`, `corrigido`, `não
reproduzido` ou `ainda não verificado`, com caminhos de código/teste que
sustentem a classificação. Se a evidência mudar, atualize análise, checklist e
plano juntos.

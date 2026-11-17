# Plano operacional do goal loop

## Objetivo

Executar, em sequência, as etapas aprovadas do plano ativo da Engine. Cada
etapa deve resultar em código coerente com os contratos, testes e documentação
que permitam revisar o que foi feito e o que permanece pendente.

## Retomada

1. Confirme repositório, branch, base da stack e estado da working tree, liste
   os PRs abertos (`gh pr list`; o PR aberto é o trabalho ativo) e, para
   storage, os contêineres de teste (`podman ps -a`). O checkout pode ser
   compartilhado com outra sessão: não presuma a branch.
2. Leia `AGENTS.md`, o plano ativo e a spec correspondente. Consulte a análise
   e o checklist específicos; aplique também o [checklist geral](../checklists/00-checklist-de-implementacao.md)
   e as [regras de evidência](03-fontes-e-evidencias.md).
3. Compare o estado documentado com o código e os testes atuais. Atualize
   descobertas antigas para `confirmado`, `corrigido`, `não reproduzido` ou
   `ainda não verificado`.
4. Escolha a menor etapa pendente, aprovada e desbloqueada. Se faltarem plano,
   análise ou checklist suficientes, escreva-os antes de uma mudança ampla.
5. Se uma decisão vital estiver pendente, siga os
   [critérios de encerramento](04-criterios-de-encerramento.md) antes de tocar
   no trabalho dependente.

## Execução de cada etapa

1. Registre objetivo, escopo, exclusões, consumidores, dependências e critérios
   de aceite no plano/checklist específico.
2. Resolva os contratos aplicáveis: dados, erros, compatibilidade, timeout,
   retry, cancelamento, concorrência, ownership, autorização, persistência,
   recuperação e observabilidade.
3. Implemente de forma incremental: core Rust, bindings afetados, consumidores
   apenas quando estiverem no escopo aprovado. Para capabilities destinadas ao
   GH Bot, use o checkout listado no `AGENTS.md` e o
   [guia de integração](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md).
   O Bot deve consumir localmente `bindings/typescript` do checkout ativo da
   Engine, sem aguardar publicação npm.
4. Acrescente testes para os critérios de aceite, inclusive falhas, limites,
   restart, concorrência e efeitos externos quando aplicável.
5. Rode verificações locais pertinentes, corrija regressões da etapa e registre
   comandos, resultados, SHA e limitações. O estado remoto de CI não é requisito
   deste loop.
6. Revise o diff e atualize plano, análise e checklist para refletirem o código
   real. Não marque como concluído o que só está parcialmente implementado.
   Ao criar Markdown, vincule seu caminho completo desde a raiz no índice,
   no prompt e nos documentos do goal e da capability que o usam.
7. Faça commit, push e abra o PR com base na branch do PR anterior da stack;
   é obrigatório e não depende de pedido. Inicie a próxima etapa já em uma nova
   branch empilhada, sem esperar aprovação nem CI. Aprovar PRs e criar issues
   só ocorre quando `brunosrz` pedir. Use a próxima etapa já aprovada no plano
   ativo e confirme a base correta antes de abrir a branch empilhada.

## Validação experimental pelo GH Bot

O GH Bot é o ambiente controlado para exercitar a Engine durante o bootstrap,
não uma etapa de adoção posterior à distribuição. Quando a etapa ativa envolver
uma capability consumível pelo Bot, teste o caminho integrado no checkout local
e corrija lacunas na Engine e no Bot dentro da mesma stack. Use uma conta GitHub
nova e repositórios experimentais; não use credenciais nem repositórios de
produção. Preserve no Bot somente a lógica de produto e integração GitHub, sem
reimplementar o runtime em TypeScript.

## Continuidade

Concluir uma etapa não conclui o goal enquanto houver outra etapa aprovada no
plano ativo. Ao terminar uma etapa, selecione a próxima pendente e empilhe o
PR seguinte sobre o anterior. Em storage, aplique a equivalência por par:
implemente nos dois lados do par e só especialize o que for incompatível. Pare somente
nas condições do documento de [encerramento](04-criterios-de-encerramento.md).

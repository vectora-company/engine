# Plano operacional do goal loop

## Objetivo

Executar, em sequência, as etapas aprovadas do plano ativo da Engine. Cada
etapa deve resultar em código coerente com os contratos, testes e documentação
que permitam revisar o que foi feito e o que permanece pendente.

## Retomada

1. Confirme repositório, branch, base da stack e estado da working tree.
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
   apenas quando estiverem no escopo aprovado.
4. Acrescente testes para os critérios de aceite, inclusive falhas, limites,
   restart, concorrência e efeitos externos quando aplicável.
5. Rode verificações locais pertinentes, corrija regressões da etapa e registre
   comandos, resultados, SHA e limitações. O estado remoto de CI não é requisito
   deste loop.
6. Revise o diff e atualize plano, análise e checklist para refletirem o código
   real. Não marque como concluído o que só está parcialmente implementado.
7. Faça commit e atualize o PR/stack segundo as instruções do mantenedor. Antes
   de iniciar outra capability, confirme sua autorização e a base correta.

## Continuidade

Concluir uma etapa não conclui o goal enquanto houver outra etapa aprovada no
plano ativo. Ao terminar uma etapa, selecione a próxima pendente. Pare somente
nas condições do documento de [encerramento](04-criterios-de-encerramento.md).

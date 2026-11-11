# Runtime agêntico e harness compartilhado

## Maturidade da implementação

O ciclo de vida de execução, a montagem de turno, a preparação de dispatch,
coordenação em memória de rodadas, avaliação experimental de policy/aprovação,
protocolo conservador de reconciliação e adapter SQLite para persistência
atômica de eventos/checkpoints do loop estão implementados como fundação. O
adapter SQLite persiste cada transição antes de devolver sua ação, mas não
torna efeitos externos atômicos nem fornece exactly-once. O consumidor ainda
executa adapters, autentica o reconciliador e coordena providers. Persistência
e revalidação de policy/concessões, reserva integrada à identidade do run,
adapter equivalente para PostgreSQL, coordenação operacional de restart,
aplicação de hooks/egress e sandbox, verificação e gates ainda não estão
implementados. A avaliação de policy não autentica o host nem o aprovador. A
sequência incremental está em
[`docs/plans/02-plano-loop-agentico.md`](../plans/02-plano-loop-agentico.md).

## Objetivo

Definir o runtime comum de execução de agentes para produtos diferentes. A engine recebe uma tarefa com contexto confiável, orçamento e capacidades explicitamente concedidas; controla o loop do modelo, chamadas de tools, decisões, estado, eventos, checkpoints, cancelamento e verificação. O produto consumidor continua dono da experiência, identidade, política de negócio e adapters que realizam efeitos externos.

O mesmo runtime deve poder ser consumido pelo Vectora, por um GH Bot independente do app Vectora e por futuros produtos como assistente pessoal ou construtor de aplicações. O contrato não pressupõe programação, workspace de código, chat visual ou um fornecedor específico.

## Princípio central

O modelo propõe ações. A engine controla a execução e só invoca uma tool depois de validar o pedido contra o contexto de autorização confiável recebido do host e as políticas efetivas. O resultado do modelo, argumentos, memória recuperada ou texto do usuário nunca elevam privilégios.

## Responsabilidades da engine

### Contrato e ciclo de execução

- Definir contratos versionados para `Run`, `Turn`, `Step`, `ToolCall`, `Approval`, `Artifact`, `Evidence`, `Usage` e eventos terminais.
- Usar IDs estáveis de run/turno/etapa/tool-call; aceitar chave idempotente fornecida pelo consumidor e detectar repetição sem executar efeitos duplicados.
- Modelar estados e transições válidas: planned, running, waiting_approval, waiting_provider, tool_running, verifying, passed, degraded, failed, cancelled e recovery_required, ou uma versão formalmente revisada desse conjunto.
- Registrar timeline ordenada e correlacionada, incluindo tentativas, decisões, eventos de tool, aprovação, evidência, uso, duração, erro e motivo terminal.
- Retomar a execução de um checkpoint compatível após restart, reconciliação ou reconexão sem assumir que a conexão de streaming é a autoridade.
- Distinguir status da execução, resultado da verificação e veredito do gate; resposta textual do modelo isolada não comprova sucesso.

### Interfaces de modelo e loop

- Consumir interfaces de provider e streaming independentes de fornecedor, com mensagens, tools, eventos, usage, timeout, retry, cancelamento e erros tipados.
- Controlar limites de iteração, contexto, tempo, tokens/orçamento e chamadas de tools.
- Preservar chamadas e respostas de tools no histórico e validar formato de saída estruturada.
- Aplicar backpressure e propagar cancelamento até chamadas de modelo e adapters.
- Permitir estratégia explícita de provider/fallback sem retry ou troca que duplique saída/efeito depois de eventos observáveis.

### Tools e efeitos externos

- Expor protocolo genérico de tools com schema de entrada/saída, capability requerida, risco, timeout, cancelamento, idempotência quando suportada e classificação de efeito.
- Separar a decisão de invocar uma tool do adapter que a executa. O host registra as tools disponíveis; a engine não incorpora GitHub, filesystem, browser, terminal, IDE, editor visual ou API privada de um produto no núcleo.
- Revalidar política antes de cada efeito, inclusive após aprovação, retomada, mudança de contexto ou chamada em lote.
- Representar efeitos não idempotentes ou parcialmente concluídos como estado recuperável/incerto, sem repetir silenciosamente.
- Permitir dry-run quando o adapter oferecer essa capacidade; não simular uma garantia que o adapter não possui.

### Política, confiança e aprovação

- Aceitar do host um snapshot versionado de principal/identidade, escopo, capabilities autorizadas, restrições, orçamento e versão/digest de política.
- Não implementar RBAC ou identidade de produto. A engine valida estrutura, integridade, escopo e validade do snapshot conforme o contrato; o host decide como ele foi autenticado e emitido.
- Aplicar default-deny a capability ausente, decisão ambígua ou política não verificável; falhar fechado para efeitos classificados como destrutivos.
- Distinguir isolamento comprovado de fallback soft. Habilitar sandbox no snapshot não prova que o runtime externo realmente isolou o processo.
- Fornecer protocolo de HITL e Plan Mode sem UI: pedido de aprovação vinculado a conteúdo, escopo, ferramenta, argumentos normalizados, versão da política e expiração; mudança relevante invalida aprovação.
- Hooks pre-tool, post-tool e pre-final podem ser informativos ou bloqueantes, com timeout, versão e resultado auditável.
- Manter segredos fora do prompt e das saídas/logs; conteúdo do modelo e recuperado é dado não confiável.

### Delegação, decisões e verificação

- Subagentes são novas execuções relacionadas por parent ID, com escopo, capacidades, orçamento, prazo, cancelamento e formato de retorno limitados.
- Delegação não cria permissões novas nem transfere automaticamente ownership do host.
- Decision Models têm interface própria para classificação, score, seleção e resposta tipada; registrar provider/capability/versão, limites e validação, sem alegar equivalência entre modelos.
- Suportar verificação genérica de evidências e gates fornecidos/configurados pelo consumidor: `passed`, `failed`, `infra_error`, `not_run`; o veredito pode ser `passed`, `degraded`, `blocked` ou `unverified`. O contrato experimental está em [`15-verification-gates.md`](15-verification-gates.md); ele ainda não persiste report/decisão no lifecycle.
- Evidências devem apontar para artifacts/saídas concretos, ter origem e digest quando aplicável, e respeitar redação/expiração.

## Responsabilidades do consumidor

- Autenticar pessoas, bots e serviços e emitir contexto de execução confiável.
- Definir identidade, tenant/account, isolamento e políticas de produto, incluindo regras de cobrança e limites comerciais.
- Registrar adapters e schemas de tools específicos: por exemplo, GitHub para GH Bot, automação do computador para assistente pessoal, ou operações de geração/publicação para um construtor de apps.
- No GH Bot, integrar microserviços de Vectora Services por adapters do Bot. A engine não importa Services e o Bot não depende do frontend nem do processo do Vectora App; self-hosted e hosted variam por deployment, não pelo contrato do runtime.
- Escolher configuração de provider, credenciais e armazenamento, sem delegar segredos ao modelo.
- Renderizar UX, chat, editor, painel de aprovação, notificações e controles.
- Determinar quais evidências e gates são adequados ao produto e como seus artifacts são armazenados.

## Fora do escopo do núcleo

- Workbenches, editor de código, IDE, Kanban, terminal UI, browser UI, filesystem UI e fluxos próprios do Vectora.
- RBAC/workspace específico do Vectora, catálogo/onboarding, billing, assinatura e contas.
- API ou integração privada de um produto como dependência interna do runtime.
- Uma implementação obrigatória de sandbox, autorização de host ou instalação de software; isso é fornecido por runtime/consumer adapter e reportado com suas garantias reais.
- Promessa de que todo produto precisa usar chat, operar computador ou executar código.

## Contratos e requisitos não funcionais

Para cada capability, especificar: schema/versionamento, ownership, erros, timeout, cancelamento, retry, idempotência, concorrência, durabilidade, redaction, observabilidade e compatibilidade. Eventos devem ser estruturados e correlacionados por run/turno, sem expor raciocínio interno bruto do modelo.

## Migração e conformidade

- Usar a execução Python do Vectora como referência observável, separando o comportamento reutilizável das decisões do produto.
- Criar casos de conformidade de loop, estado, política, ferramentas, aprovação, falha, cancelamento, retomada e verificação antes de substituir qualquer caminho.
- Comparar em workloads isolados e reproduzíveis; shadow mode não realiza efeitos externos duplicados.
- Integrar primeiro como opção reversível por capability. A engine não assume que todos os produtos migram ao mesmo tempo.
- Manter o GH Bot capaz de consumir runtime e adapters de Services sem depender do frontend, do processo desktop nem dos Workbenches do Vectora.

## Critérios de aceite

- Pelo menos dois perfis de consumidor podem descrever identidades, tools, políticas e saídas diferentes sem importar tipos de produto Vectora.
- Run/turno/tool-call têm IDs e transições determinísticas; retries e replay não duplicam effects quando há idempotency contract.
- Tool invocation exige capability e contexto confiável; policy unavailable/revoked/corrupt interrompe o efeito de acordo com a regra definida.
- Cancelamento e deadline propagam a todos os adapters e persistem resultado terminal recuperável.
- HITL vincula aprovação ao digest correto e não reutiliza aprovação após mudança do alvo.
- Gate distingue falha funcional, falha de infraestrutura e não executado; não executado nunca conta como sucesso.
- Segredos e conteúdo de raciocínio interno não aparecem em eventos públicos, evidências ou logs.
- Casos de conformidade e evidência reproduzível são registrados antes de paridade/performance ser afirmada.

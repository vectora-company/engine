# Sabatina da Engine e do ecossistema de agentes

## Propósito, método e fotografia

Esta sabatina responde a 200 perguntas sobre o que uma engine/framework agêntico
precisa oferecer, o estado verificável da Vectora Engine, a relação com Vectora
App e GH Bot, e as decisões que podem ser aprendidas de outros frameworks. Ela
não é promessa de produto nem uma lista para implementar integralmente. Use-a
para detectar lacunas, orientar contratos e evitar que nomes de módulos sejam
confundidos com capacidades utilizáveis.

**Fotografia local da Engine:** `44963ce` (`docs: cruzar Akita, Engine, Vectora e GH Bot`), topo da branch `docs/engine-vectora-akita-analysis`, PR [#36](https://github.com/vectora-company/engine/pull/36), que inclui a base de conformidade #34. As respostas sobre implementação se referem ao código dessa fotografia, salvo quando indicarem outra fonte. A auditoria anterior de conformidade registra `main` em `22c861f`; ela é evidência histórica e precisa ser reconciliada com o checkout atual antes de afirmar que representa a `main` corrente.

**Fotografias dos consumidores registradas na análise cruzada:** Vectora `origin/master` em `bf1732ac92eac21671881cc34a530c2ddb7186de`; GH Bot `main` em `b489b90263e3a4ace94cfdce45763489918a3671`. São snapshots distintos, não garantias de estado atual. A leitura do livro _Competente o Suficiente_ e suas limitações está documentada em [03 — análise cruzada](03-analise-cruzada-akita-engine-vectora-gh-bot.md).

**Legenda de estado:** **Implementado experimentalmente** = há código/API e evidência localizada, sem garantia de prontidão de produção; **Parcial** = só parte do comportamento está implementada; **Planejado** = spec ou intenção sem API executável completa; **Ausente** = não encontrado na fotografia; **Decisão** = requer escolha arquitetural/produto; **Externo** = responsabilidade do host, adapter ou serviço. Esses rótulos são relativos à fotografia e devem mudar quando a evidência mudar.

## A. Identidade, fronteiras e valor (1–20)

1. **O que é a Engine?** É o núcleo Rust reutilizável de runtime agêntico e framework de capacidades. Não é o Vectora App, uma UI, um serviço SaaS nem apenas uma coleção de chamadas a modelos. **Estado:** objetivo arquitetural; maturidade de runtime ainda experimental.
2. **Por que uma engine compartilhada?** Para que Vectora, GH Bot e produtos futuros compartilhem contratos e mecanismos, em vez de manter loops, persistência e segurança divergentes. O benefício só existe se os consumidores realmente adotarem as APIs e a conformidade for demonstrada.
3. **Qual é a API canônica?** Rust é a implementação e fonte canônica dos contratos. Python/PyPI e TypeScript/npm são bindings/embalagens do mesmo core, não forks com implementações paralelas. **Estado:** princípio documentado; distribuição/publicação completa não é provada pelo bootstrap.
4. **Por que Rust?** O projeto busca runtime eficiente, contratos explícitos e reutilização via bindings. A linguagem não garante menor latência percebida, menos bugs em lógica, nem superioridade sobre Python/TS; isso precisa de benchmarks justos.
5. **A Engine substitui o Vectora?** Não. Ela pode absorver mecanismos genéricos; interface, workbenches, identidade de produto, UX e integrações específicas permanecem no Vectora. Migração deve ser por capability com fallback e rollback.
6. **A Engine é um harness ou framework?** Pretende ser ambos: harness para executar ciclos e framework para registrar extensões que usam os mecanismos do runtime. Hoje os contratos de loop, dispatch e policy são partes, não o framework extensível completo.
7. **O que significa harness?** O conjunto que organiza entrada, contexto, modelo, tools, políticas, estado, recuperação, avaliação e observabilidade para produzir comportamento executável. Um cliente LLM isolado não é um harness completo.
8. **O que significa framework aqui?** APIs estáveis para compor e estender providers, tools, guardrails, storage, workflows e outros componentes sob regras comuns. Uma pasta chamada `extensions` sem contratos, lifecycle e compatibilidade não seria framework.
9. **Quais produtos devem consumir a Engine primeiro?** Vectora App (backend Python) e GH Bot (TypeScript) são os consumidores pretendidos. Integração e paridade devem ser medidas por casos reais de cada produto, não inferidas da existência dos bindings.
10. **A Engine é um produto voltado ao usuário?** É uma biblioteca/runtime. O host é responsável por UX, onboarding, identidade do usuário e apresentação de erros; ferramentas de operação podem existir sem transformar o núcleo em app final.
11. **Qual é a fronteira do Vectora Services?** O Services fornece serviços de produto como login/entitlements e APIs escolhidas. Não deve virar proxy obrigatório de toda chamada de modelo ou do GitHub; a topologia depende do modo pago/self-hosted do GH Bot.
12. **O que deve ser específico do Vectora?** Workbenches, interface Electron/web, compartilhamento de terminal com usuário, preferências e ferramentas cuja semântica pertence ao fluxo de engenharia do Vectora. A engine pode fornecer interfaces e execução reutilizável.
13. **O que deve ser específico do GH Bot?** Webhooks, instalação GitHub, identidade da instalação, revisão de PR, comentários, entrega de resultados e configuração do bot. A Engine fornece execução agêntica genérica; não deve conhecer a UX ou os objetos do GitHub como conceitos centrais.
14. **A Engine é uma camada de microserviços?** Não necessariamente. Pode rodar embutida como biblioteca ou atrás de um processo/serviço escolhido pelo host. Distribuição e isolamento são decisões de deployment, não sinônimos da arquitetura da Engine.
15. **O que define sucesso?** Consumers conseguem executar workloads úteis com contratos previsíveis, recuperação testada, segurança explícita e custo/latência medidos. Contagem de crates, APIs ou ferramentas não é uma métrica suficiente.
16. **A Engine deve implementar todo recurso possível?** Não. O núcleo obrigatório deve permanecer pequeno; capacidades especializadas podem ser extensions/adapters. Antes de adicionar algo, provar reutilização entre consumidores, contrato e custo de manutenção.
17. **O que não se deve copiar do Vectora automaticamente?** Nomes, modelos de dados, endpoints e abstrações acoplados à UI ou à história interna do produto. Primeiro identificar o conceito, consumidores, invariantes e limites.
18. **Como usar a issue #387?** Como enquadramento de produto e fonte de hipóteses sobre o harness, comparada ao código. Seus rótulos podem ser imprecisos; não são especificação da Engine nem evidência de implementação.
19. **Como usar _Competente o Suficiente_?** Como fonte de princípios e experiências a testar, não como prescrição ou prova. A análise cruzada registra títulos/limites de leitura e confronta teses com o código e workloads reais.
20. **Como evitar uma comparação de marketing?** Comparar capacidades, contratos, maturidade, testes, deployment e responsabilidades. Frameworks frequentemente ocupam camadas diferentes; tabela de features sem normalizar camadas induz conclusão errada.

## B. Harness, loops e orquestração (21–40)

21. **Qual é o papel de `RunLifecycle`?** Modelar estados e transições do ciclo de vida de uma execução. **Estado:** contrato experimental versionado com suporte a SQLite em escopos específicos; não é o runner completo de agentes.
22. **O que é `ModelToolLoop`?** Coordenação persistível entre turnos do modelo, chamadas de tools, resultados, cancelamento e recuperação. **Estado:** experimental; não chama provider nem executa os adapters das tools.
23. **O loop já é um agente completo?** Não. É uma máquina/contrato de coordenação. O host ainda seleciona modelo, entrega chamadas ao executor autorizado, submete resultados e decide como reconciliar efeitos incertos.
24. **O que é `TurnAssembly`?** Acumula eventos de streaming em uma resposta canônica, incluindo tool calls. **Estado:** implementado experimentalmente em Rust e bindings; não persiste conversas nem fornece replay de sessão.
25. **Como o loop deveria tratar limite de turnos?** Tornar limite configurável, persistido e verificável, finalizar com motivo tipado e evitar loops infinitos. O contrato precisa definir se a execução pode continuar por novo run.
26. **Como distinguir run, turno e chamada de tool?** Run é unidade de execução; turno é uma interação de modelo dentro dela; tool call é operação específica proposta/aceita. IDs e relações devem sobreviver a retry e recovery.
27. **A Engine já gerencia subagentes?** Não na fotografia documentada. Subagentes exigem contrato de parent/child, budget, propagação de cancelamento, autorização, estado e agregação de resultados antes de API pública.
28. **Multiagente melhora resultados por padrão?** Não. Acrescenta coordenação, latência, custo e falhas. Começar com agente único e adicionar delegação apenas quando avaliações mostrarem ganho é uma lição coerente com documentação de AutoGen e com o custo descrito por Akita.
29. **O que é workflow orchestration?** Coordenação explícita de passos, eventos, branches, waits, retries e efeitos. Um loop LLM com ferramentas não equivale a workflow durável de negócio.
30. **A Engine tem scheduler de tarefas?** Não foi demonstrado como capacidade geral. Agendamento temporal, workers, distribuição e leases exigem contrato separado; não confundir fila de mensagens com scheduler.
31. **O que é task planning?** Construção e atualização de um plano de tarefas com dependências e critérios de conclusão. Um texto gerado pelo modelo não é automaticamente plano validado nem workflow executável.
32. **Como tratar interrupções?** Cancelamento deve propagar para o transporte de modelo, tool em execução, subexecuções e waits, com estado final/reconciliável. O contrato atual cobre estado/coordenador em parte, não todos os efeitos downstream.
33. **O que ocorre se um processo cai no meio de uma tool?** O resultado pode ser desconhecido; repetir pode duplicar efeitos. Persistir intenção, chave idempotente e estado `unknown`, e pedir reconciliação ao host é mais seguro que retry cego.
34. **A Engine garante exactly-once?** Não como propriedade geral distribuída. Pode oferecer idempotência por operação e deduplicação em limites definidos; efeitos externos ainda dependem do destino e do protocolo de reconciliação.
35. **O que significa durable execution?** Poder recuperar a execução após interrupção, com passos/efeitos registrados e sem duplicação indevida. Salvar um snapshot não basta se chamadas externas não forem journaled ou reconciliadas.
36. **Como separar estado do run e resultado da verificação?** São contratos diferentes: lifecycle representa execução; gate representa evidências e veredito. Um run terminal não prova qualidade nem sucesso de todos os checks.
37. **O que falta para resumir runs de forma completa?** Contrato de identidade de sessão, checkpoints de contexto/turno, tool execution ledger, adapters executáveis, reabertura após restart e testes de falhas/replay por binding.
38. **O que é checkpoint?** Estado suficiente e versionado para continuar de um ponto seguro. Deve declarar o que ficou de fora, referências externas, invariantes e compatibilidade de schema.
39. **O que é event sourcing e a Engine já o faz integralmente?** Event sourcing trata eventos como fonte de reconstrução do estado. Um ledger append-only e checkpoints não fazem do sistema inteiro event-sourced; aqui há stores/contratos parciais.
40. **Quando encerrar uma execução?** Quando critérios explícitos de término, erro não recuperável, cancelamento ou gate aplicável forem satisfeitos. “O modelo parou de responder” não é política suficiente.

## C. Engine, Vectora App e GH Bot (41–60)

41. **O que a Engine tem que o Vectora ainda não consome?** Contratos Rust reutilizáveis e bindings para parte de chat, fallback, lifecycle, dispatch, policy, verification e storage. A fotografia comparada não mostra Vectora usando esse core como runtime substituto.
42. **O que o Vectora tem a mais hoje?** Integração funcional de produto: backend conversacional, contexto, ferramentas, workbenches, sessão e operações específicas. Isso não significa que todos os módulos tenham o mesmo nível de confiabilidade.
43. **O que a Engine tem a mais que o Vectora?** Uma fronteira multi-consumer explícita, contratos versionados compartilháveis, adapters Rust e estruturas de persistência específicas em formação. “A mais” aqui significa arquitetura/código observado, não mais capacidade de usuário pronta.
44. **O que o Vectora tem que não deve migrar?** UI, Electron/tray, Workbenches como produtos interativos, experiência de workspace e integração de terminal compartilhada. A Engine pode receber abstrações genéricas solicitadas por mais de um consumidor.
45. **`AgentRun` do Vectora é igual a `RunLifecycle`?** Não. AgentRun é estrutura integrada mais ampla no backend do produto; RunLifecycle é uma máquina/contrato menor. Equivalência requer mapeamento de estados, eventos, identidade, artifacts e migração testada.
46. **“Evolution Lab” corresponde a evaluation da Engine?** Não automaticamente. O nome do Vectora é pouco descritivo; é preciso inventariar datasets, execuções, critérios, métricas e UI. Engine evaluation deve ser chamada por seu contrato real, não renomeada por semelhança nominal.
47. **O que é Context Projection no Vectora?** Uma etapa de seleção/transformação de contexto do produto. Não há API equivalente na Engine na fotografia; embedding e VectorStore sozinhos não realizam projeção de contexto.
48. **O GH Bot deve carregar todo o backend Vectora?** Não. Seu objetivo é um serviço TypeScript focado em GitHub, consumindo a Engine e serviços selecionados, sem Electron nem o conjunto de workbenches.
49. **O GH Bot é só uma action?** A action/review-action pode iniciar ou autorizar entrega, mas o produto também prevê runtime self-hosted, CLI e dashboard. A arquitetura distingue trigger, control plane e execução.
50. **Onde fica a conexão GitHub no modo self-hosted?** No ambiente do usuário, acionada diretamente pela action/webhook e configurada com endpoint/token apropriados. Services pode autenticar e redirecionar/configurar, mas não precisa transportar cada evento ou chamada.
51. **Como diferem os modos pago e self-hosted do Bot?** No hospedado, Vectora opera runtime e oferece serviço mediante assinatura; no self-hosted, usuário opera seu runtime e mantém custos de VPS. Ambos podem compartilhar Engine; requisitos de suporte e isolamento diferem.
52. **Qual papel do Services no Bot?** Login Vectora, entitlement e serviços de conta acordados. Credenciais GitHub, chamadas de modelo e storage devem permanecer nos limites definidos pelo modo de operação, não passar pelo Services por conveniência.
53. **Qual código é host e qual é engine?** Host resolve identidade confiável, configuração do produto, adapters/integrações e efeitos externos; Engine coordena contratos comuns e valida decisões compatíveis com seu escopo.
54. **Quem autentica o aprovador de uma tool?** O host confiável. Engine pode validar vínculo do recibo com run, chamada, escopo e policy; não deve confiar em identidade livre fornecida pelo modelo.
55. **Quem define a policy de domínio?** O consumer compila entradas confiáveis (usuário, workspace, repo, plano) em capacidades/regras. Engine recebe snapshot explícito e aplica o contrato genérico.
56. **Como evitar divergência de comportamento entre App e Bot?** Fixtures de conformidade compartilhadas, schemas/erros versionados, mesmos casos de replay e testes de integração específicos de cada host. Binding compartilhado não garante fluxo idêntico.
57. **Qual deve ser o primeiro caso de adoção no Vectora?** Uma capability isolada, com limite claro e fallback para Python, comparada por casos e métricas. A decisão deve vir do plano ativo e das dependências reais, não de “migrar tudo”.
58. **O que deve ser migrado primeiro: providers ou loop?** A ordem depende das fronteiras existentes; os planos atuais priorizam portabilidade de LLM e evolução do harness. Não migrar loop antes de definir contexto, ferramentas, cancelamento e persistência necessárias ao consumer.
59. **Como manter rollback?** Feature flag/capability selector, registro compatível de entradas/saídas e não duplicação de efeitos externos em shadow mode. Persistência deve permitir voltar sem perder nem corromper estado.
60. **Qual risco estratégico principal?** Construir um framework genérico sem consumidores nem casos de conformidade. Mitigar com adoção vertical: uma capability, dois hosts quando aplicável, casos reais e decisão de continuar baseada em evidência.

## D. Modelos, providers e inferência (61–80)

61. **O que a Engine oferece para chat?** Contratos de mensagens, completion e streaming, transporte OpenAI-compatible e adapters nativos previstos/implementados para alguns providers. O status exato de cada método/binding deve ser conferido na matriz e no código.
62. **OpenAI-compatible significa universal?** Não. Fornecedores divergem em campos opcionais, ferramentas, streaming, usage, erros e autenticação. Compatibilidade exige fixtures por comportamento e documentação de limites.
63. **A Engine suporta Omnirouter/9Router?** Eles podem ser consumidos pelo caminho OpenAI-compatible quando contrato e autenticação forem compatíveis. Isso não implica suporte oficial validado de toda rota ou de uso de assinatura.
64. **Providers nativos são melhores que compatibilidade?** Não em geral. Native adapter preserva recursos específicos; API compatível reduz custo de integração. Cada provider deve declarar capacidades e perder features explicitamente, não silenciosamente.
65. **Como representar capacidades de modelo?** Com metadados tipados e verificáveis: modalities, tools, context, structured output, streaming e limites conhecidos. Não inferir capacidade apenas pelo nome do modelo.
66. **Quem escolhe o modelo?** A configuração/policy do host ou um selector documentado. Engine pode fornecer interfaces de routing; escolha automática exige decisão auditável e critérios testáveis.
67. **O que são Decision Models?** Modelos ou chamadas auxiliares para classificação/roteamento/decisões delimitadas. Não devem receber autoridade implícita para permissões, identidade ou efeitos sensíveis.
68. **Há fallback?** Há contratos/adapters de fallback explícitos para chat, embedding/rerank em superfícies documentadas. Fallback deve distinguir erro elegível, preservar saída parcial e evitar duplicar cobrança/efeitos.
69. **É seguro trocar provider após streaming parcial?** Em geral não sem contrato de retomada; o host pode ter exibido conteúdo. A política precisa registrar saída já emitida e evitar duplicar resposta como se fosse contínua.
70. **Como tratar retries?** Classificar erro, limitar tentativas, jitter, timeout e orçamento; retries para operações idempotentes diferem de requests de geração já cobrados. Nunca repetir efeito de tool sem dedupe/reconciliação.
71. **Como gerenciar API keys?** Host/secret manager fornece credenciais em tempo de uso; evitar persistir em logs, contexto, fixtures ou snapshots exportáveis. Engine deve minimizar cópias e nunca autenticar usuário final por segredo de provider.
72. **Token budgeting já está completo?** Não como sistema abrangente provado. Providers podem reportar usage; orçamento de conversa, tools e subagentes requer políticas e contexto integrados.
73. **Como medir custo?** Registrar provider, modelo, tokens/units, retries, estimativa/preço e correlação com run, preservando privacidade. Preços mudam; estimativas devem guardar origem e instante.
74. **Embeddings significam RAG pronto?** Não. Embeddings geram vetores; RAG inclui ingestão, chunking, metadata, indexação, query, filtros, ranking, contexto e avaliação.
75. **Reranker substitui recuperação?** Não. Ele reordena candidatos recuperados; depende de retrieval e pode aumentar custo/latência. Avaliar ganho de relevância em conjunto.
76. **Vision está disponível como plataforma completa?** Não se deve afirmar isso sem APIs e bindings de modalidade demonstrados. Suporte nativo de provider pode existir parcialmente, mas ingestão, limites e fixtures precisam ser auditados.
77. **Áudio, fala e vídeo fazem parte do core atual?** Não foram estabelecidos como capacidades completas na fotografia. São candidatos separados com codec, streaming, time alignment, privacy e custo próprios.
78. **Como suportar saída estruturada?** Schema versionado, validação determinística, erros de parse distintos de erro do provider e política de reparo com limite. Não confiar em JSON apenas porque o prompt pediu.
79. **Como lidar com conteúdo multimodal?** Representar blocos tipados e referências seguras, com metadados de mime, tamanho, origem e consentimento. Não serializar binários enormes no ledger sem política.
80. **Como comparar providers?** Mesmo conjunto de tarefas, prompts, ferramentas, contexto, parâmetros e orçamento; medir qualidade, taxa de conclusão, falha, latência e custo. Resultados valem para dataset e SHA indicados.

## E. Contexto, memória e conhecimento (81–100)

81. **A Engine tem Memory completa?** Não na fotografia documentada. Existem storage/vector primitives, mas não APIs de memória conversacional, working memory, consolidação ou retrieval policy integradas.
82. **O que é Conversation History?** Registro ordenado de mensagens/eventos de uma conversa com identidade, retenção e replay definidos. `TurnAssembly` produz uma resposta, não histórico durável.
83. **Memória de run e memória de usuário são iguais?** Não. Run state é necessário à recuperação daquela execução; memória de longo prazo exige escopo, consentimento, retenção, proveniência e mecanismos de correção/remoção.
84. **O que é Context Engineering?** Selecionar, estruturar e priorizar instruções, histórico, evidências e ferramentas dentro de limite e objetivo. É mais amplo do que prompt templating e não está implementado como serviço completo na Engine.
85. **O que é Context Projection?** Um artefato derivado de contexto, filtrado e limitado para determinado turno/tarefa. Precisa preservar origem e explicar omissões; não deve ser confundido com embedding.
86. **O que é Context Compaction?** Resumir/transformar histórico para caber no orçamento preservando fatos necessários e marcando incerteza. Deve permitir auditoria e evitar tratar resumo como transcrição literal.
87. **O Vectora já tem gestão de contexto?** O backend consultado possui planner, compaction, memória/RAG e Context Graph. Isso é integração de produto, não prova de qualidade homogênea ou equivalência com uma API Engine.
88. **Qual deve ser o primeiro contrato de memória?** Separar interface de append/read/search, escopo e retention; definir origem, ACL, temporalidade e deleção antes do adapter. Não lançar API `Memory` genérica sem semântica.
89. **Onde ficam Skills?** Como instruções/capacidades versionadas que o host pode carregar e autorizar; Engine poderá fornecer parsing/lifecycle comum. A fotografia não prova registry/skill runtime completo.
90. **Prompt Management é só templates?** Não. Inclui identidade, versão, composição, variables, trust boundary, redaction, rollout e avaliação. Os consumidores devem poder inspecionar prompt efetivo sem vazar secrets.
91. **Quem controla system instructions?** O host define identidade e políticas confiáveis; Engine pode compor camadas tipadas. Conteúdo de usuário/modelo nunca deve sobrescrever instrução confiável por concatenação ambígua.
92. **Como tratar RAG contra prompt injection?** Marcar documentos recuperados como dados não confiáveis, preservar provenance, filtrar por ACL e não conceder capacidades por conteúdo. Retrieval não transforma texto em instrução confiável.
93. **Como medir retrieval?** Recall/precision em corpus anotado, ranking, freshness, isolamento de tenant, custo e relevância downstream. Uma busca que retorna documentos não prova boa recuperação.
94. **Qual papel do VectorStore?** CRUD/search vetorial com dimensões, filtros e scopes definidos. Ele não é, sozinho, Knowledge Base, índice de documentos nem memória de agente.
95. **Como deve funcionar Memory Consolidation?** Processo explícito que combina/expira observações com proveniência e possibilidade de corrigir/apagar. Evitar gravação automática de toda saída do modelo como fato permanente.
96. **O que é Semantic Cache?** Reutilização aproximada baseada em similaridade, com risco de resposta errada por colisão ou mudança de contexto. Exige scopes, freshness, thresholds e opt-out; não é cache transparente universal.
97. **Como evitar vazamento entre projetos?** Escopo/ACL devem chegar ao storage adapter e ser testados negativamente; filtragem apenas no prompt é insuficiente. Tenant/workspace não pode ser campo opcional em operações sensíveis.
98. **Como equilibrar contexto e custo?** Orçar por categoria, medir tokens, priorizar instruções e evidências, recuperar sob demanda e testar qualidade. Truncamento cego pode remover justamente o dado decisivo.
99. **Como dar continuidade após restart?** Persistir identidade da conversa, mensagens, estado relevante e referência a tools/artefatos em schema compatível; revalidar credenciais e efeitos externos no resume.
100. **Memória deve ser universal na Engine?** Mecanismos e contratos potencialmente sim; política de que fatos lembrar, por quanto tempo e para qual usuário pertence ao host e à configuração explícita.

## F. Tools, extensibilidade e integrações (101–120)

101. **Quais categorias de tools são necessárias?** Native Tools compiladas no core; Runtime Tools registradas pelo host/extensions; External Tools via processo/serviço/protocolo. Origem não deve alterar contrato de descoberta, policy e resultado.
102. **Uma tool precisa rodar dentro do processo?** Não. A Engine pode coordenar ferramenta isolada em subprocesso, sandbox ou serviço remoto. Execução gerenciada pelo runtime não significa execução in-process.
103. **A Engine executa tools hoje?** A fotografia tem contrato/preparo e policy de dispatch, mas o host executa adapters. Não anunciar catálogo/runner completo sem registro, scheduler, isolamento e execução demonstrados.
104. **O que `ToolDispatch` faz?** Valida schema/argumentos e prepara decisão sob estado e capacidades fornecidos. Não descobre toda tool, autentica o host, aplica sandbox nem prova que o efeito aconteceu.
105. **Como registrar Runtime Tools?** Registry versionado, schema de entrada/saída, identidade, capabilities, lifecycle e compatibilidade. A API precisa dizer se registro é por processo, instalação ou run.
106. **Como carregar plugins de terceiros?** Manifesto assinado/verificado, permissões mínimas, versionamento, isolamento e política de atualização. Não executar código arbitrário apenas porque instalador declarou uma tool.
107. **MCP é o quê para Engine?** Um protocolo de integração cliente, conforme a fronteira atual. A Engine não se torna automaticamente servidor MCP nem deve exigir MCP para tool local.
108. **Como tratar MCP tools?** Traduzir discovery/schema/result/error para contrato interno, preservar identidade do servidor e capabilities concedidas, limitar recursos e registrar auditoria. Schema remoto não é confiável por si só.
109. **O que são hooks?** Pontos de extensão observáveis em fases do runtime. Devem definir ordem, falhas, timeout, side effects e se podem bloquear; hook não deve contornar policy.
110. **Como tool calling se diferencia de function calling?** São termos de protocolo para o modelo propor invocações; a aplicação ainda precisa validar, autorizar e executar. Saída do modelo nunca é autorização.
111. **Como evitar tool confused deputy?** Vincular invocação a principal, scope e capabilities aprovados pelo host; validar novamente antes do efeito. Não aceitar IDs/paths fornecidos pelo modelo como prova de acesso.
112. **Como cancelar uma tool?** Passar cancellation token/deadline ao adapter e registrar resultado terminal ou incerto. Se adapter não suporta cancelamento, reportar limite e não fingir que a operação foi desfeita.
113. **Como representar efeitos de tool?** Distinguir `not_started`, `succeeded`, `failed` e `unknown`, com operation ID, resultado validado e reconciliação. Erro de transporte não prova que o efeito falhou.
114. **O que é tool batching?** Executar chamadas aprovadas em paralelo/ordem controlada. Deve declarar limite de concorrência, dependências, falha parcial, cancelamento e ordenação dos resultados.
115. **Como criar tool do Vectora Workbench?** Workbench fornece implementação de produto e UI; registra capability na Engine por contrato, mas a interação com usuário (ex. senha sudo) permanece no host.
116. **Quais tools devem ficar fora do núcleo?** Tools acopladas a GitHub, workbench, conta Vectora, interface Electron ou serviço comercial. Core pode oferecer interfaces/ports reutilizáveis.
117. **Como tratar descoberta de tools?** Catálogo deve ser filtrado por principal, scope e policy antes de apresentar ao modelo; registrar versão do catálogo usado no turno para replay.
118. **Como fazer versionamento de tool?** Nome estável mais versão de contrato, digest/implementation version e estratégia de compatibilidade. Replay não deve invocar silenciosamente uma implementação incompatível.
119. **Como testar tools externas?** Testes determinísticos de contrato, simuladores e integração com serviço real em ambiente isolado; incluir timeout, erros, redelivery, idempotência e cancelamento.
120. **Qual é a meta do framework extensível?** Permitir que consumers componham capacidades sem reimplementar governança, contexto e lifecycle; não forçar cada extensão a usar um modelo único ou executar dentro do core.

## G. Storage, ledger e messaging (121–140)

121. **Quais famílias de storage estão definidas?** SQL (SQLite/PostgreSQL), vector (LanceDB/Qdrant), cache/KV (NATS/Redis), além de filas e pub/sub conforme adapters. A seleção pode combinar um membro de cada família.
122. **SQLite é apenas cache local?** Não. Para instalação simples pode ser store durável vital, inclusive fonte local de ledger; regras de backup, locking, journaling e filesystem devem ser explícitas.
123. **PostgreSQL substitui SQLite sempre?** Não. Pode ser fonte de dados para SQL do consumer, mas o modelo definido mantém cópia SQLite para ledger resiliente. Replicação exige ordering, idempotência e reconciliação testados.
124. **O que significa SQLite-first ledger?** Primeiro gravar de forma durável local, e depois projetar para remoto. A aplicação deve sobreviver à indisponibilidade remota e reparar projeção sem perder sequência.
125. **O ledger é um banco SQL genérico?** Não. É trilha append-only de eventos/operações com sequência, identidade e integridade; consultas e mutações têm semântica própria.
126. **Como definir contratos entre os seis adapters?** Semântica comum por família, com diferenças explicitadas: transação, consistência, TTL, ordering, ack, retenção, query/filter e failure modes. Não esconder capacidades ausentes atrás de interface falsa.
127. **LanceDB e Qdrant são intercambiáveis?** Só no subconjunto contratual testado. Índices, filtros, persistência, deploy e semântica de distância podem divergir; adapters devem publicar capacidades.
128. **NATS e Redis são equivalentes?** Não integralmente. NATS/JetStream e Redis têm modelos diferentes de streams, ACK, KV, retenção e distribuição. API comum precisa nomear garantias mínimas e features extras.
129. **Queue e pub/sub são a mesma coisa?** Não. Queue distribui trabalho a consumidores e requer ack/retry/dedup; pub/sub distribui eventos a assinantes e pode ter entrega efêmera ou persistente conforme backend.
130. **O adapter `memory` é implementado?** A matriz prévia indica seleção que pode serializar `memory`, mas adapter retorna `Unsupported` em certas operações. Conferir branch atual antes de afirmar; seleção configurável não prova execução.
131. **Como tratar entrega duplicada?** Projetar pelo menos uma vez como possibilidade, usar message IDs/dedupe e consumidores idempotentes. Exactly-once ponta a ponta é uma afirmação forte que não decorre do broker.
132. **Como lidar com ordem de mensagens?** Definir escopo da ordenação (global, partição, run) e não presumir ordem entre workers. Sequence numbers e optimistic concurrency ajudam em stores, mas não ordenam qualquer broker.
133. **O que acontece se Redis/NATS cai?** Política por operação: fail fast, buffer durável local, retry bounded ou degradar. Ledger crítico não deve depender apenas do cache/broker.
134. **Como tratar mudança de schema no ledger?** Eventos versionados, readers compatíveis e migrações testadas em dados antigos; evitar reescrever história sem estratégia auditável.
135. **Como fazer backup?** Documentar snapshot consistente e WAL/checkpoint quando aplicável; testar restore, corrupção e versão. “Arquivo SQLite copiado” durante writes não é plano suficiente.
136. **Como definir retention?** Por tipo de dados, finalidade, tenant e obrigação; filas, traces, mensagens e ledger podem ter políticas diferentes. Retenção também deve cobrir export/delete e backups.
137. **O que são garantias de transação entre SQL e vector store?** Normalmente não há transação distribuída comum. Usar outbox/reconciliation ou estados intermediários e deixar consistência eventual explícita.
138. **Como evitar dual-write?** Uma fonte de verdade e projeções idempotentes; registrar intenção/outbox na mesma transação local quando possível. Não gravar em dois serviços como se ambos fossem atômicos.
139. **Que testes de storage são essenciais?** Conformance por adapter, isolamento de scope, idempotência, TTL/retention, restart, reconnect, falhas parciais, redelivery, concorrência, migração e restore.
140. **A matriz de testes reais está completa?** Não foi demonstrada integralmente na auditoria: testes de serviços podem ser condicionais e não cobrir reconnect/restart/falhas combinadas. Deve-se reportar execução real por serviço e ambiente.

## H. Policy, segurança e sandbox (141–160)

141. **O que a policy da Engine faz?** Valida snapshots e vínculos de autorização de tools sob contrato genérico. **Estado:** experimental, com persistência SQLite específica; não autentica a pessoa aprovadora.
142. **O que um approval receipt deve vincular?** Principal validado pelo host, run, tool call, argumentos/digest, policy revision, escopo, expiry e decisão. Campos exatos têm de seguir spec versionada.
143. **Quem revalida aprovação?** Host autentica principal; Engine verifica estado mais recente, revisão, expiração e vínculo persistido. Ao reabrir/replay, não reutilizar recibo stale.
144. **O que é default-deny?** Negar quando capability, policy ou escopo não estão explicitamente autorizados. Falta de configuração ou erro de parser não deve virar allow.
145. **A Engine já tem sandbox?** Não como sandbox de execução geral provado na fotografia. Guardrails de dispatch não isolam processos nem limitam filesystem/rede/CPU.
146. **Quem deve fornecer sandbox?** Pode ser adapter local/containers/processo remoto implementado pela Engine ou host, mas contrato precisa declarar isolamento real, capabilities e limites. Não chamar subprocesso de sandbox sem controles.
147. **Como lidar com shell e code execution?** Separar montagem do comando de execução, validar escopo, executar em ambiente isolado e retornar stdout/stderr/exit/status com limites. Proteção depende do OS e configurador.
148. **Filesystem access é capability do core atual?** Não demonstrada como tool executável. A Engine pode definir interfaces; o Vectora fornece sua tool e contexto de workspace até existir runtime comum aprovado.
149. **Como proteger secrets?** Secret injection mínima, masking, redaction de traces e proibição de secrets em prompts/ledger. Logs estruturados também precisam de classificação e limites de retenção.
150. **Como prevenir prompt injection em tools?** Tratar conteúdo externo como dado, permitir policy fora do modelo, confirmar efeitos sensíveis e aplicar validação no adapter. Prompt adicional sozinho não é controle de segurança.
151. **Guardrails são moderação?** Mais amplos: validação de input/output, tool policy, schema, budgets e invariantes. Cada guardrail deve dizer se bloqueia antes/depois da chamada e como lida com erro.
152. **Guardrails antes/depois da tool bastam?** Não. Ainda há risco no host, adapter, dependência, credencial e efeito externo. Defense-in-depth exige isolamento e autorização no limite do efeito.
153. **Como estruturar permisos?** Princípio do menor privilégio, capabilities atômicas, escopo temporal e resource binding, com revogação e auditoria. Evitar papéis amplos “agent admin”.
154. **Como lidar com human-in-the-loop?** Pausa durável, solicitação identificável, deadline, principal autenticado no host, receipt vinculado e retomada consistente. Um modal visual não é workflow HITL completo.
155. **Como tratar dados pessoais no contexto?** Minimizar, limitar escopo e retention, obter consentimento quando aplicável e facilitar remoção. Não persistir tudo por padrão sob a justificativa de memória.
156. **Como desenhar tenant isolation?** Incluir tenant/workspace nas chaves, queries e policies em todos os adapters e provar testes cruzados negativos. Namespaces em vector DB sem filtro rigoroso podem vazar.
157. **O que deve entrar no audit log?** Actor/authority, operação, resource scope, decisão, correlation IDs e resultado essencial sem segredo. Logs de auditoria têm retenção e controle de acesso próprios.
158. **Como lidar com vulnerabilidade de plugin?** Isolamento, assinatura/proveniência, allowlist de versões, capabilities e desligamento rápido. Um plugin third-party é código hostil até revisão apropriada.
159. **Quais limites do VerificationGate?** Avalia evidência estruturada/política e produz report; não roda testes nem prova que evidência declarada é autêntica. Executor e autenticidade pertencem à cadeia confiável.
160. **Como provar segurança?** Threat model, adversarial tests, revisão de boundaries, dependency scanning e incident drills. Um type system e testes unitários reduzem classes de erro, não provam sistema seguro.

## I. Bindings, APIs e compatibilidade (161–180)

161. **Por que Python e TypeScript?** Python atende backend atual do Vectora; TypeScript atende GH Bot. Um core Rust evita portar a mesma semântica em duas linguagens.
162. **Bindings são wrappers superficiais?** Devem ser finos quanto à lógica e robustos quanto a idioms, tipos, async, exceptions/errors e lifecycle da memória. Semântica de negócio permanece em Rust.
163. **A paridade Python/TypeScript é total?** Não foi demonstrada. A fixture compartilhada cobre subset de contratos; funções e testes não enumerados podem divergir.
164. **Como provar paridade?** Fixture comum com entradas/saídas/erros/cancelamento, executada nos três níveis; somar teste específico de runtime e instalação por linguagem. Comparar apenas JSON feliz é insuficiente.
165. **Como versionar contratos?** Constantes/schema versionados, política de compatibilidade, teste de payload antigo e erro tipado para versão não suportada. Mudança semântica exige bump e plano de migração.
166. **Como modelar erros?** Categorias estáveis (validation, conflict/stale, unavailable, timeout, cancelled, unsupported, internal) com dados seguros. Mensagem livre não é API suficiente.
167. **Como tratar `null`?** Distinguir ausência, null explícito e valor; compatibilidade de provider deve preservar semântica JSON e não serializar opção ausente como null sem intenção.
168. **Como oferecer async coerente?** Bindings devem respeitar cancellation/timeout do core, não bloquear event loop e fechar recursos. Matar apenas a coroutine pode deixar socket ou efeito ativo.
169. **Como publicar Rust, PyPI e npm?** Artefatos por plataforma/ABI, assinatura/proveniência, testes de instalação limpa, SBOM e compatibilidade. Intenção de publicar não significa pipeline pronto.
170. **Quais riscos de N-API/PyO3?** ABI, toolchain, linking, wheel/native target, arquitetura e artefatos stale. Build local de um addon não prova instalação em todas plataformas suportadas.
171. **Como manter schema comum?** Fonte canônica ou geração automatizada, fixture de conformidade e verificação contra drift. Stubs devem ser gerados e comparados, não editados divergentes sem motivo.
172. **Como definir API pública mínima?** Expor apenas conceitos com consumer, semântica, erros, ownership, compatibilidade, cancelamento e evidência. APIs exploratórias devem permanecer internas/feature-gated.
173. **Quando usar API sync vs async?** Sync para transformações determinísticas sem I/O; async para rede/stream/espera. Não bloquear runtime com operação de longa duração escondida em função sync.
174. **Como evitar binding que promete demais?** Documentar capability por método e backend; retornar `Unsupported` cedo se necessário. Não sinalizar genericamente `storage=memory` quando certas operações falham somente depois.
175. **Como testar pacote distribuído?** Empacotar, instalar em ambiente limpo por OS/arch/Python/Node suportados, executar smoke real e validar loader/nativo. Workspace com dependências não detecta arquivos ausentes no tarball.
176. **Go deve ter binding?** É uma proposta registrada como issue/documentação, ainda não capability. Avaliar cgo/FFI, cross-compilation, async, distribuição e casos consumer antes de assumir prioridade.
177. **Como lidar com ABI do core Rust?** Versionar API e formato de artefatos, fixar toolchain e testar consumers; preferir fronteira C/N-API/PyO3 mantida por tooling suportado em vez de expor layout Rust interno.
178. **Como oferecer migrations aos bindings?** Contratos antigos com leitura/migração explícita, fixtures dos snapshots e erros recuperáveis. Não alterar formato persistido sem compatibilidade de dados.
179. **O que uma release deve declarar?** APIs estáveis/experimentais, plataformas suportadas, breaking changes, adapters testados, limitações, hashes e proveniência. “Engine 1.0” sem critérios de estabilidade é rótulo vazio.
180. **Qual papel das docs geradas?** Refletir assinaturas reais e estado da versão; gerar a partir de schemas/stubs quando possível. A sabatina é análise viva, não substituto da referência normativa da API.

## J. Comparação com frameworks e operação (181–200)

181. **LangChain é comparável diretamente à Engine?** Só por camadas. O stack oficial separa abstrações de agentes, LangGraph para controle/orquestração e Deep Agents em nível mais alto; Engine pretende compartilhar runtime Rust e bindings, ainda em maturidade distinta. [LangChain OSS](https://www.langchain.com/oss-overview)
182. **O que aprender do LangGraph?** Persistência/checkpointers, execução por grafo, interrupção humana e recuperação são preocupações distintas do prompt. Não copiar o modelo de grafo sem comparar com contratos/eventos da Engine. [Persistência](https://langchain-ai.github.io/langgraphjs/how-tos/cross-thread-persistence-functional/)
183. **Deep Agents é a mesma coisa que harness?** É uma camada opinativa de agente com planejamento, memória e subagentes, conforme o stack LangChain. A Engine pode oferecer mecanismos de nível inferior e deixar comportamento opinativo ao host.
184. **Como se compara OpenAI Agents SDK?** Oferece primitivas Python de agents/tools/handoffs/guardrails, sessões, HITL e tracing; Engine difere por núcleo Rust multi-binding e contratos de storage/durabilidade em desenvolvimento. Não há comparação de performance comprovada. [SDK](https://openai.github.io/openai-agents-python/)
185. **O que aprender com guardrails do OpenAI SDK?** Guardrails de input/output e de tool têm pontos de aplicação diferentes; algumas verificações podem rodar em paralelo com o agente. Engine deve declarar timing e possibilidade de efeito antes de bloquear. [Guardrails](https://openai.github.io/openai-agents-python/guardrails/)
186. **O que aprender do tracing do OpenAI SDK?** Traces podem capturar conteúdo sensível por padrão. Engine precisa de redaction, controles de exportação e documentação de dados, não somente spans ricos. [Tracing](https://openai.github.io/openai-agents-python/tracing/)
187. **O que aprender dos testes OpenAI Agents?** Harnesses determinísticos ajudam a testar provider-neutral workflows, mas integração real ainda precisa validar rede, provider e efeito externo. Separar conformance e service tests.
188. **Como Haystack difere?** Enfatiza componentes, pipelines e document stores para RAG/agents; pipeline conectado é mais determinístico, agent delega passos ao modelo. É útil para separar pipeline declarativo de loop agêntico. [Intro](https://docs.haystack.deepset.ai/docs/intro)
189. **O que aprender do Haystack em produção?** Avaliação, tracing e logs pertencem ao ciclo de produção do pipeline. Engine deve construir observabilidade correlacionada, não deixar isso como configuração posterior. [Production usage](https://docs.haystack.deepset.ai/docs/production-usage)
190. **Como LlamaIndex se compara?** Tem workflows event-driven que podem compor agents, RAG e extração; separa workflow core de utilitários/prebuilt agents. Isso reforça modularidade e extensão por nível. [Workflows TS](https://developers.llamaindex.ai/typescript/framework/modules/agents/workflows/)
191. **Como AutoGen se diferencia?** Core é runtime event-driven/distribuído com modelo de atores e comunicação assíncrona; AgentChat é camada mais opinativa de equipes. Engine pode aprender lifecycle e messaging sem tornar multiagente obrigatório. [AutoGen Core](https://microsoft.github.io/autogen/stable/user-guide/core-user-guide/index.html)
192. **O que aprender de AutoGen Teams?** Começar com agente único e só adicionar equipe se casos mostrarem insuficiência; definir terminação e state resume. Multiagente não deve ser feature de marketing sem orçamento e avaliação.
193. **Como Pydantic AI aborda execução durável?** Integra agentes a motores como Temporal/DBOS/Prefect/Restate para journaling e replay. A lição é separar agent framework de durable workflow engine e não confundir snapshot com replay de efeitos. [Durable execution](https://pydantic.dev/docs/ai/capabilities/durable_execution/overview/)
194. **A Engine deve implementar seu próprio Temporal?** Não por padrão. Pode começar com garantias locais/SQLite e adapters bem delimitados; se precisar orquestração distribuída, avaliar integração com engine durável existente versus custo de construir scheduler/replay robusto.
195. **Como Vercel AI SDK/Workflow se compara?** AI SDK prioriza experiência TypeScript de modelos e ferramentas; WorkflowAgent adiciona passos duráveis, retry e suspensão. É referência para ergonomia TS, não para core Rust multi-binding. [WorkflowAgent](https://vercel.com/kb/guide/what-is-workflowagent)
196. **Como Mastra se diferencia?** Framework TS com agentes/workflows/storage e observabilidade correlacionada por traces, logs, uso e métricas. É referência forte de visibilidade integrada; a Engine precisa atingir equivalência operacional antes de proclamar harness maduro. [Observabilidade](https://mastra.ai/docs/observability/overview)
197. **O que aprender de DSPy?** Tratar programa/prompt como objeto avaliável e otimizar contra métricas/datasets, não confiar em prompt artesanal. DSPy não substitui lifecycle, sandbox ou storage durável de agente. [DSPy](https://dspy.ai/)
198. **Semantic Kernel e CrewAI entram na comparação?** Sim, como exemplos de ecossistemas com plugins/funções (Semantic Kernel) e flows/agent orchestration (CrewAI), mas cada um deve ser comparado por fonte oficial, versão e camada; não deduzir recursos por popularidade.
199. **Como comparar sem benchmarks enganosos?** Definir workloads equivalentes, mesmo provider, ferramentas, budgets e hardware; publicar dataset, SHA, ambiente, configuração, taxa de sucesso, latência/custo e intervalos. Docs de vendors descrevem seus próprios produtos, não avaliação independente.
200. **Quando esta sabatina está atualizada e pronta para ser referência?** Quando cada resposta está reconciliada com código/spec/plano mais recente, marcada por estado e ligada a evidência, snapshots e data; qualquer mudança relevante dispara revisão. O prompt do goal loop torna essa atualização obrigatória em todos os ciclos.

## Manutenção obrigatória desta sabatina

1. No início de cada ciclo do goal loop, leia esta sabatina junto da spec,
   análise e checklist da etapa. Use-a para formular perguntas novas e localizar
   claims que precisam de evidência; ela não substitui os contratos normativos.
2. Depois de qualquer mudança relevante em código, testes, bindings, storage,
   segurança, plano, spec, consumers ou fontes, atualize todas as respostas
   afetadas no mesmo ciclo. Inclua perguntas novas quando aparecer uma decisão,
   capability, falha ou comparação importante; remova ou marque respostas
   obsoletas em vez de deixá-las parecer atuais.
3. Cada resposta sobre estado deve apontar, quando útil, para arquivo/spec/teste,
   commit SHA e consumer snapshot. Marque explicitamente **Implementado
   experimentalmente**, **Parcial**, **Planejado**, **Ausente**, **Decisão** ou
   **Externo**. Uma issue/spec/PR aprovado não prova que código está completo;
   código presente não prova adoção, confiabilidade nem produção.
4. Ao consultar framework externo, registrar versão/data e usar documentação ou
   repositório oficial. Descrever o que a fonte diz; separar inferência e não
   transformar página comercial em benchmark neutro. Revisar links quando a
   comparação for retomada.
5. Antes de encerrar cada ciclo, reconciliar o SHA da Engine e snapshots de
   Vectora/GH Bot usados, revisar todas as respostas tocadas pelo diff, conferir
   consistência com a matriz de conformidade e atualizar data/escopo desta
   fotografia. Se um snapshot não foi revalidado, dizer isso claramente.
6. Se não houver mudança de evidência, não inventar mudança apenas para alterar
   o documento; registrar que a sabatina foi revisada no checklist/relatório da
   etapa. O requisito é manter respostas corretas continuamente, não criar
   churn textual.

## Referências primárias consultadas

Páginas oficiais consultadas em 11 de novembro de 2026. Elas descrevem os
projetos conforme seus próprios mantenedores; não constituem comparação
independente de performance ou qualidade.

- [LangChain OSS: LangChain, LangGraph e Deep Agents](https://www.langchain.com/oss-overview)
- [LangGraph JS: persistência entre threads](https://langchain-ai.github.io/langgraphjs/how-tos/cross-thread-persistence-functional/)
- [OpenAI Agents SDK](https://openai.github.io/openai-agents-python/)
- [OpenAI Agents: guardrails](https://openai.github.io/openai-agents-python/guardrails/)
- [OpenAI Agents: tracing](https://openai.github.io/openai-agents-python/tracing/)
- [OpenAI Agents: testing](https://openai.github.io/openai-agents-python/testing/)
- [Haystack: introdução](https://docs.haystack.deepset.ai/docs/intro)
- [Haystack: produção](https://docs.haystack.deepset.ai/docs/production-usage)
- [LlamaIndex TypeScript: Workflows](https://developers.llamaindex.ai/typescript/framework/modules/agents/workflows/)
- [AutoGen Core](https://microsoft.github.io/autogen/stable/user-guide/core-user-guide/index.html)
- [AutoGen: AgentChat Teams](https://microsoft.github.io/autogen/stable/user-guide/agentchat-user-guide/tutorial/teams.html)
- [Pydantic AI: Durable Execution](https://pydantic.dev/docs/ai/capabilities/durable_execution/overview/)
- [Vercel: WorkflowAgent](https://vercel.com/kb/guide/what-is-workflowagent)
- [Mastra: Observability](https://mastra.ai/docs/observability/overview)
- [Mastra: Storage](https://mastra.ai/docs/storage)
- [Mastra: Resumable Streams](https://mastra.ai/docs/agents/resumable-streams)
- [DSPy](https://dspy.ai/)
- [Microsoft Semantic Kernel: agent functions/plugins](https://learn.microsoft.com/en-us/semantic-kernel/frameworks/agent/agent-functions)
- [CrewAI: Flows](https://docs.crewai.com/en/concepts/flows)
- [Vectora issue #387](https://github.com/vectora-company/vectora/issues/387), usada como contexto de produto e confrontada com os snapshots de código.
- [Análise cruzada local: Akita, Engine, Vectora e GH Bot](03-analise-cruzada-akita-engine-vectora-gh-bot.md), incluindo escopo e limites da leitura do livro.

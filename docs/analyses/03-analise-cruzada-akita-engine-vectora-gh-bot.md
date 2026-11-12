# Análise cruzada: Akita, Engine, Vectora e GH Bot

## Objetivo e limites

Este documento cruza as ideias de _Competente o Suficiente_, de Fabio Akita,
com o estado verificável da Vectora Engine, do backend do Vectora e da proposta
do GH Bot. A pergunta não é qual projeto tem “mais IA”, mas quais mecanismos
são realmente compartilhados, quais já executam em um produto e onde estão as
fronteiras que evitam manter motores duplicados.

A leitura de referência foi feita sobre a edição local em português: conferi o
sumário/outline de todos os capítulos, interlúdios, epílogo e apêndices e li
diretamente as seções relevantes para harness, contexto, medição, segurança,
revisão e operação. O cruzamento não trata cada relato do livro como requisito
de arquitetura. O livro reúne experiências e opiniões datadas; não é uma
especificação da Engine. As conclusões abaixo usam as teses relevantes e
apontam onde a evidência do código confirma, limita ou contradiz uma aplicação
direta dessas teses.

### Snapshots usados

| Fonte            | Snapshot consultado                                                                                                           | Uso                                                                           |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Livro            | `CompetenteOSuficiente` em `b0e7e1c0ec1411113ae5e8d893ebe05ead89535e`                                                         | Argumentos e experiências do autor; edição local em `book/pt/`.               |
| Engine           | topo da stack, PR [#34](https://github.com/vectora-company/engine/pull/34), commit `a2afb65dc898a6214dd83a6473967132b6a9ac79` | Contratos e fixture de conformidade disponíveis nesta etapa.                  |
| Vectora          | `origin/master` em `bf1732ac92eac21671881cc34a530c2ddb7186de`                                                                 | Implementação atual do backend consumidor para a comparação.                  |
| GH Bot           | `main` em `b489b90263e3a4ace94cfdce45763489918a3671`                                                                          | Bootstrap e documentação atuais; integração com a Engine ainda é roadmap.     |
| Contexto Vectora | issue [#387](https://github.com/vectora-company/vectora/issues/387)                                                           | Tese de produto e perguntas de arquitetura; não substitui inspeção do código. |

Os snapshots são diferentes entre si e representam o estado consultado nesta
análise, não uma afirmação de que os repositórios continuam nesses commits.
“Existe no repositório” também não significa “está completo, habilitado por
padrão ou validado em produção”.

## Síntese

- **A Engine é a camada reutilizável em formação.** Ela já tem contratos Rust
  versionados para providers, loop modelo/tool, lifecycle, política,
  verificação, ledger e adapters de storage/messaging, com bindings Python e
  TypeScript para parte dessas APIs. Algumas capacidades são experimentais ou
  parciais; os bindings não provam paridade global.
- **O Vectora é hoje o sistema integrado mais amplo.** Seu backend Python
  conecta providers, contexto, tools, persistência, sandbox, UI/workbenches,
  tarefas e operação num produto. Isso não torna cada módulo uniforme ou
  infalível, mas significa que há execução integrada que ainda não existe na
  Engine.
- **O GH Bot é um terceiro consumidor, não um segundo nome para o Vectora.** A
  proposta é um produto TypeScript mais focado em GitHub. O bootstrap documenta
  CLI, dashboard e modos de operação, mas a integração real com Services,
  GitHub e Engine ainda não deve ser presumida a partir do desenho.
- **A Engine não tem mais funcionalidades de usuário que o Vectora neste
  snapshot.** Ela tem contratos comuns e componentes de runtime próprios que o
  Vectora ainda não consome. O Vectora tem mais capacidades conectadas e
  específicas de produto. Essa diferença é de fronteira e estágio, não uma
  classificação de qualidade.

## O que o livro ajuda a decidir

Referências de capítulos abaixo usam os títulos do próprio sumário local.
São princípios de engenharia extraídos de relatos do autor, não requisitos
atribuídos a ele.

| Ideia do livro                                                                                                                                                                                                                        | Consequência para a Engine                                                                                                                                                                        | Consequência para Vectora e GH Bot                                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **O modelo e o harness formam o comportamento observado** (cap. 3, “Um modelo com mãos”). Ferramentas, instruções, contexto e loop alteram o resultado.                                                                               | Versionar contratos e medir o conjunto executado, em vez de atribuir qualidade apenas ao provider. A Engine deve deixar explícito quais decisões pertencem ao host.                               | Comparar o mesmo caso, configuração e provider no sistema consumidor; não atribuir uma melhora ao Rust sem separar mudanças de prompt, tools e contexto.                                                                                                                                                 |
| **Contexto tem limite e compactação perde informação** (cap. 2, “A memória de um peixinho”). Memória recuperada é pista e pode estar errada; RAG é útil em situações específicas, não uma resposta universal.                         | `VectorStore`, embeddings e ledger não equivalem a memória, Context Planner, RAG ou `ContextProjection`. Essas APIs ainda não existem no snapshot da Engine.                                      | O Vectora já tem planner, projeção/compactação, memória, RAG e Context Graph. Devem continuar avaliáveis por provenance, escopo, orçamento e qualidade de recuperação. O GH Bot pode começar sem copiar toda essa camada.                                                                                |
| **Uma avaliação só vale se o oráculo estiver correto** (caps. 8 e 11; apêndice “Como ler um benchmark ou um anúncio”). O autor relata ter corrigido o próprio corretor; testes que simulam uma API inventada podem certificar o erro. | A fixture cruzada do PR #34 é uma amostra de contratos, não benchmark de agente ou prova de paridade. Testes devem observar comportamento real de adapters e erros, e registrar SHA/configuração. | A suíte/replay do Vectora pode ajudar como fonte de casos, mas resultados precisam ser reavaliados contra dependências reais e a intenção do caso. O GH Bot precisa de casos de revisão GitHub que meçam utilidade e falsos positivos, não apenas quantidade de comentários.                             |
| **Um software funcional ainda pode falhar em operação** (caps. 6 e 7). Deploy revela concorrência, recuperação, observabilidade e requisitos não escritos.                                                                            | Persistência, cancelamento, recuperação, erros tipados e correlação são contratos de primeira classe. A matriz atual registra lacunas de testes reais de serviços e integração.                   | O host continua responsável por processo, UI, autenticação, recursos e operação. O Vectora deve fornecer evidência do comportamento real; o GH Bot precisa ter idempotência de delivery e recuperação compatível com execução contínua.                                                                  |
| **Permissão em prompt não é isolamento** (cap. 15 e apêndice B, “Kit mínimo para usar agentes com segurança”). A jaula, desfazer, backup e limites efetivos são camadas diferentes.                                                   | `ToolPolicy` vincula dispatch/aprovação, mas não autentica a pessoa, executa sandbox ou limita o processo. A Engine não pode anunciar essa policy como uma sandbox.                               | O host autentica identidade, aplica autorização, isola credenciais e executa o adapter sob limites reais. No Vectora, o código de sandbox existe, mas a configuração pode operar sem sandbox; isso precisa ser entendido por deployment. No GH Bot, tokens do GitHub nunca devem ser expostos ao prompt. |
| **Código gerado ainda exige revisão humana e segurança de supply chain** (cap. 16). A triagem automatizada não transfere responsabilidade.                                                                                            | APIs de policy, evidência e execução precisam declarar exatamente o que validam. A Engine não deve transformar um resultado do modelo em prova.                                                   | GH Bot pode sugerir revisão, mas resultado é recomendação com evidências. O Vectora oferece superfícies de inspeção/edição; a aprovação e o merge continuam decisões do usuário/equipe.                                                                                                                  |
| **Instruções persistentes ajudam, mas não eliminam julgamento** (cap. 9 e apêndice técnico sobre Skills).                                                                                                                             | Prompt management, Skills e configuração ainda não são APIs da Engine neste snapshot; não se deve inferir que já há sistema de extensões por haver providers e loop.                              | Regras de projeto do Vectora ou do GH Bot são dados do consumidor e devem ser escopadas, inspecionáveis e versionadas.                                                                                                                                                                                   |
| **Multiagente não é ganho automático** (cap. 11, seção sobre combinação de modelos). Dependência e coordenação podem aumentar latência e custo.                                                                                       | Não há orquestração de subagentes na Engine. Se entrar, precisa de contrato de orçamento, prazo, parent run, cancelamento e resultados.                                                           | O Vectora tem subagentes e scheduling; seus benefícios devem ser medidos por classe de tarefa. GH Bot não precisa adotar delegação para cumprir sua função.                                                                                                                                              |

### Como ler as conclusões do livro

O livro não prova que Rust torna o agente melhor, que uma arquitetura
específica vencerá nem que um produto concorrente carece de uma capacidade.
Ele oferece um método útil: formular hipótese, definir um oráculo independente,
rodar casos repetíveis, inspecionar falhas e preservar a decisão humana. A
Engine deve aplicar esse método aos contratos e aos consumidores, sem importar
conclusões de benchmarks de coding para workloads diferentes.

## Comparativo de capacidades

“Engine” significa o commit do PR #34 indicado acima; “Vectora” significa o
snapshot `origin/master` indicado acima. A coluna GH Bot distingue proposta
documentada de implementação já observada.

| Área e vocabulário da Engine              | Engine                                                                                                                                                                                                                                                      | Backend Vectora                                                                                                                                                                                                                                                                                                  | GH Bot                                                                                                                                                                                                             |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **`Chat` / providers / streaming**        | Core Rust com contrato OpenAI-compatible, adapters nativos para Anthropic, Google e Cohere, fallback explícito, embeddings e rerank; bindings Python e TS para superfícies selecionadas. Estado experimental, não cobertura completa de todos os providers. | `backend/llm/` tem adapters e clientes de mais provedores, fallback, configuração de modelos, streaming e fluxos multimodais. É mais integrado ao produto, mas possui implementação Python própria, ainda não substituída pela Engine.                                                                           | TypeScript será host/consumidor. Configuração de providers e modelos está na proposta; `EngineClient`/execução integrada ainda não deve ser tratada como concluída.                                                |
| **`RunLifecycle`**                        | Máquina de estados Rust versionada e stores SQLite especializados para lifecycle. Não autentica identidade do host.                                                                                                                                         | `AgentRun` modela estados, eventos, checkpoints, aprovações, tools, artifacts, evidências e métricas no armazenamento do produto. Não é o mesmo contrato automaticamente.                                                                                                                                        | Deve criar/associar execuções de revisão a eventos GitHub, com identidade e tenant resolvidos pelo host.                                                                                                           |
| **`ModelToolLoop`**                       | Contrato de coordenação e persistência SQLite para iniciar, aceitar, submeter resultado, recuperar, cancelar e reconciliar. Não chama provider nem executa adapters por si só.                                                                              | `backend/engine/conversation_loop.py` executa loop imperativo: remonta histórico persistido, chama LLM em streaming, executa lote de tools, trata limites, HITL e eventos. Tem execução integrada hoje.                                                                                                          | Deve delegar a execução genérica à Engine; mapeamento de delivery, escopo do repositório e publicação de review são comportamento GH Bot.                                                                          |
| **`ToolDispatch`**                        | Valida/prepara chamada, schema e estado; default-deny. Não descobre nem executa ferramentas.                                                                                                                                                                | Registry/decorators e `tool_batch.py` descobrem/despacham tools, tratam lotes/concorrência e conectam efeitos à UI e à persistência.                                                                                                                                                                             | Registrará tools de GitHub que o produto autorizar, sem colocar API GitHub no núcleo genérico.                                                                                                                     |
| **`ToolPolicy` / aprovação**              | Policy snapshot e vínculo de aprovação persistidos em SQLite; verifica escopo e binding. O host autentica quem aprovou e aplica a decisão efetiva.                                                                                                          | HITL/guardrails e políticas de produto ficam ligados ao loop, sessão e tools. Autorização de usuário/workspace é do backend Vectora.                                                                                                                                                                             | Regras de revisão, permissão de escrita e tokens GitHub são do Bot/host. Services pode fornecer login/entitlements; não deve ser presumido como proxy do tráfego de modelo ou GitHub.                              |
| **`VerificationGate` / evidência**        | Avalia evidência estruturada e persiste resultados em store específico. Não roda testes, não executa checks e não prova que evidência declarada seja verdadeira.                                                                                            | Harness pode executar ferramentas/checks e registrar artifacts/evidências em AgentRun; a disponibilidade de um evaluator não significa que toda execução tenha um gate obrigatório.                                                                                                                              | Pode verificar o resultado da revisão por regras definidas pelo produto, mas não deve alegar que “review concluído” prova ausência de bugs.                                                                        |
| **Ledger / `SqliteLedger` / checkpoints** | Ledger SQLite append-only, reservations/checkpoints e `ResilientLedger` SQLite-first com projeção PostgreSQL. PostgreSQL não substitui stores especializados de lifecycle/loop/policy; não é SQL genérico.                                                  | Tem stores nativos de sessão, AgentRun, ledger e providers SQL locais/remotos. Schemas de produto e política de retenção continuam próprios.                                                                                                                                                                     | Precisa mapear deliveries idempotentes e runs; pode usar storage de Engine onde o contrato servir, além de estado específico de instalação/GitHub.                                                                 |
| **`VectorStore` / KV / queue / pub-sub**  | Adapters LanceDB/Qdrant, NATS/Redis para KV, fila e pub/sub. Garantias variam por adapter. Seleção `memory` pode ser serializada, mas adapter retorna `Unsupported`; não há RAG pronto.                                                                     | Integra SQLite/LanceDB/NATS nativos e opções remotas; tem memória, indexação, Context Graph e operação de produto associadas.                                                                                                                                                                                    | Storage local SQLite está no desenho inicial; escala multi-instância exige configuração remota e coordenação. Não deve herdar RAG só porque há VectorStore na Engine.                                              |
| **`ContextProjection` / memória / RAG**   | Ausentes como API de runtime no snapshot. Embedding + `VectorStore` não constituem pipeline de documentos, memória ou seleção de contexto.                                                                                                                  | `context_planner`, `context_compaction`, memória, RAG, retrieval/evaluation e `context_graph` aparecem no backend. A issue #387 chama partes disso por nomes abrangentes; a existência do módulo não comprova qualidade uniforme.                                                                                | Não é requisito portar contexto completo do Vectora. O Bot deve fornecer contexto do PR/repositório necessário, com origem e limites, e evoluir por evidência.                                                     |
| **Tools e integrações**                   | Sem catálogo de tools do Vectora, integração GitHub, browser, shell, filesystem ou execução de efeito externo no core atual.                                                                                                                                | `backend/tools/` inclui filesystem, Git/GitHub, terminal, browser/CDP, computer use, web, MCP cliente, memória, busca, integrações de produto e outras tools. Esta é uma capacidade integrada que a Engine não tem.                                                                                              | Tools primárias são eventos/PRs/issues/repositórios GitHub. A proposta é integração direta via GitHub Action/endpoint, com modos hospedado e self-hosted.                                                          |
| **Sandbox / execução isolada**            | Não oferece executor/sandbox. `ToolPolicy` não isola processo, filesystem ou rede.                                                                                                                                                                          | `backend/sandbox/` contém backends local/Linux, macOS, Docker, SSH, Modal e Singularity e políticas/limites. O próprio `run_sandboxed` documenta caminho sem wrapper se sandbox não estiver habilitado; portanto não presumir isolamento obrigatório.                                                            | Precisa isolar segredos e efeitos do bot conforme deployment e permissões; não reutilizar a existência da policy Engine como prova de sandbox.                                                                     |
| **Subagents / scheduling / workflow**     | Ausentes como APIs atuais. `RunLifecycle` não é orquestração multiagente.                                                                                                                                                                                   | Subagents/SOULs, background tasks, DAG/queue e scheduler estão no backend. Catálogo atual tem dez perfis nomeados; isso não prova que toda delegação seja útil.                                                                                                                                                  | GH Bot pode começar com uma execução por evento; paralelismo e delegação são decisão posterior baseada em custo, idempotência e latência.                                                                          |
| **Sessão / UI / Workbenches**             | Não contém aplicação, sessão de conversa do produto ou workbench.                                                                                                                                                                                           | Vectora App integra Electron/React, estado de sessão, streaming SSE, Files, Git, Plan, Tasks/Kanban, Browser, Storage/Memory, Context Graph, Library e Terminal. A issue #387 chama a tese de workspace compartilhado de “workbenches simétricas”; essa é uma caracterização de produto, não contrato da Engine. | O GH Bot prevê dashboard React/TanStack, CLI global e modo background/tray conforme bootstrap. Ele não deve depender da UI ou processo Electron do Vectora.                                                        |
| **Avaliação / benchmarking**              | Rust unit tests, testes dos bindings e fixture comum do PR #34 para casos escolhidos. Não é uma suite de benchmark de qualidade do agente nem prova de equivalência global.                                                                                 | `backend/evaluation/` oferece cassette, recording, replay e runner; há avaliação de retrieval em outras áreas. Não chamar isso de benchmark completo sem validar datasets, oráculos e execução em ambiente limpo.                                                                                                | Ainda precisa de suite própria para taxa de revisão útil, falsos positivos, cobertura de cenários, latência/custo e recuperação de delivery.                                                                       |
| **Observabilidade / custo**               | Alguns contratos carregam uso/estado e requisitos documentais pedem correlação, duração, retry e redaction; a matriz de conformidade registra lacunas operacionais.                                                                                         | Há logging, telemetria, tracing e AgentRun metrics em vários módulos; isso não equivale a observabilidade consistente ponta a ponta.                                                                                                                                                                             | Precisa correlacionar delivery GitHub → run Engine → tool → comentário/resultado, sem vazar token ou conteúdo privado.                                                                                             |
| **Identidade / billing / UX do produto**  | Fora do core.                                                                                                                                                                                                                                               | Conta, workspace, UX, workbench, onboarding, licença e integração de Services são do Vectora/Services.                                                                                                                                                                                                           | Login Vectora Services e entitlement conforme proposta; GitHub connections, settings e UI são do GH Bot. Serviços externos devem ser usados por contratos explícitos, não como dependência implícita de todo dado. |

## O que a Engine tem a mais que o Vectora hoje

“A mais” aqui significa **formalizado no snapshot da Engine e ainda não
consumido como contrato comum pelo Vectora**, não mais capacidade integrada ou
maior maturidade.

1. **Um núcleo canônico Rust reutilizável**, com APIs consumíveis diretamente
   por Rust e bindings que encaminham operações ao mesmo core para Python e
   TypeScript. O Vectora ainda executa sua implementação Python própria.
2. **Contratos explícitos e versionados** para `RunLifecycle`, `ModelToolLoop`,
   `ToolDispatch`, `ToolPolicy`, `VerificationGate`, storage e messaging. O
   Vectora possui estruturas e operações correspondentes em seu harness, mas
   não usa esses contratos Rust como autoridade no snapshot analisado.
3. **Persistência específica e independente de produto** para lifecycle,
   loop/cancelamento/reconciliação, policy/aprovação e ledger/checkpoints em
   SQLite. A Engine explicita que esses stores não equivalem a schema de
   negócio; que o PostgreSQL ainda não os tem é uma lacuna de equivalência a
   fechar, não uma decisão.
4. **Adapters reutilizáveis em múltiplas linguagens** para grupos de storage e
   messaging. Isso permite uma base compartilhada para GH Bot e Vectora, mas
   ainda tem diferenças de TTL, ACK, filtros e score. Cada uma é lacuna a fechar
   nos dois lados do par ou especialização com motivo de incompatibilidade, e
   não difere por escolha.
5. **Uma fixture compartilhada de conformidade entre Rust/Python/TypeScript**
   no topo do PR #34 para cenários selecionados. É ganho de disciplina
   contratual; não é paridade global.

Não contabilizo como “vantagem” a Engine possuir tipos com nomes semelhantes
ao Vectora. Enquanto a implementação do produto não consome esses tipos e não
passa pelos mesmos casos, isso é uma rota de migração, não uma substituição
concluída.

## O que o Vectora tem a mais que a Engine hoje

O Vectora tem mais **capacidades executáveis e conectadas ao produto** no
snapshot analisado:

1. **Loop de conversa funcionando como integração**: provider → streaming →
   chamada de tool → resultado → próxima iteração, com estado persistido,
   eventos e limites. `ModelToolLoop` na Engine só coordena contrato/estado.
2. **Mais providers e caminhos multimodais integrados**, mais decisão e
   roteamento de modelos do produto, fallback e configurações operacionais.
   A Engine oferece um conjunto menor de adapters experimentais.
3. **Tool registry e execução real**, batching, tools específicas, validação de
   produto, execução de efeitos e integração com workbenches. A Engine valida
   e prepara dispatch, mas não executa tools.
4. **`ContextProjection`, planejamento/compactação de contexto, memória, RAG,
   retrieval e Context Graph**, incluindo fluxo de indexação e ferramentas
   associadas. A Engine tem primitivas de embedding/vector storage, não esse
   sistema.
5. **Subagents/SOULs, tarefas em background, DAG/queue e scheduler**, com
   políticas e comportamento de produto conectados. A Engine ainda não tem
   orquestração multiagente/workflow.
6. **Backends de sandbox e execução** para diferentes ambientes, além de
   integração com terminal, navegador e computer use. A proteção depende da
   configuração; não se deve confundir suporte de backend com isolamento sempre
   ativo.
7. **Persistência de sessões e experiência de usuário**, chat, Electron,
   frontend, workbenches, Git UI, terminal compartilhado e ciclo de vida do
   aplicativo.
8. **Integrações específicas** com Git/GitHub, browser, filesystem, web, MCP
   cliente e serviços do ecossistema Vectora.
9. **Primitivas de avaliação/replay ligadas ao produto** em `backend/evaluation/`.
   Ainda requer auditoria dos oráculos e da cobertura antes de serem tratadas
   como avaliação abrangente.

Essa lista descreve superfícies presentes no código; não garante que cada uma
esteja habilitada, segura em qualquer configuração ou coberta em todos os
caminhos de produção.

## Fronteiras arquiteturais

### Deve caber na Engine quando for mecanismo genérico

- Provider/model adapter, streaming, retries/fallback com semântica explícita,
  limites e erros tipados.
- `RunLifecycle`, `ModelToolLoop`, ledger, checkpoints, idempotência,
  cancelamento, recuperação e eventos de execução.
- Contratos de `ToolDispatch`, `ToolPolicy`, approvals, verificação e evidência;
  execução isolada só depois de definir ownership, threat model e adapter.
- SQL, vector, KV, queue, pub/sub, memória/contexto/RAG se e quando os contratos
  forem genéricos e comprováveis entre consumidores.
- Extensibilidade para que o host registre tools, providers, policies,
  backends e workflows sem colocar APIs privadas de produto no core.
- Observabilidade de runtime estruturada e redigida, com correlação, duração,
  resultado, retries, cancelamento e backend efetivo.

### Deve permanecer no Vectora

- Workbenches e suas telas, integração humana com os mesmos estados e recursos,
  Electron/React, SSE/UI e affordances de edição.
- Tools e serviços que expressam o produto Vectora: Git Workbench, browser
  compartilhado, filesystem/editor, terminal/PTY, computer use, integrações de
  usuário, Library, Kanban e operações de workspace.
- SOULs, configuração de workspace, conta, login, licença, billing e políticas
  específicas do app. Um mecanismo genérico de extensão pode vir da Engine;
  catálogo e UX não são automaticamente dela.
- Sidecars e ciclo de vida que pertencem à distribuição do Vectora. A Engine
  pode definir interface de executor/adapter, mas não deve assumir Electron,
  branding ou layout de instalação do app.

### Deve permanecer no GH Bot

- Recepção e validação de eventos GitHub, deduplicação de deliveries, escopo
  por instalação/repositório, permissões GitHub e escrita de comentários,
  reviews/status/checks.
- Configuração de conexões GitHub, regras de review e apresentação do resultado
  em dashboard/CLI.
- Escolha de deployment hosted ou self-hosted e o contrato com Services para
  login/entitlements. A Engine não é responsável pelo plano comercial.
- Adaptadores de produto que mapeiam review/evento a `RunLifecycle` e
  `ModelToolLoop`, sem transformar o GH Bot em fork do backend Vectora.

### Services não é o motor de execução por padrão

Pela arquitetura descrita pelo mantenedor, Services participa de identidade e
entitlements, enquanto a execução hosted ou self-hosted do GH Bot se conecta
diretamente aos endpoints configurados do bot. Cada tráfego adicional por
Services precisa de justificativa e contrato. Não presumir proxy obrigatório
para providers, storage ou API GitHub.

## Issue #387: uso correto como referência

A issue [#387](https://github.com/vectora-company/vectora/issues/387) é útil para
entender a tese de produto do Vectora — workspace integrado, workbenches e
operação humano/agente — e para formular perguntas de segurança e adoção da
Engine. Ela não deve ser usada como inventário definitivo do código:

- O sumário do e-book local nomeia o capítulo 11 como “Como se mede uma IA”,
  12 como “Modelos que colam na prova”, 14 como “A fábrica de propaganda”,
  15 como “Um estagiário com acesso root” e 16 como “Código que ninguém leu”.
  A issue #387 atribui outros títulos/conteúdos a vários desses números.
- A issue apresenta números e comparações de workbenches e concorrentes, e
  descreve capacidades em tom de posicionamento. Não use essas comparações
  como fatos sem fonte independente e versão específica do produto.
- O repositório do Vectora confirma muitos módulos citados, mas um módulo,
  nome de workbench ou comentário não demonstra sozinho comportamento
  bidirecional, garantia de segurança, profundidade de recurso ou qualidade.
- Termos como “Evaluation Lab” e “SystemOne” são nomes do produto. Nesta
  análise, as capacidades são descritas pelos mecanismos observáveis:
  execução/replay de avaliação; roteamento/decisão de modelo; políticas,
  evidência e gates; contexto, retrieval e memória.

## Sabatina: decisões que os mantenedores precisam sustentar

As respostas são conclusões desta análise no snapshot indicado, não promessas
de roadmap.

### 1. A Engine já substitui o harness do Vectora?

**Não.** Ela fornece contratos e componentes Rust para partes do runtime, mas
não tem o fluxo conectado de provider, contexto, tools e UI que
`run_conversation` possui no backend. A migração é por capability e requer
conformidade com o comportamento observável.

### 2. `ModelToolLoop` é um agente completo?

**Não.** É uma máquina/contrato durável de coordenação entre modelo, tool,
resultado, cancelamento e reconciliação. O host ainda escolhe provider,
constrói contexto, executa tool e lida com os efeitos reais.

### 3. `ToolPolicy` impede prompt injection?

**Não por si só.** Ela valida o snapshot e a relação entre policy, dispatch e
aprovação. O host precisa autenticar o principal, limitar capabilities,
proteger credenciais e executar adapters isolados conforme o threat model.

### 4. `VerificationGate` garante que a tarefa foi feita corretamente?

**Não.** Ele avalia evidências fornecidas e produz um resultado segundo o
contrato. O host executa os checks; qualidade do oráculo e autenticidade da
evidência continuam sendo requisitos externos.

### 5. `VectorStore` significa que a Engine já tem RAG e memória?

**Não.** Persistir e buscar vetores não define ingestão, chunking, provenance,
permissão, ranking, Context Projection, consolidação ou política de confiança.

### 6. Se há SQLite e PostgreSQL, o ledger é tolerante a qualquer falha?

**Não.** `ResilientLedger` grava SQLite como fonte local e projeta ao
PostgreSQL; isso ainda não torna os stores de lifecycle, loop e policy equivalentes entre
SQLite e PostgreSQL (lacuna a fechar),
nem garante consistência sob qualquer falha sem testes de serviço e
recuperação.

### 7. Podemos dizer que os bindings Python e TypeScript têm paridade?

**Só para os casos que passaram pela fixture comum e APIs comparadas.** PR #34
melhora evidência para um conjunto de contratos; não cobre todas as funções,
erros, cancelamentos, ambientes ou diferenças de runtime.

### 8. O que a Engine tem que o Vectora não tem?

**Uma autoridade Rust compartilhável e contratos comuns já formalizados para
algumas capabilities.** O Vectora não os consome neste snapshot. Isso é
vantagem de arquitetura e potencial de reuse, não vantagem de execução do
produto.

### 9. O que o Vectora tem que a Engine não tem?

**Um harness integrado mais amplo:** contexto e memória, tool execution,
subagents, sandbox adapters, sessões, UI/workbenches, sidecars e integrações
de produto. A Engine não deve receber essas capacidades em bloco; deve receber
os mecanismos genéricos e extensíveis que mais de um consumidor realmente
precisa.

### 10. O GH Bot deve importar ferramentas do Vectora?

**Não como dependência de produto.** Deve registrar suas próprias tools
GitHub-specific sobre contratos genéricos da Engine. Reaproveitar código do
Vectora só é correto quando for mecanismo independente de UI, workspace e
política do app.

### 11. Services deve intermediar todo o GH Bot?

**Não de acordo com a arquitetura definida.** Services atende login/entitlement
e contratos pontuais. A conexão GitHub e a execução podem ser diretas ao bot
self-hosted ou ao serviço hospedado. Evitar Cloudflare no caminho de cada
chamada quando isso não for necessário.

### 12. O que precisa acontecer antes de anunciar paridade ou ganho?

Definir workload e oráculo; fixar SHA, configuração, provider e dados; rodar
mais de uma repetição quando houver variância; medir qualidade, latência,
cancelamento, custo e trabalho humano residual; inspecionar falhas e preservar
rollback. Uma fixture verde ou benchmark público isolado não basta.

### 13. Devemos construir agora a lista completa de capacidades de um harness?

**Não como um único escopo.** A tese de plataforma extensível é válida, mas
implementar de uma vez ferramentas, skills, subagents, workflows, memória,
sandbox e observabilidade produziria contratos sem consumidores comprovados.
Priorizar a capability com consumidor real — GH Bot primeiro, Vectora depois —
e ampliar a API quando outro consumidor confirmar a abstração.

### 14. A issue #387 define o que a Engine deve implementar?

**Não.** Ela contextualiza o produto Vectora. O escopo da Engine vem das specs
da Engine, contratos testáveis e necessidades compartilhadas dos consumidores.
UI, workbenches e integrações particulares permanecem produto mesmo quando
sejam centrais para o Vectora.

## Ordem de trabalho sugerida

1. Fechar a matriz de contratos do PR #34 e corrigir divergências Rust/Python/TS
   encontradas por casos compartilhados.
2. Aumentar testes reais de storage e recuperação por adapter: restart,
   reconnect, ACK/redelivery, TTL/retention, concorrência e shutdown.
3. Portar primeiro a superfície de LLM do Vectora, com fixtures que comparem
   request/response/stream/erro sem fazer duas execuções de efeitos externos.
4. Integrar `ModelToolLoop`, `ToolPolicy` e `VerificationGate` num consumidor
   pequeno e real, preferencialmente o GH Bot, medindo idempotência de eventos,
   cancelamento e revisão útil.
5. Definir APIs de extensão e execução de `Tool` só com um adapter consumidor
   concreto; separar autorização, execução e isolamento.
6. Adicionar Context Projection/memória como etapa própria depois de definir
   provenance, escopo, confiança, budget e oráculo de retrieval. Não inferir
   isso a partir de embeddings ou vector search.
7. Migrar o Vectora por capability com fallback/rollback e comparar resultado
   sob workloads reais. Não fazer dual-write de efeitos externos.

## Referências de código e documentação

- Engine: [`matriz de conformidade`](02-matriz-conformidade-capabilities.md),
  [`runtime agêntico e harness`](../specs/01-runtime-agentico-e-harness.md),
  [`storage, memória, contexto e RAG`](../specs/02-storage-memoria-contexto-rag.md),
  [`Model Tool Loop`](../specs/11-model-tool-loop.md),
  [`Tool Policy e Approval`](../specs/12-policy-and-approval.md) e
  [`Verification Gates`](../specs/15-verification-gates.md).
- Vectora no snapshot `bf1732ac`: `backend/engine/conversation_loop.py`,
  `backend/persistence/native/agent_run.py`, `backend/tools/`, `backend/llm/`,
  `backend/context_graph/`, `backend/evaluation/`, `backend/sandbox/`,
  `backend/scheduling/` e `backend/services/context_compaction.py`.
- GH Bot no snapshot `b489b902`: `README.md`, `docs/architecture.md`,
  `docs/engine-integration.md`, `docs/services-integration.md`,
  `docs/github-integration.md`, `docs/configuration.md` e `docs/operations.md`.
  Verifique a versão do checkout antes de usar esses caminhos como estado atual.
- Livro local: `book/pt/ch02.md`, `ch03.md`, `ch06.md`–`ch12.md`, `ch15.md`,
  `ch16.md`, `ch19.md`, `ch20.md`, `apB.md`, `apC.md`, `apmemoria.md` e
  `aptecnico.md`.

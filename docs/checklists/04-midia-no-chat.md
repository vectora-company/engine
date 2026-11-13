# Checklist — mídia nativa no chat

Checklist da [spec de mídia](../specs/17-midia-no-chat.md) e do
[plano ativo](../plans/03-plano-continuidade.md). O payload (#79), a integração
Rust (#81) e os bindings (#82) estão implementados em PRs abertos; a stack parte
de #73 e ainda não está integrada em `main`. A revisão documental está no PR [#83](https://github.com/vectora-company/engine/pull/83), filha do #82. A etapa de limites está no PR [#84](https://github.com/vectora-company/engine/pull/84), filha do #83. O retry multimodal está no PR aberto [#85](https://github.com/vectora-company/engine/pull/85), filho do #84.

## Contrato e implementação

- [x] Confirmar que Anthropic Messages aceita blocos `document` com fonte
      `base64` para PDFs e que Gemini `generateContent` aceita `inlineData`.
- [x] Emitir PDF inline no formato de documento da Anthropic.
- [x] Emitir mídia inline em `inlineData` no adapter Gemini.
- [x] Emitir PDF como `file.file_data` e áudio WAV/MP3 como `input_audio` no
      adapter OpenAI Chat Completions.
- [x] Manter formatos de áudio que OpenAI Chat Completions não aceita na forma
      textual.
- [x] Testar payloads Anthropic, Gemini e OpenAI e resolução das capabilities
      com assertions Rust; o teste integrado percorre `MediaRef` → decisão
      nativa → payload OpenAI.
- [x] Verificar que áudio não aceito pelo formato OpenAI não envia bytes inline.

## Verificação e entrega

- [x] `cargo fmt --all -- --check`.
- [x] `cargo test -p vectora-engine --lib chat:: --locked` — 44 testes passaram.
- [x] `cargo clippy -p vectora-engine --all-targets --all-features --locked -- -D warnings`.
- [x] Prettier e `git diff --check`; revisar o diff.
- [x] Atualizar plano, spec, análise e Q&As com evidência real e limites.
- [x] Commit e push do código da etapa de payload.
- [x] Abrir o PR #79 sobre a branch do PR #77.
- [x] Iniciar a próxima etapa em `feat/chat-media-routing-evidence`, filha do PR #79.
- [x] Commit, push e abrir o PR #80 empilhado sobre #79, sem marcar brunosrz no GitHub.
- [x] Registrar a decisão de brunosrz no chat: oferecer os três modos de integração.

## Evidência do consumidor Vectora

A stack de mídia da Vectora, PRs [#479–#502](https://github.com/vectora-company/vectora/pulls),
foi integrada em 19 PRs empilhados em 2026-11-13; isso confirma a necessidade
funcional no consumidor, não paridade da Engine. Padrões transferíveis e ainda
pendentes na Engine: roteamento por modelo/catálogo, limites de bytes por
provider, reidratação apenas no turno atual, retry único em rejeição específica
de mídia e evento/evidência de roteamento compartilhados com o consumidor.
STT local (faster-whisper/Speech do macOS), conversão de PDF/vídeo, preferências
UI, permissões e empacotamento SCons são responsabilidades do Vectora nesta
implementação; não foram adicionados à API pública da Engine.

## Limites

- OpenAI Responses API não existe neste adapter; o contrato se limita aos
  formatos do Chat Completions.
- Catálogo automático de capabilities/limites por modelo, limites agregados do request e `ToolResult` multimodal seguem no backlog da spec 17. O retry específico está implementado no PR aberto #85. Os overrides de limite por asset/modalidade são configuráveis no contexto de execução e estão implementados no PR aberto #84; catálogo automático e limites agregados não estão incluídos.

## Integração — etapa atual

- [x] brunosrz escolheu oferecer preparo manual, contexto por chamada e
      contexto padrão por cliente.
- [x] Contexto por chamada prevalece sobre o padrão; mídia já preparada não é
      processada de novo.
- [x] `MediaStore` async e `ChatRequest` sem bytes persistidos.
- [x] Implementar preparo no core Rust para OpenAI-compatible, native e fallback.
- [x] Reidratar só a mensagem de usuário atual e resultados de tools posteriores.
- [x] Limitar cada asset a 20 MiB por padrão, configurável, antes da leitura.
- [x] Incluir evidência sem bytes em `ChatCompletion` e evento `media_routed`.
- [x] Cobrir no teste HTTP o contexto explícito por chamada prevalecendo sobre
      o padrão do cliente.
- [x] Verificar Rust: 49 testes `chat::`, suíte Rust do workspace e docs no hook,
      fmt, Clippy e `git diff --check` passaram.
- [x] Verificar TypeScript: build, typecheck e testes; 37 passaram e 10 testes
      dependentes de serviços externos foram ignorados sem URLs configuradas.
- [x] Verificar Python: build editável e `test_binding.py` — 51 passaram.
- [x] Commit, push e abrir o PR #82, filho do #81: https://github.com/vectora-company/engine/pull/82.
- [x] Expor os três modos nos bindings Python/TypeScript com mapa temporário
      `asset_id → bytes`; documentar a autorização no host.
- [ ] Criar fixtures de conformidade comuns para conteúdo de mídia.
- [x] Aceitar limites explícitos de bytes por provider/modelo e modalidade nos três níveis; prevalece o menor entre o limite específico e o teto global por asset.
- [x] Cobrir limite documental em Rust, Python e TypeScript; o override não vaza para outro modelo.
- [ ] Resolver automaticamente capabilities e limites a partir de catálogo/metadados confiáveis e ligados ao endpoint/protocolo.
- [ ] Validar limites de requisição total, quantidade de partes e duração/resolução, além dos bytes por asset.
- [x] Implementar retry textual único em rejeição HTTP 400 classificada como
      incompatibilidade multimodal, no mesmo modelo; preservar erros de
      autenticação/rede e não repetir erros após abertura do stream. Os 51 testes
      `chat::` incluem classificação, retry complete/stream no cliente direto e
      retry de completion no wrapper fallback. Falta rodar hooks completos.
- [ ] Representar resultados de tool como partes tipadas e preservar imagens
      nos protocolos compatíveis.
- [ ] Definir se a Engine permanece consumidora de um store do host ou se ganha
      um asset store persistente próprio; não ligar armazenamento persistente à
      leitura de mídia antes de fechar ownership e autorização.
- [ ] Não declarar a spec concluída até os critérios aplicáveis estarem
      implementados e verificados nos três níveis.

## Evidência do consumidor Vectora

A stack de mídia da Vectora, PRs [#479–#502](https://github.com/vectora-company/vectora/pulls),
foi integrada em 19 PRs empilhados em 2026-11-13; isso confirma a necessidade
funcional no consumidor, não paridade da Engine. Padrões transferíveis ainda
pendentes: catálogo automático de modalidades/limites por modelo e imagens em resultados de tools. O retry está implementado no PR aberto #85, com cobertura de cliente direto e wrapper fallback; 51 testes `chat::` e hooks completos passaram no SHA `0767636`. A API já aceita overrides explícitos por provider/modelo/modalidade; ainda não consulta catálogo nem deriva limites automaticamente. STT local, conversão de PDF/vídeo,
preferências UI, permissões e empacotamento SCons permanecem capacidades do
Vectora nesta implementação.

## Limites

- OpenAI Responses API não existe neste adapter; o contrato é Chat Completions.
- O mapa efêmero carrega bytes em memória para a duração da chamada; hosts com
  stores externos validam escopo antes de montar `asset_id → bytes`.

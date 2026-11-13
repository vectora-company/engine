# Checklist — mídia nativa no chat

Etapa da [spec de mídia](../specs/17-midia-no-chat.md) e do
[plano ativo](../plans/03-plano-continuidade.md). A etapa de payload foi entregue
no PR #79 sobre #77; esta branch filha registra a decisão para integrar a mídia
no envio.

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
- [ ] Aguardar a resposta no chat; manter bloqueadas as mudanças de API que dependem da decisão.

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
- A integração do preparador no fluxo real de envio, fallback após rejeição,
  limites e roteamento/evidência seguem no backlog da spec 17.

## Próxima etapa — decisão vital

- [x] Concluir e verificar a serialização de payloads específica do PR #79.
- [x] Investigar o caminho de envio e confirmar que os adapters não recebem
      `MediaStore` nem `CapabilityResolver`.
- [x] Registrar as opções de API e a decisão de síncrono/assíncrono em
      [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
- [ ] Aguardar a resposta de brunosrz no chat antes de alterar a API pública.

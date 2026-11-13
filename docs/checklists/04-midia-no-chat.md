# Checklist — mídia nativa no chat

Etapa da [spec de mídia](../specs/17-midia-no-chat.md) e do
[plano ativo](../plans/03-plano-continuidade.md), executada em uma branch
empilhada sobre o PR #77.

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
- [ ] Commit, push e PR empilhado sobre `fix/storage-vector-collection-dimensions` (#77).
- [ ] Iniciar a etapa seguinte em branch empilhada, sem aguardar CI.

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

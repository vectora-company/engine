# Checklist: mídia no chat

Contrato em [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md),
plano geral em [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md)
e estado de providers na
[`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](05-providers-e-modalidades-llama-cpp-v1.md).
Os estados abaixo descrevem o checkout verificado; teste de binding/fake não
substitui serviço real.

## Modelo e segurança

- [x] Tipos de conteúdo para imagem, áudio, vídeo, documento e arquivo.
- [x] `ToolResult` com partes tipadas e helpers Rust/Python/TypeScript.
- [x] Separar `PersistableChatRequest` de `EphemeralChatRequest`; persistência
      rejeita bytes inline e URLs `data:`.
- [x] Três modos nos bindings: preparo manual, contexto por chamada e padrão
      do cliente.
- [x] Resolver bytes somente por mapa efêmero `asset_id → bytes` fornecido
      pelo consumidor; autorização e storage persistente pertencem ao host.
- [x] Validar digest, assinatura/MIME, modalidade e limite por asset antes do
      envio; padrão 20 MiB por asset, configurável.
- [x] Limitar bytes totais/quantidade de assets do turno de chat e rejeitar
      requests inline efêmeros acima do orçamento configurado.
- [x] Reidratar somente o turno atual e resultados de tools subsequentes;
      registrar evidência sem bytes.
- [x] Retry único de texto somente em rejeição multimodal classificada, antes
      de saída parcial e sem repetir falhas de auth/rede.
- [x] Responses de erro e logs não incluem bytes ou credenciais.

## Routing e operações

| Estado   | Capacidade                                    | Notas                                                                                                                                                                          |
| -------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| parcial  | Chat multimodal por protocolo                 | Anthropic, Gemini e OpenAI Chat Completions/Responses têm mapeamentos próprios; Responses cobre imagem e áudio WAV/MP3. Conferir suporte por modelo antes de rotear.           |
| parcial  | Imagens em resultados de tools                | Partes tipadas disponíveis; conversão depende do protocolo e da capability.                                                                                                    |
| parcial  | PDF, áudio e vídeo                            | Tipos e algumas rotas nativas existem; conversão de arquivos e STT local são responsabilidade do consumidor.                                                                   |
| parcial  | Embeddings/rerank multimodais                 | llama-server suporta entradas efêmeras documentadas e protegidas; não generalizar para outros providers.                                                                       |
| parcial  | Áudio em OpenAI Responses                     | WAV/MP3 são serializados como `input_audio`; a capability depende de metadata por modelo ou override nos dois perfis. Falta smoke real.                                        |
| parcial  | Catálogo de capability/limites por modelo     | Metadata de `/v1/models` pode ser convertida explicitamente e vence o perfil genérico; discovery/cache, catálogo por operação e limites completos não existem.                 |
| feito    | Limites agregados                             | Tetos configuráveis de bytes totais e assets materializados cobrem chat, Decision Model efêmero, embeddings e rerank multimodais do llama-server; duração/resolução pendentes. |
| pendente | Geração de imagem e operações gerais de vídeo | Requerem contratos e adapters dedicados.                                                                                                                                       |

## Testes e critérios de conclusão

- [x] Fixtures cobrem serializers e adapters Rust e bindings para modalidades
      implementadas.
- [x] Testes negativos cobrem MIME divergente, bytes inválidos, limite,
      capability ausente e envelope persistível.
- [ ] Fixtures compartilhadas para a mesma matriz de conteúdo nos três
      bindings e em todos os adapters que alegam equivalência.
- [x] Limites agregados de bytes e contagem cobertos em Rust/Python/TypeScript no preparo de chat; requests efêmeros inline são rejeitados antes da rede.
- [x] Aplicar orçamento agregado e validar modalidades descobertas no Decision Model efêmero do llama-server, com override do consumidor; embeddings e rerank também usam metadata do modelo.
- [ ] Cobrir catálogo com dados ausentes, conflitantes e desatualizados; conversão de metadata `/v1/models` é opt-in, sem discovery/cache automático.
- [ ] Smoke tests opt-in com modelos reais para cada operação multimodal; não
      considerar teste ignorado por falta de endpoint como sucesso.
- [ ] Atualizar spec, análise e checklist provider × operação sempre que um
      adapter ampliar ou restringir suporte.
- [ ] Concluir revisão de cancelamento/stream parcial para áudio e mídia em
      todas as bindings.

## Responsabilidades do consumidor

O consumidor decide autorização, escopo e persistência de assets e fornece os
bytes autorizados ao mapa efêmero. UI, preview, PDF/vídeo para imagens, OCR,
transcrição local e seleção de modelo de produto permanecem no consumidor, a
menos que exista capability genérica aprovada separadamente.

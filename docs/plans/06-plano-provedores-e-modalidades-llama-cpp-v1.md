# Plano: providers e modalidades de IA

Este plano organiza capacidades por protocolo e modalidade. O documento registra
contratos, critérios e lacunas funcionais; estados de branch e integração são
transitórios e consultados no GitHub quando necessários. A referência de
compatibilidade do servidor local é a tag
[`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/tree/v1.0.0), sem
vincular a Engine ao runtime C++.

## Objetivo

Oferecer uma superfície Rust canônica para chat, decisão, visão/mídia,
embeddings, rerank, áudio e operações correlatas. Python e TypeScript/Node
encaminham ao mesmo núcleo e preservam a mesma semântica. Perfis nomeados
descrevem protocolos concretos; compatibilidade de API não implica que todo
modelo ofereça todas as modalidades.

## Estado observado no checkout

| Domínio          | Implementado                                                                                                                                                                                                           | Limites e trabalho restante                                                                                                                                                                                                                    |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat e streaming | Transporte OpenAI-compatible; Anthropic Messages; Gemini; Cohere; Ollama `/api/chat`; OpenAI Chat Completions e Responses; OpenRouter; fallback antes de emitir saída; perfil `llama-server` e cliente standalone Laya | Ainda não há catálogo confiável e completo por modelo/operação; endpoints remotos reais dependem de credenciais e modelos configurados. 9router, OmniRouter e Kimi podem usar o transporte genérico quando compatíveis, sem garantia por nome. |
| Decision Models  | Cliente Laya standalone e `/v1/systemone` do `llama-server`; perguntas choice/score/noul e entrada efêmera multimodal onde anunciada                                                                                   | Smoke real com servidor/checkpoint Laya não foi executado neste ambiente; ampliar matriz de providers somente após contratos de protocolo verificáveis.                                                                                        |
| Áudio            | TTS bufferizado e streaming; sessão realtime para transcrição/conversa no `llama-server`                                                                                                                               | Não há batch STT genérico. Não inferir suporte a transcrição a partir de TTS ou realtime.                                                                                                                                                      |
| Embeddings       | OpenAI-compatible, OpenRouter, Cohere, Voyage, Ollama e `llama-server`; texto e entradas multimodais via asset map no llama-server                                                                                     | Catálogo de modalidades por modelo e integração de servidor/modelo real precisam de cobertura opt-in.                                                                                                                                          |
| Rerank           | Cohere, Voyage, OpenRouter e `llama-server`; texto e imagens efêmeras no `llama-server`                                                                                                                                | Completar conformidade por provider, formato de score e erro; não declarar paridade por rótulo de provider.                                                                                                                                    |
| Imagem e vídeo   | Tipos de mídia no chat; roteamento conforme protocolos cobertos; embeddings e rerank podem consumir imagens no llama-server                                                                                            | Geração de imagem e operações de vídeo dedicadas não estão implementadas como APIs de provider. OpenAI Responses ainda requer revisão de entrada de mídia/saída multimodal.                                                                    |
| Bindings         | Python e TypeScript/Node expõem as operações implementadas pelo core Rust                                                                                                                                              | Conformidade total e matriz de instalação/publicação continuam no plano da EPIC.                                                                                                                                                               |

Evidências por teste e observações do protocolo estão em
[`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md)
e no [checklist desta capability](../checklists/05-providers-e-modalidades-llama-cpp-v1.md).

## Organização do trabalho

### 1. Matriz provider × operação

Manter uma linha por operação concreta (chat, stream, decisão, visão,
embeddings, rerank, TTS, STT, imagem e vídeo) e colunas para protocolo,
autenticação, request/response, streaming, limites, bindings e evidência.
Adicionar perfil dedicado quando defaults, conversão ou semântica forem
materialmente diferentes. Usar transporte genérico apenas onde fixtures
confirmarem compatibilidade. Não prometer suporte a partir do nome comercial.

### 2. Contrato local llama.cpp v1.0.0

Validar cada endpoint contra a tag fixada: `/health`, `/v1/models`, chat,
`/v1/systemone`, embeddings, `/rerank`, `/tts`, `/v1/audio/speech` e
`/v1/realtime`. Preservar extensões úteis sem aceitar respostas ambíguas; usar
fixtures deterministas e smoke tests opt-in com versão/modelo registrados.
Documentar diferenças entre extensão upstream e superfície comum da Engine.

### 3. Requests efêmeros e mídia

Manter bytes fora de requests persistíveis. Assets são resolvidos pelo mapa
efêmero do consumidor com digest, MIME e limites verificados. Rejeitar URLs,
paths e bytes inline nas APIs persistíveis. Aplicar limites por
provider/modelo/modalidade e aplicar os limites agregados configuráveis de
bytes/assets ao chat. Estender o mesmo contrato às operações restantes após
definir suas semânticas de entrada.

### 4. Conformidade e instalação

Cada operação precisa de fixtures Rust e bindings; serviços reais são
complementares e opt-in. Cobrir erros, resposta parcial, cancelamento,
redaction, limites e fallback. Atualizar exemplos e documentação dos
consumidores sem mover UI, catálogo de produto ou credenciais para a Engine.

## Critérios de aceite

- Matriz explícita de providers/protocolos por operação, com estados
  implementado, parcial, não suportado ou não verificado.
- Contratos de request/response e erros testados no Rust e nos dois bindings.
- Fluxos de mídia persistível/efêmera, asset map, integridade e limites
  cobertos contra payloads malformados e capacidades ausentes.
- Fallback interrompido após primeira saída observável; retries nunca duplicam
  efeitos nem repetem após saída parcial.
- Referência llama.cpp v1.0.0 conferida por endpoint e comportamento, com
  smoke test real apenas quando servidor/modelo estão disponíveis.
- Limitações de provider, operação, plataforma e distribuição visíveis nos
  documentos de capability e nos gates da EPIC.

## Fora do escopo da Engine

Contas, credenciais de produto, UI, catálogo de modelos mantido pelo produto,
transcrição local do sistema operacional, conversão de PDF/vídeo e ferramentas
de workbench pertencem aos consumidores, salvo contratos genéricos aprovados.

## Dependências e próximo trabalho

A ordem segue dependências dos contratos e gaps verificados. Limites
agregados opcionais já cobrem chat, Decision Model, embeddings e rerank. A
metadata de `/v1/models` agora pode ser convertida explicitamente em
`gateway_metadata` no Rust, Python e TypeScript, com precedência sobre o perfil
genérico e abaixo de overrides do consumidor. O binding não consulta nem
atualiza metadata por conta própria: discovery/cache, catálogo confiável por
provider e operação, além de limites de duração/resolução, continuam pendentes.
Em paralelo, o plano geral da EPIC avança nas demais capabilities.

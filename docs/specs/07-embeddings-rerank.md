# Embeddings e rerank

**Estado: implementada como capability experimental na Engine; sem adoção automática pelos produtos.**

Esta especificação adapta os contratos que o Vectora usa em
`backend/llm/base.py`, nos adapters nativos de embeddings e rerank e nos
wrappers de fallback. A engine oferece APIs de inferência reutilizáveis; cada
produto continua dono de seu RAG, seleção de modelo, credenciais e regras de
indexação.

## Fronteira e consumidores

- O consumidor fornece modelo, endpoint, credenciais, headers, timeout,
  documentos e opções por chamada. A engine não lê settings, `.env`, catálogos
  ou banco de dados e não descobre credenciais.
- Crate Rust, PyPI/Python e npm/TypeScript chamam a mesma implementação Rust.
  Os bindings são parte do bootstrap e recebem a nova capacidade sem manter
  transporte ou parsing duplicado.
- A engine calcula vetores e scores; não grava no LanceDB/Qdrant, não altera
  índices, não conhece coleções e não decide se deve reindexar.
- O Vectora verifica a compatibilidade da dimensão com o índice persistido. A
  engine valida consistência interna da resposta e a dimensão solicitada, mas
  não pode inferir a dimensão já armazenada pelo consumidor.

## Embeddings

O contrato recebe `model`, textos não vazios, tipo lógico `document` ou
`query`, e dimensão de saída opcional. Retorna vetores float na ordem dos
textos, dimensão resultante e usage/custo quando o provider os fornece. Lote
vazio retorna sem chamada HTTP. Quantidade divergente, vetor vazio, dimensão
inconsistente, índice duplicado/ausente ou vetor inválido é erro tipado; nunca
se associa resposta por ordem de chegada quando o protocolo fornece índices.

Adapters iniciais:

- **Cohere Embed v2:** `POST /v2/embed`, tipos `search_document` e
  `search_query`, `embedding_types: ["float"]`, truncamento `END`, lotes de no
  máximo 96. A resposta não traz índice por item; a API garante a ordem do
  lote, que deve ser preservada.
- **Voyage:** `POST /embeddings`, `input_type: document|query`, dimensão de
  saída opcional, dtype float e truncation habilitado. Reordenar `data[]` pelo
  índice explícito antes de devolver vetores.
- **OpenRouter:** `POST /embeddings`, `input_type: document|query`, dimensão
  opcional e usage/custo; reordenar `data[]` pelo índice.
- **Ollama:** `POST /api/embed`, `model` e `input`; o protocolo não recebe tipo
  query/documento nem dimensão de saída. O adapter mantém ordem de entrada e
  não deve alegar suporte às opções que o protocolo não oferece.

## Rerank

O contrato recebe modelo, consulta, documentos e `top_n` opcional; retorna
resultados na ordem de relevância do provider, com índice no array original e
score bruto. Os resultados podem ser um subconjunto até `top_n`. Índices
duplicados, ausentes do intervalo ou formato inválido falham explicitamente;
consumidores mapeiam os índices de volta a seus próprios documentos e
metadados.

- **Cohere:** `POST /v2/rerank`, `top_n` e `max_tokens_per_doc` configurável;
  resposta em `results[]` com `index` e `relevance_score`.
- **Voyage:** `POST /rerank`, `top_k` e truncamento; resposta em `data[]`,
  preservando a ordem de ranking e o índice original. O contrato expõe a opção
  `truncation` do protocolo Voyage.
- **OpenRouter:** `POST /rerank`, `top_n`; resposta `results[]` indexada.
- Lista de documentos vazia retorna vazia sem chamada externa. Resposta vazia
  não é transformada silenciosamente na ordem original: o consumidor decide se
  deve degradar para o ranking anterior.

## Fallback e falhas

Fallback é opt-in e percorre somente a lista explícita de candidatos que o
consumidor montou e autorizou. Não existe seleção automática de outro provider.
Para manter o comportamento atual do Vectora, embeddings e rerank só tentam o
próximo candidato em quota/rate limit; falhas de autenticação, configuração,
dimensão, parsing, conexão ou protocolo propagam. A ordem de resultados e os
identificadores `from`/`to` das trocas ficam no resultado para o consumidor
registrar/logar ou notificar conforme sua política.

Chamadas em lote podem já ter sido cobradas no provider primário antes de uma
quota numa página posterior; fallback reexecuta a operação inteira no próximo
candidato e não persiste nenhum resultado parcial. Os consumidores devem
considerar esse custo e nunca gravar vetores parciais.

## Conformidade e limites

Fixtures locais devem cobrir os formatos e erros de cada protocolo, lote e
ordem, índice embaralhado, quantidade/dimensão divergente, top-k, score,
resposta vazia, fallback por quota, erros que não ativam fallback e ausência
de rede para entrada vazia. A API foi exposta pelas bindings Python e
TypeScript que fazem parte do bootstrap; as fixtures de bindings exercitam os
protocolos sobre o mesmo núcleo Rust sem credenciais reais. A cobertura ainda
é experimental e não equivale a conformidade completa de todos os erros e
limites descritos acima.

Este trabalho não migra `storage/factory.py`, `tools/rag.py`, jobs de
indexação, persistência de dimensões ou integração com SystemOne. Também não
afirma paridade completa com todos os modelos ou gateways desses providers.

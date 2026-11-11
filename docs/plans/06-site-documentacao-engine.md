# Site de documentação da Vectora Engine

## Objetivo

Manter um site público para documentar a Engine e suas interfaces Rust, Python
e TypeScript. Hugo gera o site estático a partir de Markdown e Hextra fornece
navegação, busca e suporte multilíngue. O código-fonte fica em `website/`.

## Escopo entregue nesta etapa

- Preservar Hugo + Hextra, adicionar marca própria da Engine derivada do
  beija-flor Vectora e gerar assets de favicon/compartilhamento.
- Configurar identidade, metadados SEO, navegação, tema e idiomas inglês e
  português.
- Documentar primeiros passos, arquitetura e fronteiras host/runtime, maturidade,
  chat, embeddings/reranking, harness, runs duráveis, policy/aprovações,
  verificação, storage, erros/segurança e APIs Rust, Python e TypeScript/Node.
- Manter documentação pública bilíngue em português e inglês, com links para
  contratos versionados e indicação explícita de capacidades incompletas.
- Remover páginas, layouts, configurações de deploy e artefatos gerados que
  pertencem ao site do aplicativo Vectora.
- Não publicar automaticamente planos, auditorias, sabatinas ou specs internas
  armazenadas em `docs/`.
- Usar uma URL-base de preview no build; confirmar e substituir pela URL final
  quando a hospedagem for escolhida. Não configurar pipeline de publicação.

## Pendências antes de publicação pública

1. Confirmar domínio, versionamento da documentação e ambiente de preview.
2. Rever URLs canônicas, favicon e imagem social para o domínio escolhido.
3. Criar workflow de build e publicação após definir hosting, versões e política
   de releases.
4. Adicionar validação automatizada de links e exemplos ao CI.
5. Atualizar cada guia quando contratos ou bindings mudarem; não anunciar como
   disponível o que a [matriz de capabilities](../../website/content/docs/reference/capabilities.pt.md)
   classifica como parcial, planejado ou indisponível.

## Critérios de conclusão desta etapa

- Hugo reconhece o site e Hextra sem conteúdo de produto alheio à Engine.
- Inglês e português aparecem como idiomas disponíveis.
- Metadados SEO, título, favicon e imagem social descrevem a Vectora Engine,
  sem links de trial, billing ou instalação do aplicativo.
- `website/public/` e demais saídas de build não são versionadas.
- O conteúdo técnico é mantido em português e inglês e está ligado pela
  navegação do site.
- O build local não publica o site; a URL-base configurada serve como referência
  para gerar links canônicos e metadados sociais e deve ser confirmada antes do
  deploy.

# Plano do site de documentação da Vectora Engine

## Objetivo

Manter um site público para documentar a Engine e suas interfaces Rust, Python
e TypeScript. Hugo gera o site estático a partir de Markdown e Hextra fornece
navegação, busca e suporte multilíngue. O código-fonte fica em `website/`.

## Limites deste bootstrap

- Preservar a configuração Hugo + Hextra e os assets de marca reutilizáveis.
- Configurar identidade, metadados SEO, navegação, tema e idiomas inglês e
  português.
- Deixar apenas os índices iniciais vazios nos dois idiomas; conteúdo técnico
  será escrito em etapas posteriores.
- Remover páginas, layouts, configurações de deploy e artefatos gerados que
  pertencem ao site do aplicativo Vectora.
- Não publicar automaticamente planos, auditorias, sabatinas ou specs internas
  armazenadas em `docs/`.
- Usar uma URL-base de preview no build; confirmar e substituir pela URL final
  quando a hospedagem for escolhida. Não configurar pipeline de publicação.

## Etapas posteriores

1. Confirmar domínio, versionamento da documentação e ambiente de preview.
2. Escrever guias Rust, Python e TypeScript com exemplos derivados da API
   realmente implementada e publicada.
3. Documentar capabilities, maturidade, contratos e limites entre Engine e
   aplicações hospedeiras; distinguir implementação de proposta.
4. Criar workflow de build e publicação após definir hosting, versões e
   política de releases.
5. Adicionar validação de links, build e exemplos documentados ao CI.

## Critérios de aceite do bootstrap

- Hugo reconhece o site e Hextra sem conteúdo de produto alheio à Engine.
- Inglês e português aparecem como idiomas disponíveis.
- Metadados SEO e identidade descrevem a Vectora Engine, sem links de trial,
  billing ou instalação do aplicativo.
- `website/public/` e demais saídas de build não são versionadas.
- O conteúdo Markdown em `website/content/` consiste apenas nos dois índices
  vazios iniciais.
- O build local não publica o site; a URL-base configurada serve como referência
  para gerar links canônicos e metadados sociais e deve ser confirmada antes do
  deploy.

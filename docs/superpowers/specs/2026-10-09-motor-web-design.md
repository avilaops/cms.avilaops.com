# motor-web: desenho

Data: 09/10/2026. Aprovado em conversa por Nicolas; aguarda revisão deste arquivo.

## Contexto

A Ávila Ops vai ter um CMS próprio ao lado do `lojas.avilaops.com`. O
diferencial é o site já nascer correto em SEO técnico, desempenho, dados
estruturados, imagens, `llms.txt` e sinais de confiança, com travas que impedem
quem edita de estragar isso.

O Lojas já resolve boa parte disso para e-commerce, dentro do próprio código.
A decisão foi tirar essas regras de dentro de um produto e colocá-las em um
motor compartilhado, usado pelos dois.

### Decisões já tomadas

- Motor compartilhado, em repositório próprio, consumido por commit fixo.
- CMS como aplicação única que serve N sites por host, no desenho do Lojas.
- Editam o conteúdo: o cliente, a equipe da Ávila Ops e agentes de IA pelo
  conector. Por isso nenhuma regra mora na tela.
- A primeira versão do CMS é provada com um site novo de demonstração.

### Ciclos

| # | Entrega | Pronto quando |
|---|---|---|
| 1 | `avilaops/motor-web` | Pacote testado sozinho, com o site de demonstração como fixture |
| 2 | `avilaops/cms.avilaops.com` | Site de demonstração no ar, editável por painel e conector, medido no Lighthouse |
| 3 | Adoção pelo Lojas | Lojas gera metadados, JSON-LD, sitemaps e `llms.txt` pelo motor, sem regressão |

Este documento cobre o ciclo 1. Os ciclos 2 e 3 ganham spec própria.

## Objetivo do ciclo 1

Um pacote TypeScript que recebe conteúdo e devolve, de forma determinística:
a lista de problemas que impedem a publicação, o cabeçalho da página, o JSON-LD,
as variantes de imagem, os sitemaps, o `robots.txt` e o `llms.txt`.

### Fora do escopo

- Banco de dados, painel, autenticação, conector MCP e rotas HTTP (ciclo 2).
- Carrinho, checkout, estoque e preço: continuam no Lojas.
- Alteração de qualquer arquivo do Lojas (ciclo 3).
- Armazenamento das imagens: o motor devolve os bytes das variantes, quem chama
  decide onde gravar.

## Princípios

- Núcleo sem banco, sem rede e sem Next. Quem chama entrega os dados.
- Funções puras: mesma entrada, mesma saída. Data e hora entram por parâmetro.
- Validação devolve problemas, não lança exceção.
- Português nos nomes e comentários, TypeScript estrito, dinheiro em centavos
  inteiros, como no Lojas.
- O código não conhece cliente: nada de `if (site === "x")`.

## Módulos

Cada módulo é uma entrada do pacote (`motor-web/validacao`, `motor-web/seo`...).

### `tipos`

Contrato do conteúdo. É o payload que a API do CMS entrega e que o Lojas vai
mapear a partir do catálogo dele.

```ts
type Site = {
  origem: string;                 // https://exemplo.com.br, sem barra final
  nome: string;
  descricao: string;              // contexto da marca; abre o llms.txt
  idioma: string;                 // pt-BR
  logo: Midia;
  organizacao: {
    razaoSocial?: string;
    telefone?: string;
    email?: string;
    endereco?: Endereco;
    perfis: string[];             // sameAs
  };
  diretrizesIa: string[];         // regras do negócio para assistentes
  orcamentoPesoKb: number;        // por página
};

type Midia = {
  id: string;
  alt: string;
  largura: number;
  altura: number;
  variantes: { formato: "avif" | "webp"; largura: number; url: string }[];
  legenda?: string;
  credito?: string;
};

type Seo = {
  titulo: string;
  descricao: string;
  canonica: string;               // calculada pelo motor, nunca digitada
  indexar: boolean;
  imagemSocial?: Midia;
};

type Trilha = { nome: string; url: string }[];

type Autor = {
  slug: string;
  nome: string;
  cargo: string;
  bio: string;
  foto: Midia;
  perfis: string[];
  credenciais?: string[];
};

type Pagina = {
  slug: string;                   // "" é a home
  titulo: string;
  corpo: Bloco[];
  capa?: Midia;
  publicadoEm: string;            // ISO 8601
  atualizadoEm: string;
  seo: Seo;
  trilha: Trilha;
};

type Post = Omit<Pagina, "capa"> & {
  tipo: "artigo" | "guia-tecnico";
  resumo: string;
  capa: Midia;
  autor: Autor;
  categoria: { slug: string; nome: string };
  tags: string[];
};

type Product = {
  slug: string;
  nome: string;
  descricao: string;
  marca?: string;
  imagens: Midia[];               // a primeira é a principal
  variacoes: {
    sku: string;
    nome: string;
    precoCentavos: number;
    disponibilidade: "em-estoque" | "sem-estoque" | "sob-encomenda";
    gtin?: string;
  }[];
  moeda: "BRL";
  avaliacoes?: { media: number; total: number };  // só do próprio site
  politicaDevolucaoUrl: string;
  atualizadoEm: string;
  seo: Seo;
  trilha: Trilha;
};
```

### `conteudo`

O corpo é uma lista de blocos tipados. Não existe HTML livre.

| Bloco | Campos |
|---|---|
| `paragrafo` | texto com marcações de negrito, itálico e link |
| `titulo` | nível 2, 3 ou 4; texto |
| `imagem` | `Midia` |
| `lista` | ordenada ou não; itens |
| `citacao` | texto; fonte opcional |
| `tabela` | cabeçalho; linhas |
| `perguntas` | pares de pergunta e resposta |
| `video` | URL do YouTube ou do Vimeo; título |
| `chamada` | texto; rótulo; URL |

O `<h1>` não é bloco: sai do título da página. Fica impossível ter dois ou
nenhum.

Entregas: `renderizarCorpo(blocos)` como componente React de servidor, com HTML
semântico, e `textoDoCorpo(blocos)` em Markdown, usado pelo `llms-full.txt` e
pela contagem de palavras.

### `validacao`

```ts
type Problema = {
  codigo: string;                 // estável, ex.: "seo.titulo.longo"
  campo: string;                  // caminho, ex.: "corpo[3].alt"
  gravidade: "bloqueia" | "avisa";
  mensagem: string;               // português simples, diz o que fazer
};

validarPagina(pagina, contexto): Problema[]
validarPost(post, contexto): Problema[]
validarProduto(produto, contexto): Problema[]
validarUpload(arquivo): Problema[]
podePublicar(problemas): boolean   // nenhum "bloqueia"
```

`contexto` traz o que a regra precisa do resto do site: os títulos e
descrições já usados e o slug publicado anterior, se houver. O motor não
consulta nada.

Rascunho salva sempre. A trava é na publicação.

| Regra | Efeito |
|---|---|
| Título ou descrição vazio | Bloqueia |
| Título ou descrição igual ao de outra página do site | Bloqueia |
| Título acima de 70 caracteres; descrição acima de 170 | Bloqueia |
| Título acima de 60; descrição acima de 155 | Avisa |
| Título de seção pulando nível (2 direto para 4) | Bloqueia |
| Imagem sem `alt`, ou `alt` igual ao nome do arquivo | Bloqueia |
| Post sem autor; autor sem bio ou sem foto | Bloqueia |
| Capa de post com menos de 1200 px de largura | Bloqueia |
| Upload acima de 15 MB | Recusa |
| Upload fora de JPG, PNG, WebP e AVIF, ou arquivo ilegível | Recusa |
| Slug com acento, espaço ou maiúscula | Corrige sozinho (`normalizarSlug`) |
| Slug alterado em página publicada, sem redirecionamento | Bloqueia |
| Link que não é `http(s)`; vídeo fora de YouTube e Vimeo | Bloqueia |
| Produto sem variação, com preço zero ou negativo | Bloqueia |
| `indexar: false` em página publicada | Avisa |
| Página acima de `orcamentoPesoKb` | Avisa |

Os limites de 60 e 155 são recomendação: o buscador corta por largura, não por
contagem. Por isso avisam, e o bloqueio fica em 70 e 170.

`deveAtualizarData(antes, depois)` diz se "atualizado em" muda: só quando o
corpo, o título ou a capa mudam. Salvar sem mexer no conteúdo não altera a
data.

### `seo`

```ts
montarCabecalho(doc, site): Cabecalho
// { titulo, metas, links, jsonLd }

paraMetadataNext(cabecalho): Metadata     // entrada motor-web/next
<HeadSEO cabecalho={...} />               // entrada motor-web/react
```

- Canônica: `site.origem` + caminho, sem query e sem barra final (exceto a
  home). Paginação aponta para a própria página.
- `robots`: `noindex` quando `indexar` é falso.
- Open Graph e Twitter Card saem do título, da descrição e da
  `imagemSocial`, com a capa como reserva. Sem imagem, o card sai em formato
  resumido, sem campo vazio.
- `<HeadSEO/>` devolve `<title>`, `<meta>`, `<link>` e os `<script
  type="application/ld+json">`. O React 19 sobe os três primeiros para o
  `<head>`. No App Router o caminho preferido é `paraMetadataNext`, com o
  JSON-LD renderizado pelo componente `<DadosEstruturados/>`.
- O JSON-LD é serializado com `<` escapado, para o texto do editor não fechar
  o `<script>`.

### `schema`

| Página | Tipos |
|---|---|
| Home | `Organization`, `WebSite` |
| Página | `WebPage`, `BreadcrumbList` |
| Post `artigo` | `Article`, `Person`, `BreadcrumbList` |
| Post `guia-tecnico` | `TechArticle`, `Person`, `BreadcrumbList` |
| Corpo com bloco `perguntas` | acrescenta `FAQPage` |
| Produto | `Product` com `Offer` ou `AggregateOffer`, `BreadcrumbList` |

- `datePublished` e `dateModified` vêm de `publicadoEm` e `atualizadoEm`.
- `aggregateRating` só existe quando `avaliacoes` está preenchido com
  avaliações do próprio site e `total` é maior que zero. Nota de outra origem
  não entra.
- `Offer` leva preço em reais com duas casas, `availability`, `sku` e a URL da
  política de devolução (`hasMerchantReturnPolicy`).
- Campo opcional ausente some da saída.

### `midia`

```ts
processarImagem(bytes, nomeOriginal): Promise<ImagemProcessada | Problema[]>
// { nome, largura, altura, variantes: { formato, largura, bytes }[] }

<MediaRenderer midia={...} papel="principal" | "conteudo" sizes="..." />
```

- Formatos AVIF e WebP, nas larguras 480, 768, 1200 e 1920, sem ampliar além
  do original.
- Nome do arquivo: `normalizarSlug` do nome original mais um trecho do hash do
  conteúdo, para permitir cache longo.
- Metadados EXIF removidos; orientação aplicada antes.
- `<MediaRenderer/>` gera `<picture>` com `<source>` AVIF, `<source>` WebP e
  `<img>` de reserva, sempre com `width`, `height`, `alt` e `decoding="async"`.
- `papel="principal"` aplica `fetchpriority="high"` e `loading="eager"`;
  `papel="conteudo"` aplica `loading="lazy"`. Só a capa do post e a primeira
  imagem do produto são principais.
- `sharp` é dependência par: o Lojas já tem a dele.

### `descoberta`

```ts
gerarSitemaps(site, itens): { caminho: string; xml: string }[]
gerarRobots(site, opcoes): string
gerarLlms(site, itens): string
gerarLlmsCompleto(site, itens): string
```

- `sitemap.xml` é um índice que aponta para `sitemap-paginas.xml`,
  `sitemap-posts.xml`, `sitemap-categorias.xml`, `sitemap-produtos.xml` e
  `sitemap-imagens.xml`. Arquivo sem item não é gerado. Acima de 50 mil URLs o
  arquivo é dividido.
- Só entra o que está publicado, com `indexar` verdadeiro. `lastmod` é
  `atualizadoEm`.
- `robots.txt` libera o site, bloqueia painel e API, e aponta o sitemap. A
  lista de robôs de IA liberados ou bloqueados vem em `opcoes`; o padrão é
  liberar.
- `llms.txt` em Markdown: nome e descrição da marca, as páginas principais com
  uma linha cada, os posts recentes, os produtos em destaque e as
  `diretrizesIa` do site. As diretrizes padrão pedem para não inventar preço
  nem disponibilidade e para citar com link para a página de origem.
- `llms-full.txt` traz o texto dos posts e páginas por `textoDoCorpo`.

O `llms.txt` é convenção proposta, não padrão que os modelos comprovadamente
leiam. Entra porque custa pouco; não é garantia de citação.

## Erros

- Validação e upload devolvem `Problema[]`. Mensagem escrita para leigo:
  "Escreva uma descrição da foto para quem não enxerga a imagem".
- Imagem corrompida vira `Problema` com gravidade `bloqueia`, não exceção.
- Geradores aceitam só documento que passou em `podePublicar`. Recebendo um
  documento inválido, lançam erro de programação com o `codigo` do primeiro
  problema: é falha de quem chamou, não de quem editou.

## Desempenho

O motor não garante nota do Lighthouse: um script de terceiros colado pelo
cliente derruba qualquer página. O que ele entrega é o que depende dele:
imagem no formato e no tamanho certos, dimensões declaradas, prioridade só na
imagem principal, nenhum JavaScript de cliente nos componentes e o aviso de
orçamento de peso. A medição pelo Lighthouse, com build falhando abaixo do
alvo, é do ciclo 2, sobre o site de demonstração.

## Testes

- Unitários por regra de validação: um caso que passa e um que barra, com o
  `codigo` conferido.
- Fixture `demonstracao`: empresa fictícia com home, três páginas, cinco posts
  (um `guia-tecnico`, um com bloco `perguntas`), dois autores e dois produtos.
  Nenhum nome de cliente real.
- A fixture atravessa o motor inteiro, e a saída é comparada com arquivos de
  referência versionados: JSON-LD, sitemaps, `robots.txt`, `llms.txt`.
- JSON-LD conferido contra a lista de campos obrigatórios do Google para cada
  tipo.
- Componentes renderizados no servidor e conferidos pelo HTML: um `<h1>`,
  `width` e `height` em toda imagem, `fetchpriority="high"` em uma só.
- `processarImagem` testado com imagens pequenas geradas no próprio teste,
  inclusive um arquivo corrompido.
- Executor: `tsx --test`, como no Lojas.

## Empacotamento

- Repositório público `avilaops/motor-web`. O código não tem dado de cliente.
- ESM com tipos; entradas por módulo, mais `motor-web/react` e
  `motor-web/next`. `react`, `next` e `sharp` como dependências pares; `next`
  e `sharp` opcionais.
- O `dist` é gerado no `prepare`, para a instalação por
  `github:avilaops/motor-web#<commit>` funcionar sem publicar pacote.
- Workflow de validação (typecheck, lint, testes) em push e PR, e
  `merge-automatico.yml` no padrão da casa. Sem deploy: é biblioteca.
- `AGENTS.md` com as regras do projeto e `README.md` com um exemplo de uso por
  módulo.

## Critério de pronto

1. `npm run typecheck`, `npm run lint` e `npm test` passam no Actions.
2. A fixture gera todos os artefatos e eles batem com a referência.
3. Um projeto Next.js vazio instala o pacote pelo commit e renderiza um post
   da fixture com `paraMetadataNext`, `<DadosEstruturados/>` e
   `<MediaRenderer/>`.

## O que fica para os próximos ciclos

- Ciclo 2: modelo de dados no Postgres, painel, login pelo Auth, conector MCP,
  armazenamento de imagens, redirecionamentos, cache por página, IndexNow e
  medição pelo Lighthouse.
- Ciclo 3: mapear o catálogo do Lojas para `Product` e trocar a geração
  própria de metadados, JSON-LD, sitemaps e `llms.txt` pelo motor, um artefato
  por vez, comparando a saída antes e depois.

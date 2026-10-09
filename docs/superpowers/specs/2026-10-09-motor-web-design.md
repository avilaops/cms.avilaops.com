# motor-web: desenho

Data: 09/10/2026. Substitui a versão em TypeScript do mesmo dia, depois da
decisão de Nicolas de escrever a plataforma em Rust. Escopo e travas são os já
aprovados; muda a linguagem e saem os componentes React.

Convenções comuns: `2026-10-09-base-rust-design.md`.

## Contexto

A Ávila Ops vai ter um CMS próprio ao lado do Lojas. O diferencial é o site já
nascer correto em SEO técnico, desempenho, dados estruturados, imagens,
`llms.txt` e sinais de confiança, com travas que impedem quem edita de estragar
isso.

O Lojas atual resolve boa parte disso dentro do próprio código. O `motor-web`
tira essas regras de dentro de um produto e as coloca em um crate usado pelos
dois.

### Ciclos

| # | Entrega | Pronto quando |
|---|---|---|
| 1 | `avilaops/motor-web` | Crate testado sozinho, com o site de demonstração como fixture |
| 2 | `avilaops/cms.avilaops.com` | Site de demonstração no ar, editável por painel e conector, medido no Lighthouse |
| 3 | Lojas em Rust | Fatia por fatia, conforme `2026-10-09-lojas-rust-design.md` |

Este documento cobre o ciclo 1.

## Objetivo

Um crate que recebe conteúdo e devolve, de forma determinística: a lista de
problemas que impedem a publicação, o cabeçalho da página, o JSON-LD, o HTML do
corpo e das imagens, as variantes de imagem, os sitemaps, o `robots.txt` e o
`llms.txt`.

### Fora do escopo

- Banco, HTTP, painel, autenticação e conector MCP.
- Carrinho, checkout, estoque e preço.
- Onde as imagens são gravadas: o crate devolve os bytes das variantes.
- Layout das páginas: o crate entrega os pedaços, o template de cada produto
  compõe.

## Princípios

- Sem banco, sem rede, sem relógio e sem estado global. Data e hora entram por
  parâmetro.
- Função pura: mesma entrada, mesma saída, byte a byte. Onde a ordem importa, a
  saída é ordenada; nenhum `HashMap` é iterado para gerar texto.
- Validação devolve problemas. Não há `panic!`, `unwrap` nem `expect` fora de
  teste.
- O código não conhece cliente.
- Português nos nomes e comentários. Dinheiro em centavos, `i64`.

## Módulos

### `tipos`

Contrato do conteúdo. É o que o CMS grava e entrega, e o que o Lojas mapeia a
partir do catálogo dele. Todos derivam `Serialize`, `Deserialize`, `Clone`,
`Debug` e `PartialEq`, com `#[serde(rename_all = "camelCase")]` e
`#[serde(deny_unknown_fields)]`.

```rust
pub struct Site {
    pub origem: String,             // https://exemplo.com.br, sem barra final
    pub nome: String,
    pub descricao: String,          // contexto da marca; abre o llms.txt
    pub idioma: String,             // pt-BR
    pub logo: Midia,
    pub organizacao: Organizacao,
    pub diretrizes_ia: Vec<String>,
    pub orcamento_peso_kb: u32,     // por página
}

pub struct Organizacao {
    pub razao_social: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub endereco: Option<Endereco>,
    pub perfis: Vec<String>,        // sameAs
}

pub struct Endereco {
    pub logradouro: String,
    pub cidade: String,
    pub uf: String,
    pub cep: String,
}

pub struct Midia {
    pub id: String,
    pub alt: String,
    pub largura: u32,
    pub altura: u32,
    pub variantes: Vec<Variante>,
    pub legenda: Option<String>,
    pub credito: Option<String>,
}

pub struct Variante {
    pub formato: Formato,           // Avif | Webp
    pub largura: u32,
    pub url: String,
}

pub struct Seo {
    pub titulo: String,
    pub descricao: String,
    pub indexar: bool,
    pub imagem_social: Option<Midia>,
}

pub struct ItemTrilha { pub nome: String, pub url: String }

pub struct Autor {
    pub slug: String,
    pub nome: String,
    pub cargo: String,
    pub bio: String,
    pub foto: Midia,
    pub perfis: Vec<String>,
    pub credenciais: Vec<String>,
}

pub struct Pagina {
    pub slug: String,               // "" é a home
    pub titulo: String,
    pub corpo: Vec<Bloco>,
    pub capa: Option<Midia>,
    pub publicado_em: DateTime<Utc>,
    pub atualizado_em: DateTime<Utc>,
    pub seo: Seo,
    pub trilha: Vec<ItemTrilha>,
}

pub struct Post {
    pub tipo: TipoPost,             // Artigo | GuiaTecnico
    pub slug: String,
    pub titulo: String,
    pub resumo: String,
    pub capa: Midia,
    pub corpo: Vec<Bloco>,
    pub autor: Autor,
    pub categoria: Categoria,
    pub tags: Vec<String>,
    pub publicado_em: DateTime<Utc>,
    pub atualizado_em: DateTime<Utc>,
    pub seo: Seo,
    pub trilha: Vec<ItemTrilha>,
}

pub struct Categoria { pub slug: String, pub nome: String }

pub struct Product {
    pub slug: String,
    pub nome: String,
    pub descricao: String,
    pub marca: Option<String>,
    pub imagens: Vec<Midia>,        // a primeira é a principal
    pub variacoes: Vec<Variacao>,
    pub avaliacoes: Option<Avaliacoes>,   // só do próprio site
    pub politica_devolucao_url: String,
    pub atualizado_em: DateTime<Utc>,
    pub seo: Seo,
    pub trilha: Vec<ItemTrilha>,
}

pub struct Variacao {
    pub sku: String,
    pub nome: String,
    pub preco_centavos: i64,
    pub disponibilidade: Disponibilidade, // EmEstoque | SemEstoque | SobEncomenda
    pub gtin: Option<String>,
}

pub struct Avaliacoes { pub media: f32, pub total: u32 }

pub struct Documento {
    pub caminho: String,            // /blog/como-escolher, definido por quem chama
    pub conteudo: Conteudo,
}

pub enum Conteudo { Pagina(Pagina), Post(Post), Produto(Product) }
```

A canônica não é campo: é calculada de `site.origem` e `documento.caminho`. O
caminho é de quem chama, porque CMS e Lojas têm rotas diferentes.

A moeda é o real. O campo entra quando existir a segunda.

### `conteudo`

O corpo é uma lista de blocos tipados. Não existe HTML livre.

```rust
#[serde(tag = "tipo", rename_all = "kebab-case")]
pub enum Bloco {
    Paragrafo { trechos: Vec<Trecho> },
    Titulo { nivel: u8, texto: String },          // 2, 3 ou 4
    Imagem { midia: Midia },
    Lista { ordenada: bool, itens: Vec<String> },
    Citacao { texto: String, fonte: Option<String> },
    Tabela { cabecalho: Vec<String>, linhas: Vec<Vec<String>> },
    Perguntas { itens: Vec<Pergunta> },
    Video { url: String, titulo: String },
    Chamada { texto: String, rotulo: String, url: String },
}

pub struct Trecho {
    pub texto: String,
    pub negrito: bool,
    pub italico: bool,
    pub link: Option<String>,
}

pub struct Pergunta { pub pergunta: String, pub resposta: String }
```

O `<h1>` não é bloco: sai do título do documento. Fica impossível ter dois ou
nenhum.

```rust
pub fn renderizar_corpo(blocos: &[Bloco]) -> String   // HTML semântico
pub fn texto_do_corpo(blocos: &[Bloco]) -> String     // Markdown
pub fn contar_palavras(blocos: &[Bloco]) -> usize
```

- `renderizar_corpo` escapa todo texto e todo atributo. Imagem sai por
  `renderizar_midia` com papel de conteúdo.
- Vídeo sai como `<iframe loading="lazy">` do endereço de incorporação
  (`youtube-nocookie.com` ou `player.vimeo.com`), com `title` e dimensões.
- `texto_do_corpo` alimenta o `llms-full.txt`.

### `validacao`

```rust
pub struct Problema {
    pub codigo: &'static str,       // estável, ex.: "seo.titulo.longo"
    pub campo: String,              // caminho, ex.: "corpo[3].midia.alt"
    pub gravidade: Gravidade,       // Bloqueia | Avisa
    pub mensagem: String,           // português simples, diz o que fazer
}

pub struct Contexto<'a> {
    pub titulos_em_uso: &'a [String],      // de outros documentos do site
    pub descricoes_em_uso: &'a [String],
    pub slug_publicado: Option<&'a str>,   // slug anterior, se já publicado
    pub redirecionados: &'a [String],      // slugs antigos que já têm 301
    pub peso_kb: Option<u32>,              // medido por quem chama
    pub orcamento_peso_kb: u32,
}

pub fn validar(documento: &Documento, contexto: &Contexto) -> Vec<Problema>
pub fn validar_upload(nome: &str, bytes: &[u8]) -> Vec<Problema>
pub fn pode_publicar(problemas: &[Problema]) -> bool      // nenhum Bloqueia
pub fn normalizar_slug(texto: &str) -> String
pub fn deve_atualizar_data(antes: &Documento, depois: &Documento) -> bool
```

O retorno é `Vec<Problema>`, e não `Result<(), Vec<Problema>>`: com `Result`, o
`Ok` não carrega os avisos, e quem edita deixaria de ver "título acima de 60"
justamente quando a página pode ser publicada.

Rascunho salva sempre. A trava é na publicação.

| Regra | Código | Efeito |
|---|---|---|
| Título ou descrição vazio | `seo.titulo.vazio`, `seo.descricao.vazia` | Bloqueia |
| Título ou descrição igual ao de outro documento do site | `seo.titulo.repetido`, `seo.descricao.repetida` | Bloqueia |
| Título acima de 70 caracteres; descrição acima de 170 | `seo.titulo.excedido`, `seo.descricao.excedida` | Bloqueia |
| Título acima de 60; descrição acima de 155 | `seo.titulo.longo`, `seo.descricao.longa` | Avisa |
| Slug vazio depois de normalizado (exceto a home) | `slug.vazio` | Bloqueia |
| Slug alterado em documento publicado, sem redirecionamento | `slug.sem-redirecionamento` | Bloqueia |
| Título de seção pulando nível, ou primeiro título diferente de 2 | `corpo.titulo.nivel` | Bloqueia |
| Imagem sem `alt` | `midia.alt.vazio` | Bloqueia |
| `alt` igual ao nome do arquivo | `midia.alt.arquivo` | Bloqueia |
| Autor sem bio ou sem foto | `post.autor.incompleto` | Bloqueia |
| Capa de post com menos de 1200 px de largura | `post.capa.pequena` | Bloqueia |
| Link que não é `http(s)` nem caminho do próprio site | `link.invalido` | Bloqueia |
| Vídeo fora de YouTube e Vimeo | `video.origem` | Bloqueia |
| Produto sem variação | `produto.sem-variacao` | Bloqueia |
| Variação com preço zero ou negativo | `produto.preco` | Bloqueia |
| `indexar` falso em documento publicado | `seo.noindex` | Avisa |
| Página acima do orçamento de peso | `pagina.peso` | Avisa |
| Upload acima de 15 MB | `upload.grande` | Recusa |
| Upload fora de JPG, PNG, WebP e AVIF | `upload.formato` | Recusa |
| Arquivo ilegível | `upload.ilegivel` | Recusa |

Detalhes que a tabela não diz:

- Comprimento conta caracteres como a pessoa vê (grafemas, texto em NFC), não
  bytes. "Ação" tem quatro.
- Comparação de repetido ignora caixa e espaço nas pontas.
- 60 e 155 são recomendação: o buscador corta por largura. Por isso avisam, e o
  bloqueio fica em 70 e 170.
- O formato do upload é lido dos primeiros bytes do arquivo, não da extensão.
- Link interno é caminho que começa com uma barra só (`/contato`). `//` e
  qualquer esquema fora de `http` e `https` são recusados.
- O nome do arquivo é deduzido da URL da variante, descontados hash e largura.
- `normalizar_slug` tira acento, baixa a caixa e troca o que não for letra ou
  número por hífen, sem hífen repetido nem nas pontas.
- `deve_atualizar_data` é verdadeiro só quando muda o corpo, o título ou a
  capa. Salvar sem mexer no conteúdo não altera "atualizado em".

Mensagem é para leigo: "Escreva uma descrição da foto para quem não enxerga a
imagem".

### `seo`

```rust
pub struct Cabecalho {
    pub titulo: String,
    pub metas: Vec<Meta>,           // name ou property, e content
    pub links: Vec<Link>,           // rel e href
    pub json_ld: Vec<serde_json::Value>,
}

pub fn canonica(site: &Site, caminho: &str) -> String
pub fn montar_cabecalho(documento: &Documento, site: &Site)
    -> Result<Cabecalho, DocumentoInvalido>

impl Cabecalho {
    pub fn para_html(&self) -> String   // <title>, <meta>, <link>, <script ld+json>
}
```

Substitui o componente `<HeadSEO/>`. O template do produto põe
`cabecalho.para_html()` dentro do `<head>`, ou percorre os campos se quiser
controlar a ordem.

- Canônica: origem mais caminho, sem query e sem barra final, exceto a home.
- `robots`: `noindex, follow` quando `indexar` é falso.
- Open Graph e Twitter Card saem do título, da descrição e da
  `imagem_social`, com a capa como reserva. Sem imagem, o card sai em formato
  resumido, sem campo vazio.
- Post leva `article:published_time` e `article:modified_time`.
- O JSON-LD é serializado com `<`, `>` e `&` escapados em `\u`, para o texto de
  quem edita não fechar o `<script>`.

### `schema`

```rust
pub fn montar_json_ld(documento: &Documento, site: &Site)
    -> Result<Vec<serde_json::Value>, DocumentoInvalido>
```

| Documento | Tipos |
|---|---|
| Home | `Organization`, `WebSite` |
| Página | `WebPage`, `BreadcrumbList` |
| Post `Artigo` | `Article` com `Person` em `author`, `BreadcrumbList` |
| Post `GuiaTecnico` | `TechArticle` com `Person` em `author`, `BreadcrumbList` |
| Corpo com bloco `Perguntas` | acrescenta `FAQPage` |
| Produto | `Product` com `Offer` ou `AggregateOffer`, `BreadcrumbList` |

- `datePublished` e `dateModified` vêm das datas do documento.
- `aggregateRating` só existe com `avaliacoes` preenchido e `total` maior que
  zero. São avaliações do próprio site; nota de outra origem não entra.
- Uma variação gera `Offer`. Várias geram `AggregateOffer`, com menor e maior
  preço, quantidade e as ofertas dentro.
- `Offer` leva preço em reais com duas casas, `availability`, `sku`, `gtin`
  quando há, e a política de devolução em `hasMerchantReturnPolicy`.
- Campo opcional ausente some da saída. As chaves saem em ordem fixa.

### `midia`

```rust
pub enum Papel { Principal, Conteudo }

pub fn renderizar_midia(midia: &Midia, papel: Papel, sizes: &str) -> String
pub fn url_principal(midia: &Midia) -> Option<&str>
```

Substitui o componente `<MediaRenderer/>`.

- `<picture>` com `<source>` AVIF, `<source>` WebP e `<img>` de reserva, sempre
  com `width`, `height`, `alt` e `decoding="async"`.
- `Papel::Principal` aplica `fetchpriority="high"` e `loading="eager"`.
  `Papel::Conteudo` aplica `loading="lazy"`. Só a capa do post e a primeira
  imagem do produto são principais.
- Com legenda, sai dentro de `<figure>` com `<figcaption>`.
- `url_principal` escolhe a variante WebP mais próxima de 1200 px, para Open
  Graph e JSON-LD.

### `imagem` (feature `imagem`)

Fica atrás de uma *feature* do crate, desligada por padrão. Quem só valida e
gera texto não compila codificador de imagem.

```rust
pub struct ImagemProcessada {
    pub nome: String,               // slug do nome original + 8 caracteres do hash
    pub largura: u32,
    pub altura: u32,
    pub variantes: Vec<VarianteGerada>,   // formato, largura, bytes
}

pub fn processar_imagem(nome_original: &str, bytes: &[u8])
    -> Result<ImagemProcessada, Vec<Problema>>
```

- AVIF e WebP com perdas, nas larguras 480, 768, 1200 e 1920, sem ampliar além
  do original. Imagem menor que 1920 ganha também a variante na largura dela.
- A orientação do EXIF é aplicada antes; os metadados não são copiados.
- O hash é SHA-256 do arquivo original, o que permite cache longo.
- A função é síncrona e pesada. Quem chama decide a fila e a thread, conforme a
  base Rust.
- Crates: `image`, `webp` e `ravif`.

### `descoberta`

```rust
pub struct ItensDoSite<'a> {
    pub documentos: &'a [Documento],
    pub categorias: &'a [PaginaDeCategoria],   // caminho, nome, atualizado_em
    pub destaques: &'a [String],               // caminhos de produtos
}

pub struct Arquivo { pub caminho: String, pub conteudo: String }

pub fn gerar_sitemaps(site: &Site, itens: &ItensDoSite) -> Vec<Arquivo>
pub fn gerar_robots(site: &Site, opcoes: &OpcoesRobots) -> String
pub fn gerar_llms(site: &Site, itens: &ItensDoSite) -> String
pub fn gerar_llms_completo(site: &Site, itens: &ItensDoSite) -> String
```

- `sitemap.xml` é um índice que aponta para `sitemap-paginas.xml`,
  `sitemap-posts.xml`, `sitemap-categorias.xml`, `sitemap-produtos.xml` e
  `sitemap-imagens.xml`. Arquivo sem item não é gerado. Acima de 50 mil URLs o
  arquivo é dividido.
- Só entra documento com `indexar` verdadeiro. `lastmod` é `atualizado_em`.
- Texto e URL são escapados para XML.
- `robots.txt` libera o site, bloqueia os caminhos de `opcoes` (o padrão é
  painel e API) e aponta o sitemap. Os robôs de IA bloqueados vêm em `opcoes`;
  o padrão é não bloquear nenhum.
- `llms.txt` em Markdown: nome e descrição da marca, as páginas com uma linha
  cada, os dez posts mais recentes, os produtos em destaque e as diretrizes.
  Seção sem item não aparece.
- As diretrizes padrão pedem para não inventar preço nem disponibilidade e para
  citar com link para a página de origem. As de `site.diretrizes_ia` vêm
  depois.
- O `llms.txt` não traz preço: preço muda, e o arquivo manda consultar a
  página.
- `llms-full.txt` traz o texto de páginas e posts por `texto_do_corpo`.

O `llms.txt` é convenção proposta, não padrão que os modelos comprovadamente
leiam. Entra porque custa pouco; não é garantia de citação.

## Erros

- Validação e upload devolvem `Vec<Problema>`.
- Os geradores de página (`montar_cabecalho`, `montar_json_ld`) devolvem
  `Err(DocumentoInvalido)` com o código do primeiro problema que bloqueia,
  quando recebem documento que não passaria na validação sem contexto. É falha
  de quem chamou, não de quem editou.
- Os geradores de site (`gerar_sitemaps`, `gerar_llms`) pulam documento
  inválido em vez de derrubar o arquivo inteiro. Quem chama sabe quais são
  rodando `validar`, que é a mesma régua.
- Nenhuma função entra em pânico com entrada malformada. Isso é testado com
  entradas geradas (`proptest`).

## Desempenho

O crate não garante nota do Lighthouse: um script de terceiros colado pelo
cliente derruba qualquer página. Ele entrega o que depende dele: imagem no
formato e no tamanho certos, dimensões declaradas, prioridade só na imagem
principal, nenhum JavaScript nos pedaços que gera e o aviso de orçamento de
peso. A medição, com build falhando abaixo do alvo, é do ciclo 2.

## Testes

- Unitários por regra de validação: um caso que passa e um que barra, com o
  código conferido.
- Fixture `demonstracao`: empresa fictícia com home, três páginas, cinco posts
  (um guia técnico, um com bloco de perguntas), dois autores e dois produtos
  (um com uma variação, um com três). Nenhum nome de cliente real.
- A fixture atravessa o crate inteiro, e a saída é comparada com arquivos de
  referência versionados (`insta`): cabeçalho, JSON-LD, HTML do corpo,
  sitemaps, `robots.txt`, `llms.txt`.
- JSON-LD conferido contra a lista de campos obrigatórios do Google para cada
  tipo.
- HTML conferido: um `<h1>` por página montada no teste, `width` e `height` em
  toda imagem, `fetchpriority="high"` em uma só.
- Entradas que quem edita produz e que quebram geradores ingênuos:
  `</script>` no título, `&` e `<` em URL e nome, emoji e acento no limite de
  60 caracteres, slug que normaliza para vazio, site só com a home.
- `processar_imagem` testado com imagens geradas no próprio teste: maior que
  1920, menor que 480, com orientação de EXIF e um arquivo corrompido.
- A fixture é gravada em JSON e relida, para o contrato `serde` não mudar sem
  alguém ver.

## Empacotamento

- Repositório público `avilaops/motor-web`. O código não tem dado de cliente.
- Consumo por commit fixo:
  `motor-web = { git = "https://github.com/avilaops/motor-web", rev = "<commit>" }`.
- *Features*: `imagem`, desligada por padrão.
- Versão mínima do Rust declarada em `rust-version`.
- Workflow: `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`,
  `cargo test --all-features`, e `merge-automatico.yml` no padrão da casa. Sem
  deploy: é biblioteca.
- `AGENTS.md` com as regras do projeto e `README.md` com um exemplo por módulo.

## Critério de pronto

1. `fmt`, `clippy` e `test` passam, com e sem a *feature* `imagem`.
2. A fixture gera todos os artefatos e eles batem com a referência.
3. Um binário mínimo em Axum com Askama consome o crate pelo commit e serve um
   post da fixture, com cabeçalho, JSON-LD, corpo e imagem.

## O que mudou em relação à versão TypeScript

| Antes | Agora |
|---|---|
| Pacote ESM com entradas por módulo | Crate com módulos e uma *feature* |
| `<HeadSEO/>` e `paraMetadataNext` | `montar_cabecalho` e `Cabecalho::para_html` |
| `<MediaRenderer/>` | `renderizar_midia` |
| Corpo como componente React | `renderizar_corpo`, HTML em texto |
| `sharp` | `image`, `webp` e `ravif` |
| Canônica como campo de `Seo` | Calculada de origem e caminho |
| `Problema[]` | `Vec<Problema>`, mesma semântica |
| Gerador lança erro de programação | `Result` com `DocumentoInvalido` |

As travas, os limites, os tipos de JSON-LD, as larguras de imagem e a
estrutura dos sitemaps e do `llms.txt` são os mesmos.

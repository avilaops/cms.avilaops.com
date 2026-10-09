# Lojas em Rust: desenho

Data: 09/10/2026. Reescrita do `lojas.avilaops.com` em Rust, por fatias de
rota, no mesmo domínio e no mesmo banco.

Convenções comuns: `2026-10-09-base-rust-design.md`. Regras de conteúdo:
`2026-10-09-motor-web-design.md`.

Este documento fica no repositório do CMS enquanto é desenho. Aprovado, vai
para `docs/` do repositório do Lojas, que é onde o código mora.

## Decisões de Nicolas (09/10/2026)

- O Lojas passa a ser escrito em Rust: Axum, SQLx, Askama.
- Migração por fatias de rota: o Caddy manda para o Rust um grupo de caminhos
  por vez; o Next encolhe até ser desligado.
- SQLx é dono do esquema desde o dia 0. O Next não migra mais.
- Painel também em Rust, por template.
- Sem Cloudflare e sem Twilio.
- n8n continua nas automações de negócio.

## O que existe hoje

Levantado no repositório em 09/10/2026.

| Item | Quantidade |
|---|---|
| Rotas de API | 144 |
| Páginas | 45 (14 de vitrine, 11 do site da plataforma, 18 de painel, 2 outras) |
| Componentes React | 130, dos quais 84 rodam no navegador |
| Módulos em `src/lib` | 168, com 94 arquivos de teste |
| Modelos Prisma | 45, em 64 migrações |
| Rotinas agendadas | 14 |
| Ferramentas do conector MCP | 26 |
| Layouts de vitrine | 13 |
| Suítes de integração | 10 |
| Linhas de TypeScript | cerca de 59 mil |

O tamanho da migração de dados é pequeno. O da reescrita de código é este.

**A confirmar com Nicolas:** quantas lojas estão em produção. O
`docs/LANCAMENTO.md` registra três no banco; outros documentos do repositório
citam mais lojas no ar. A ordem das fatias não depende do número, mas o cuidado
no corte da vitrine e do checkout depende.

## Objetivo

O Lojas em Rust faz tudo o que o Next faz hoje, com o mesmo banco, as mesmas
URLs e os mesmos contratos de API, e o Next é desligado. Durante a transição,
cada caminho é atendido por um servidor só.

### Fora do escopo

- Redesenho do esquema do banco. As tabelas ficam como estão.
- Funcionalidade nova. Fatia que entra faz o que a rota antiga fazia.
- Mudança visual nos 13 layouts.
- Amazon, Shopee e Magalu além do que já existe (conexão por OAuth).

## Regras que viram requisito

Vêm do `AGENTS.md` do Lojas e valem igual em Rust:

- Nada por loja em código. O ramo da loja é dado (`Tenant.segmento`).
- Preço nunca vem do navegador. Toda rota que cobra monta o pedido pelo
  catálogo.
- Um caminho só para cobrar: validar, montar pelo catálogo, reservar, cobrar,
  registrar. Recusa com resposta do gateway solta a reserva; só tempo-limite é
  incerto, e quem resolve é a rotina de reconciliação.
- Token de gateway só cifrado. Chave e token de API só como SHA-256.
- Regra da lei não mora no template: controle especial é recusado no carrinho e
  na reserva, não só escondido na tela. Arrependimento tem piso de 7 dias no
  leitor.
- Texto do lojista nunca é marcação.
- Medição própria, sem IP nem user-agent, sem perfil sem consentimento.
- Evento que vira mensagem leva a chave do fato, e o mesmo fato dá o mesmo
  identificador.
- Suspender loja é decisão de gente.
- Histórico do conector guarda ferramenta e identificador, nunca argumentos nem
  resultado.

## Convivência entre Next e Rust

### Roteamento

O Caddy do host decide por caminho. Cada loja e o domínio-base ganham, no
mesmo bloco, uma lista de caminhos que vão para o container Rust; o resto
segue para o Next.

- A lista é um arquivo só, incluído nos blocos das lojas, versionado no
  repositório do Lojas e aplicado pelo mesmo mecanismo que hoje gera
  `/etc/caddy/lojas.d/`.
- Fatia entra: os caminhos dela entram na lista. Fatia volta: saem. Voltar não
  exige deploy, só recarregar o Caddy.
- O Caddy não faz reserva por erro: se o Rust responde 500, o visitante vê 500.
  A segurança é a volta rápida, não um plano B automático.

O Caddyfile de produção hoje não é versionado em lugar nenhum. Versionar o
trecho do Lojas é pré-requisito da primeira fatia.

### Sessão

Os dois servidores leem os mesmos cookies, com o mesmo formato e o mesmo
segredo, enquanto conviverem:

- `lojas_sessao`: painel. `base64url(JSON)` mais HMAC-SHA256 com `LOJAS_SECRET`.
- `loja_conta`: comprador, preso ao slug.
- `lojas_mcp_pedido`: pedido de autorização do conector.

O Rust implementa leitura e escrita compatíveis byte a byte, com teste que
valida um cookie gerado pelo Next.

### Banco

- **Migração-base.** O esquema de produção é exportado (`pg_dump --schema-only`)
  e vira a primeira migração do SQLx, marcada como já aplicada nos bancos
  existentes. Banco novo, de teste ou de desenvolvimento, nasce dela.
- **Conferência.** Antes de valer, a migração-base é aplicada em banco vazio e
  o esquema resultante é comparado com o de produção. Diferença zero, incluindo
  gatilhos, funções, `CHECK`s, índices parciais e a extensão `pg_trgm`.
- **Next sem migrar.** O `prisma migrate deploy` sai do deploy do Next. Quando
  o SQLx muda o esquema, o `schema.prisma` é atualizado por `prisma db pull`,
  só para o cliente do Next continuar lendo. A pasta `prisma/migrations` é
  congelada.
- **Ordem do deploy.** Migração do SQLx só acrescenta. O Next antigo continua
  funcionando com coluna a mais.
- **Gancho no `avila-deploy`.** Descrito na base Rust. Sem ele não há dump
  antes da migração, e ele é pré-requisito da primeira fatia que migra.

O que mora no banco e é preservado como está:

- `Produto.busca`, mantida pelo gatilho `produto_busca` e pela função
  `produto_texto_de_busca()`, com índice GIN `gin_trgm_ops`.
- Gatilhos `catalogo_proteger_projecao` e `catalogo_exigir_variante`. A escrita
  legítima se anuncia com `set_config('lojas.catalogo_escrita','on',true)`
  dentro da transação; o Rust faz o mesmo.
- `CHECK`s de saldo, reserva, mídia e preço; índices únicos parciais de
  variante.
- 27 colunas JSON (11 em `Tenant`) e 13 colunas de lista. Em Rust cada coluna
  JSON tem um tipo com `serde`, que aceita o que já está gravado; leitura que
  falha vira alerta, não erro para o visitante.

### Rotinas e eventos

A trava de cada rotina já é a linha da tabela `Rotina`. Uma rotina é executada
por um servidor só: ao passar para o Rust, ela sai do catálogo do Next no
mesmo deploy. Não há período com os dois executando a mesma rotina.

## Fatias

Da menor para a maior. Cada fatia entra em produção sozinha.

### 0. Fundação

Não atende tráfego. Entrega o que as outras precisam:

- *Workspace* Cargo no repositório do Lojas, em `servidor/`, com imagem
  própria (`image-suffix` no build).
- Migração-base do SQLx conferida contra produção.
- Resolução da loja pelo host, igual a `src/lib/tenant.ts`: subdomínio do
  domínio-base por `slug`, qualquer outro host por `dominios`.
- Leitura dos três cookies, compatível com o Next.
- Tipos `serde` das colunas JSON de `Tenant`.
- Cofre AES-256-GCM compatível com `src/lib/cofre.ts`: decifra o que o Next
  cifrou.
- `GET /api/saude`.
- Gancho de migração no `avila-deploy` e trecho do Caddy versionado.
- Container no ar na rede `edge`, sem nenhum caminho desviado.

Pronto quando: o container responde saúde em produção e os testes de
compatibilidade (cookie, cofre, colunas JSON de todas as lojas) passam contra
uma cópia do banco.

### 1. Borda e descoberta

Caminhos: `/robots.txt`, `/sitemap.xml` e filhos, `/llms.txt`,
`/llms-full.txt`, `/feed/merchant.xml`, `/indexnow-key.txt`,
`/plataforma/sitemap-lojas.xml`, `/api/health`, `/api/dominio-permitido`,
`/uploads/*`, `/v1/health`, `/v1/ready`.

- Primeiro uso real do `motor-web`: o catálogo é mapeado para `Product`, e
  sitemaps, `robots.txt` e `llms.txt` saem do crate.
- O feed do Merchant segue `docs/MERCHANT-CENTER.md`: loja que não vende
  publica feed vazio, e o que vai é o que a página mostra.
- Paridade: para cada loja, a saída do Rust é comparada com a do Next.
  Diferença só onde o `motor-web` muda o formato de propósito, e cada uma é
  listada e aprovada antes do corte.
- Risco: sitemap ou `robots.txt` errado tira loja da busca. Depois do corte, o
  Search Console de cada loja é conferido.

### 2. API de desenvolvedores

Caminhos: `/api/v1/*` (15 rotas).

- Uma porta só, como hoje: autenticação por chave, escopo, limite, CORS e erro
  em uma camada `tower`.
- O limitador sai da memória e vai para o banco.
- A saída é projeção explícita por recurso, nunca a linha inteira.
- Paridade: as três suítes de integração existentes rodam contra o Rust sem
  alteração de expectativa. `docs/API.md` é o contrato.
- `vitrine/checkout` cobra, e depende do núcleo de cobrança. Essa rota fica no
  Next até a fatia 6.

### 3. Agendador, eventos e webhooks de saída

Sem caminho público novo; assume `/api/admin/rotinas*` e
`/api/admin/automacoes/eventos/*`.

- As 14 rotinas passam para o Rust uma a uma, começando pelas que não tocam
  dinheiro: `webhooks.entregar`, `automacoes.eventos`, `seo.categorias`,
  `relatorios.semanal`, `estoque.avisos`, `carrinhos.verificar`.
- `cobranca.verificar`, `reservas.reconciliar`, `mercadopago.renovar`,
  `pedidos.verificar` e `pix.lembrete` passam junto com a fatia 6. As duas do
  Mercado Livre, com a fatia 8.
- E-mail com `lettre`, WhatsApp pela Cloud API da Meta. O texto de cada
  mensagem é função pura, como hoje.
- **A decidir por Nicolas:** com o n8n assumindo o que não precisa ser código
  (09/10/2026), esta fatia poderia deixar e-mail e WhatsApp no n8n em vez de
  portá-los. Isso desfaz uma decisão anterior do Lojas, registrada no
  `AGENTS.md` dele ("o relógio é nosso, e o e-mail também"), tomada porque
  executar metade dos canais fazia aviso ao lojista sumir sem ninguém notar. A
  spec mantém o envio no servidor até essa decisão ser revista de propósito.
- Webhook de saída: só `https`, nome resolvido e recusado se for rede interna,
  redirecionamento não seguido.
- O contrato com o n8n não muda: mesmo corpo, mesmas rotas de reivindicar e
  encerrar.
- Paridade: a suíte `webhooks-api` e, para as rotinas, testes escritos nesta
  fatia, porque hoje não há integração para elas.

### 4. Conector MCP e OAuth

Caminhos: `/api/mcp`, `/oauth/*`, `/.well-known/*`, `/autorizar`.

- Servidor de autorização próprio, PKCE S256 obrigatório, registro dinâmico,
  código e tokens só como SHA-256.
- As 26 ferramentas são reescritas sobre as funções de domínio. As que só leem
  entram nesta fatia; as que alteram catálogo, pedido e cupom entram quando a
  fatia dona do domínio entrar. Até lá, `/api/mcp` continua no Next: o
  protocolo é um endpoint só e não se divide por ferramenta.
- Consequência: esta fatia é desenvolvida aqui e cortada depois da fatia 7.
- Paridade: a suíte `mcp-oauth`.

### 5. Vitrine de leitura

Caminhos: `/`, `/produtos`, `/produtos/*`, `/categoria/*`, `/promocoes`,
`/sobre`, `/contato`, `/politicas/*`, `/blog`, `/blog/*`, `/nao-encontrado`, e
as rotas de apoio `/api/busca`, `/api/sugestoes`, `/api/avaliacoes`,
`/api/cep`, `/api/frete`, `/api/cupom`, `/api/avise-me`, `/api/minha-moto`,
`/api/vitrine/sessao`.

- Os 13 layouts viram templates Askama. A regra do Lojas continua: template
  compõe, não personaliza; o contrato do template (`src/lib/templates.ts`) vira
  um tipo, e a página pergunta ao contrato.
- Cabeçalho e JSON-LD de produto, categoria e post saem do `motor-web`.
- JavaScript que continua no navegador, como arquivo estático: carrinho em
  `localStorage`, busca, galeria, carrossel, consentimento e pixels. A regra de
  um caminho só para o Google (GTM ou `gtag`, nunca os dois) é preservada.
- Cache de página em memória, por host e caminho, só sem cookie de sessão,
  invalidado quando o catálogo da loja muda.
- Corte por loja dentro da fatia: a lista de caminhos entra primeiro em uma
  loja, e nas outras depois de uma semana sem regressão.
- Paridade: os scripts Playwright existentes (`scripts/validar-templates.mts`)
  comparam cada layout nas larguras de celular e de computador, e o Lighthouse
  de cada layout não pode piorar.
- Risco: é a fatia que o cliente do lojista vê.

### 6. Checkout, pedido, estoque e Mercado Pago

Caminhos: `/carrinho`, `/checkout`, `/pedido/*`, `/conta`, `/api/checkout/*`,
`/api/conta/*`, `/api/v1/vitrine/checkout`, `/api/webhooks/mercadopago`,
`/api/webhooks/mercadopago-assinatura`, `/mercado-pago/callback`.

- O núcleo do `packages/checkout` (totais, pedido mínimo, validação de
  documentos) é lógica pura e é portado com os testes dele.
- Reserva de estoque com `SELECT … FOR UPDATE`, como hoje.
- Mercado Pago por HTTP direto: Pix, cartão, boleto, estorno, OAuth do
  lojista, mensalidade. Validação da assinatura do webhook por HMAC.
- A tela de cartão usa o SDK do Mercado Pago no navegador. Deixa de ser
  componente React e passa a ser um script estático que monta o campo de
  cartão; o servidor só recebe o token do cartão.
- Entram junto as rotinas de dinheiro da fatia 3.
- Paridade: as suítes `checkout`, `idempotencia`, `catalogo` e
  `api-v1-checkout`, mais compra real em cada meio de pagamento, com estorno,
  em loja de teste, antes do corte.
- Corte por loja, e por último nas lojas que mais vendem.
- Risco: dinheiro e concorrência. É a fatia com mais teste exigido.

### 7. Painel do lojista e plano de controle

Caminhos: `/painel*`, `/api/painel/*` (57 rotas), `/api/admin/*` (29 rotas,
menos as de rotinas e eventos, que são da fatia 3, e as do Mercado Livre, da
fatia 8), `/v1/manifest`, `/v1/stats`, `/v1/ops/status`, e do site da
plataforma: `/criar`, `/confirmar`, `/entrar`, `/recuperar`, `/redefinir`,
`/ajuda`, `/developers`, `/blog*`, `/`.

- As 18 páginas viram templates com formulários e htmx. O levantamento mostra
  que não há editor rico, biblioteca de gráfico nem reordenação por arrasto: a
  maior parte é formulário.
- Três portas de entrada preservadas: Auth, Google e senha (`?senha=1`),
  enquanto o cadastro de loja não criar a conta no Auth.
- Papéis DONO, GERENTE e OPERADOR, com as cinco permissões atuais, conferidos
  em uma camada só.
- Senha em scrypt, compatível com os hashes já gravados.
- Prévia do tema só lê: não grava, não mede, não emite evento.
- Importação e exportação de planilha: `calamine` para ler `.xlsx` e
  `rust_xlsxwriter` para escrever.
- Texto com IA (descrição de produto, essência da loja, SEO de categoria): a
  chamada ao Gemini passa a ser HTTP direto, com a saída validada por tipo.
- Corte por seção do painel, não de uma vez: pedidos, depois produtos, depois
  configurações.
- Paridade: não existe teste de integração do painel hoje. Esta fatia começa
  escrevendo esses testes contra o Next, e só depois reescreve.
- Risco: é a maior massa de código e a menos testada.

### 8. Canais e imagens

Caminhos: `/ml/*`, `/canais/*`, `/melhor-envio/callback`, e as rotas de painel
e admin do Mercado Livre.

- Mercado Livre: publicação, pedidos, perguntas, reputação, avisos e as duas
  rotinas. O `docs/ROTINAS.md` documenta uma janela de venda dupla; a fatia
  não entra sem teste que a cubra.
- Conexão OAuth de Amazon, Shopee e Magalu.
- Processamento de imagem do catálogo pelo `motor-web`, com fila no banco.
- Remoção de fundo: o modelo `u2netp.onnx` roda pelo crate `ort`. É a única
  dependência com biblioteca nativa pesada; fica atrás de uma *feature* e, se o
  build ou a memória não fecharem, vira um serviço separado.
- Etiqueta pela CepCerto (`src/lib/postagem.ts`) é legado e não é portada sem
  decisão de Nicolas: não há contrato com a CepCerto.

### Desligamento do Next

Depois da fatia 8 e de 30 dias sem volta de nenhuma fatia:

- O container do Next é parado e removido do compose.
- `prisma/`, `src/` e `packages/` saem do repositório em um commit, que fica
  no histórico.
- O `AGENTS.md` do Lojas é reescrito para a base Rust.

## Ordem e dependências

```
0 Fundação
├─ 1 Borda e descoberta        (precisa do motor-web pronto)
├─ 2 API de desenvolvedores
├─ 3 Agendador e eventos       (rotinas sem dinheiro)
├─ 5 Vitrine de leitura        (precisa do motor-web pronto)
│   └─ 6 Checkout e pagamento  (leva as rotinas de dinheiro)
│       └─ 7 Painel
│           ├─ 4 Conector MCP  (desenvolvido antes, cortado aqui)
│           └─ 8 Canais e imagens
```

As fatias 1, 2 e 3 não dependem uma da outra e podem ser feitas em paralelo
por agentes diferentes.

## Como uma fatia é dada por pronta

1. Testes de paridade passam contra o Rust.
2. Roda em produção em modo sombra, quando a fatia só lê: o Rust recebe cópia
   do pedido e a resposta é comparada com a do Next, sem ser entregue.
3. Os caminhos entram na lista do Caddy para uma loja.
4. Uma semana sem alerta e sem diferença nas métricas da loja.
5. Os caminhos entram para as demais.
6. O código correspondente do Next é removido, e a fatia não volta mais sem
   reverter esse commit.

Fatia que escreve (6, 7, 8) não tem modo sombra. O que a substitui é o teste em
loja de teste e o corte loja a loja.

## O que não tem equivalente pronto

| Hoje | Em Rust | Risco |
|---|---|---|
| SDK `@google/genai` | HTTP direto para a API do Gemini | Baixo |
| `onnxruntime-node` | crate `ort` | Médio: biblioteca nativa, tamanho da imagem |
| `sharp` | `motor-web` com `image`, `webp`, `ravif` | Médio: AVIF é lento, exige fila |
| SMTP próprio sobre `node:net` | crate `lettre` | Baixo |
| Leitor e escritor de `.xlsx` próprios | `calamine` e `rust_xlsxwriter` | Baixo |
| Componentes React do checkout | Template e script estático com o SDK do Mercado Pago | Médio: tela que cobra |
| `unstable_cache` e `cache()` | Cache em memória por loja, invalidado na escrita | Médio: cache errado mostra preço velho |
| DNS pela API da Cloudflare ao provisionar | A decidir: registro curinga no domínio-base dispensa criar DNS por loja | Depende da decisão |
| 84 componentes de cliente | Templates, htmx e scripts estáticos | Alto pelo volume |

## Sem Cloudflare no Lojas

Hoje `src/lib/provisionar.ts` e `src/lib/dominio.ts` criam DNS pela API da
Cloudflare, e o apex e `/uploads` passam por ela. Proposta:

- Subdomínio de loja: um registro curinga `*.lojas.avilaops.com` para o
  servidor de aplicações. Criar loja deixa de mexer em DNS.
- Domínio próprio: o lojista aponta o registro A, e o TLS sob demanda do Caddy
  emite o certificado depois de `GET /api/dominio-permitido` confirmar.
- `/uploads` servido pelo próprio servidor, com cache longo.

**A decidir por Nicolas:** onde fica o DNS da zona `avilaops.com` quando sair
da Cloudflare. Sem essa resposta o registro curinga não tem onde ser criado.

## Riscos

- **Duração.** São cerca de 59 mil linhas com regras de negócio acumuladas.
  Durante a reescrita, toda mudança no Next precisa ser replicada ou adiada. A
  spec propõe congelar funcionalidade nova na parte ainda não migrada, com
  exceção de correção.
- **Dois agentes no mesmo repositório.** Quem mexe no Next e quem escreve o
  Rust precisam saber qual fatia está em andamento. O `AGENTS.md` do Lojas
  ganha uma tabela com o estado de cada fatia.
- **Painel sem teste.** A fatia 7 não tem contrato automatizado hoje.
- **Cache.** O Next atual não guarda a loja entre requisições justamente por
  causa de um bug antigo. O cache de página do Rust é novo e precisa de
  invalidação testada.
- **Esquema com dois leitores.** Enquanto o Next ler o banco, coluna renomeada
  ou removida quebra o Next. Só se acrescenta.

## O que muda no repositório do Lojas

Nada é alterado lá nesta etapa. Quando esta spec for aprovada:

- `AGENTS.md`: registrar a decisão, o estado das fatias e a regra de que o
  Prisma não migra mais.
- `docs/`: esta spec.
- `deploy`: tirar `prisma migrate deploy` do caminho do Next.
- `servidor/`: o *workspace* Cargo.

## Critério de pronto do projeto

1. Todas as fatias cortadas em todas as lojas.
2. As 10 suítes de integração atuais, mais as escritas nas fatias 3, 7 e 8,
   passam contra o Rust.
3. Trinta dias sem volta de fatia.
4. Next desligado e removido.

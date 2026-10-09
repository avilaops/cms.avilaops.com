# CMS: desenho

Data: 09/10/2026. Aplicação que cria, edita e serve sites institucionais e
blogs, vários por instalação.

Convenções comuns: `2026-10-09-base-rust-design.md`. Regras de conteúdo:
`2026-10-09-motor-web-design.md`.

## Decisões de Nicolas

- Rust: Axum, SQLx, Askama. Painel por template.
- Editam: o cliente, a equipe da Ávila Ops e agentes de IA pelo conector.
- Criam site: o cliente, a equipe e agentes de IA pelo conector.
- Endereço: todo site nasce em subdomínio da Ávila Ops e pode ganhar domínio
  próprio.
- Papéis: Dono, Editor e Autor.
- Prova da primeira versão: um site novo de demonstração.
- Sem Cloudflare e sem Twilio. n8n nas automações de negócio.

## Objetivo

Uma pessoa sem conhecimento técnico cria um site, escreve páginas e posts, e o
que vai ao ar já sai com cabeçalho, dados estruturados, imagens, sitemaps e
`llms.txt` corretos. O que quebraria isso não é publicado.

### Fora do escopo desta versão

- Catálogo, carrinho e checkout. São do Lojas.
- Cobrança e planos. Ver "Pendências".
- Editor de layout livre, arrastar seções, CSS do cliente.
- Comentários, newsletter, formulários com armazenamento de contatos.
- Vários idiomas por site.
- Importação de WordPress ou de outro CMS.

## Endereços

| Endereço | O que serve |
|---|---|
| `cms.avilaops.com` | Painel, API, conector e a página do produto |
| `<slug>.sites.avilaops.com` | O site, desde a criação |
| Domínio do cliente | O mesmo site, depois de apontado e conferido |

- `sites.avilaops.com` é proposta de nome; o domínio-base é configuração.
- Com domínio próprio ativo, o subdomínio redireciona para ele com 301, e a
  canônica passa a ser o domínio próprio.
- Domínio próprio: o Dono informa o domínio, o painel mostra o registro A a
  criar, e o site só responde nele depois de a aplicação resolver o nome e
  confirmar que aponta para o servidor. O Caddy emite o certificado sob
  demanda, perguntando a `GET /api/dominio-permitido`.
- Um domínio pertence a um site só. Slugs reservados (`www`, `api`, `painel`,
  `admin`, `cms`, nomes de produtos da casa) não podem ser usados.
- Site sem domínio próprio sai com `noindex` até o Dono marcar que o endereço
  provisório é o definitivo. Evita dois endereços indexados para o mesmo
  conteúdo.

## Contas, sites e papéis

O login é do Auth. O CMS guarda a participação: conta, site e papel.

| Ação | Autor | Editor | Dono |
|---|---|---|---|
| Criar e editar rascunho próprio | sim | sim | sim |
| Editar rascunho de outra pessoa | não | sim | sim |
| Enviar para revisão | sim | sim | sim |
| Publicar, despublicar, agendar | não | sim | sim |
| Enviar mídia | sim | sim | sim |
| Apagar mídia em uso | não | não | não |
| Autores, categorias, menu | não | sim | sim |
| Identidade, tema, domínio, robôs | não | não | sim |
| Equipe e convites | não | não | sim |
| Conector e chaves | não | não | sim |
| Apagar o site | não | não | sim |

- Quem cria o site é o Dono. Um site tem pelo menos um Dono.
- A equipe da Ávila Ops (papel `ADMIN` no token do Auth) entra em qualquer
  site com poder de Dono, e toda ação dela fica no histórico com esse rótulo.
- Convite por e-mail: a pessoa entra pelo Auth e a participação é criada no
  primeiro acesso.
- A permissão é conferida em uma camada só do servidor, por ação, nunca no
  template.

### Criação de site

Três portas, uma função (`criar_site`):

- **Cliente:** tela "Novo site" no painel.
- **Equipe:** a mesma tela, podendo indicar o e-mail de quem será o Dono.
- **Agente de IA:** ferramenta `criar_site` do conector, em nome da conta que
  autorizou.

Como a criação é aberta, a primeira versão já nasce com freios:

- Limite de sites por conta e de criações por dia, configuráveis.
- Site novo fica como "em montagem": responde no subdomínio com `noindex` e
  não aceita domínio próprio até ter a home publicada.
- A equipe pode suspender um site. Site suspenso sai do ar com 410 e some dos
  sitemaps. Suspender é decisão de gente.

Pendência no Auth: o CMS precisa que qualquer conta do Auth possa entrar
(aplicação cadastrada como não restrita) e que exista cadastro de conta aberto
no Auth. Isso precisa ser confirmado lá antes do plano de implementação.

## Modelo de dados

Migrações SQLx. Todas as tabelas de conteúdo têm `site_id`, e toda consulta
filtra por ele.

| Tabela | O que guarda |
|---|---|
| `site` | slug, nome, descrição, idioma, situação, identidade (logo, cores, fontes), organização, diretrizes para IA, opções de robôs, tema |
| `dominio` | site, host, situação (pendente, ativo), conferido em |
| `participacao` | conta do Auth (`sub`, e-mail), site, papel |
| `convite` | site, e-mail, papel, token só como hash, validade |
| `autor` | site, slug, nome, cargo, bio, foto, perfis, credenciais |
| `categoria` | site, slug, nome, descrição |
| `documento` | site, espécie (página ou post), slug, situação, versão publicada, versão de rascunho, publicado em, atualizado em, agendado para |
| `versao` | documento, número, conteúdo completo em JSON no contrato do `motor-web`, quem gravou, quando |
| `midia` | site, nome, hash, largura, altura, `alt`, legenda, crédito, situação das variantes |
| `variante` | mídia, formato, largura, caminho, bytes |
| `uso_de_midia` | mídia, documento: de onde cada imagem é usada |
| `redirecionamento` | site, caminho antigo, caminho novo |
| `menu` | site, posição (topo, rodapé), itens em ordem |
| `tarefa` | fila: tipo, alvo, situação, tentativas |
| `evento` | o que vira automação: tipo, chave do fato, situação |
| `historico` | quem fez o quê em qual documento, sem o conteúdo |
| `cliente_mcp`, `conexao_mcp`, `chamada_mcp` | conector, como no Lojas |
| `limite` | janelas do limitador de requisições e de tentativas |

Pontos do modelo:

- **Rascunho e publicado são versões.** Editar um post publicado cria nova
  versão de rascunho; o site continua servindo a publicada até alguém
  publicar. Cada publicação guarda a versão, então voltar é apontar para a
  anterior.
- **O conteúdo é o tipo do `motor-web`**, gravado em JSON. O banco não tem uma
  coluna por campo de SEO: o contrato é do motor, e o CMS não o duplica.
- **Slug único por site e espécie.** Trocar o slug de documento publicado cria
  o redirecionamento na mesma transação, o que satisfaz a trava do motor.
- **Mídia em uso não é apagada.** `uso_de_midia` diz onde ela aparece.
- **Datas são do servidor.** "Publicado em" é a primeira publicação;
  "atualizado em" só muda quando `deve_atualizar_data` diz que mudou.

## Fluxo de publicação

```
rascunho ──enviar──▶ em revisão ──publicar──▶ publicado
    ▲                    │                        │
    └────devolver────────┘         editar cria novo rascunho
```

- Salvar rascunho nunca é barrado. O painel mostra os problemas do motor a
  cada salvamento, com o campo marcado.
- "Enviar para revisão" e "Publicar" rodam `validar` no servidor. Com problema
  que bloqueia, a ação é recusada e a lista volta.
- Autor envia para revisão. Editor e Dono publicam, do rascunho ou da revisão.
- Agendar publica na hora marcada, por rotina, com a validação rodando de
  novo no momento.
- Publicar, na mesma transação: grava a versão, atualiza datas, cria
  redirecionamento se o slug mudou, registra no histórico, emite o evento
  `conteudo.publicado` e invalida o cache do site.
- Depois da transação: o n8n avisa o IndexNow e devolve o resultado.
  "Buscadores avisados" só aparece no painel com pelo menos uma resposta 2xx.

O `contexto` da validação (títulos e descrições em uso, slug publicado,
redirecionamentos, peso da página) é montado pelo CMS a partir do banco.

## Renderização pública

- O servidor resolve o site pelo host e o documento pelo caminho, monta o
  `Documento` do motor a partir da versão publicada e renderiza o template.
- `<head>`: `montar_cabecalho`. Corpo: `renderizar_corpo`. Capa:
  `renderizar_midia` com papel principal.
- Rotas de cada site: `/`, `/<pagina>`, `/blog`, `/blog/<post>`,
  `/blog/categoria/<slug>`, `/autor/<slug>`, `/sitemap.xml` e filhos,
  `/robots.txt`, `/llms.txt`, `/llms-full.txt`.
- Caminho com redirecionamento responde 301. Documento despublicado responde
  410; nunca existiu, 404.
- Listagens paginadas, com canônica na própria página.

### Tema

A primeira versão tem um tema só, bem feito, e ele é dado: cores, fontes entre
uma lista fechada, logo e itens de menu. Tema novo entra como contrato, à
maneira dos templates do Lojas, e não como `if` no template.

- CSS em um arquivo, sem framework carregado no navegador. O que é crítico vai
  embutido no `<head>`.
- Fontes servidas pelo próprio servidor, com `font-display: swap`. Nenhuma
  requisição a terceiros na página padrão.
- Nenhum JavaScript na página pública padrão.

### Sinais de confiança

Saem do conteúdo, sem o cliente montar:

- Caixa do autor no fim do post: foto, nome, cargo, bio, credenciais e perfis.
- "Publicado em" e "Atualizado em" visíveis, as mesmas datas do JSON-LD.
- Página do autor com os posts dele.
- Rodapé com razão social, endereço e contato, quando preenchidos.
- Trilha de navegação visível, a mesma do `BreadcrumbList`.

### Scripts de terceiros

O Dono pode informar o identificador do Google Tag Manager. O campo aceita só
o identificador, não código. Com ele preenchido, o painel avisa que a nota de
desempenho passa a depender do que estiver no contêiner.

### Cache

Em memória, por host e caminho, só para pedido sem cookie de sessão.
Publicar, despublicar, mudar tema, menu ou domínio invalida o site inteiro. O
cache guarda a resposta pronta e tem limite de memória; passou, descarta o
menos usado.

## Painel

Templates Askama, formulários, htmx. Telas:

- **Início do site:** o que falta para publicar bem (home sem descrição, autor
  sem foto, imagens sem variante), em linguagem simples.
- **Páginas e Posts:** lista com situação, busca e filtro; editor.
- **Editor:** título, resumo, capa, corpo, e ao lado o painel de busca (título
  e descrição com contador e prévia de como aparece no buscador). Os problemas
  do motor aparecem junto do campo.
- **Mídia:** envio, `alt` obrigatório no envio, onde cada imagem é usada.
- **Autores, Categorias, Menu.**
- **Aparência:** identidade e tema, com prévia.
- **Domínio:** endereço provisório, domínio próprio e situação.
- **Buscadores e IA:** sitemaps gerados, `robots.txt`, `llms.txt`, robôs de IA
  liberados, diretrizes para assistentes.
- **Equipe, Conector, Histórico.**

### Editor de blocos

Uma lista vertical de blocos, cada um com o seu formulário. Sem área de texto
livre com HTML.

- Acrescentar bloco escolhendo o tipo; subir, descer e remover por botão.
- Parágrafo com negrito, itálico e link, em um campo simples que grava
  `Trecho`, não HTML. É o único ponto do painel com JavaScript próprio além do
  htmx.
- Título só oferece os níveis permitidos naquele ponto, então pular nível não
  é possível pela tela. A trava do servidor continua valendo para o conector.
- Salvamento automático do rascunho.
- Prévia: a página renderizada pelo mesmo template do site, a partir do
  rascunho, em rota do painel. Só lê: não grava, não conta visita.
- Funciona no celular: é uma lista de formulários.

## Mídia

1. O arquivo chega; `validar_upload` recusa formato e tamanho errados.
2. O original é gravado, a linha de `midia` nasce com dimensões e `alt`, e a
   resposta volta.
3. Uma tarefa gera as variantes com `processar_imagem`, fora do pedido.
4. Enquanto não há variante, a imagem não pode ser capa de documento
   publicado: a publicação espera ou é recusada com mensagem clara.

Arquivos em disco no volume do container, em caminho com o hash, servidos com
cache longo. Limite de espaço por site, configurável.

## Conector para assistentes de IA

Mesmo desenho do Lojas: o cliente cola o endereço do conector no assistente e
autoriza em uma tela do CMS. O servidor de autorização é o próprio CMS, com
registro dinâmico e PKCE S256. Não há lista de assistentes aceitos.

- A conexão é da conta e pode o que a pessoa marcou ao autorizar, limitado ao
  papel dela em cada site.
- Cada ferramenta tem escopo. O histórico guarda ferramenta e identificador,
  nunca argumentos nem resultado.

| Ferramenta | Escopo |
|---|---|
| `listar_sites`, `ver_site` | `sites:ler` |
| `criar_site` | `sites:criar` |
| `listar_documentos`, `ver_documento` | `conteudo:ler` |
| `criar_rascunho`, `editar_rascunho` | `conteudo:escrever` |
| `validar_documento` | `conteudo:ler` |
| `enviar_para_revisao` | `conteudo:escrever` |
| `publicar`, `despublicar` | `conteudo:publicar` |
| `listar_midia`, `enviar_midia` | `midia:escrever` |
| `listar_autores`, `listar_categorias` | `conteudo:ler` |

- `validar_documento` devolve os problemas do motor, com código, campo e
  mensagem. É assim que o assistente corrige antes de pedir a publicação.
- `publicar` passa pela mesma função do painel. Por padrão, o escopo
  `conteudo:publicar` vem desmarcado na tela de autorização: o assistente
  escreve e envia para revisão, e uma pessoa publica.
- `enviar_midia` exige `alt`. Imagem por endereço segue a regra de requisição
  para endereço informado por cliente, da base Rust.

## O que fica com o n8n

Decisão de Nicolas (09/10/2026): o n8n assume o que não precisa ser código
Rust. A régua é uma só: **o que acontece depois do fato e fala com terceiros
vai para o n8n; o que decide se algo pode acontecer fica no Rust.**

| Trabalho | Onde | Motivo |
|---|---|---|
| Validar, publicar, agendar, despublicar | Rust | É a trava do produto |
| Login, papéis, permissões, limites | Rust | Segurança |
| Renderizar página, cache, sitemaps, `llms.txt` | Rust | Caminho do visitante |
| Variantes de imagem | Rust | Usa o `motor-web` |
| Conferir domínio próprio e liberar o certificado | Rust | Decide emissão de TLS |
| Conector MCP | Rust | Escopo e histórico por chamada |
| E-mail de convite, de revisão pendente e de publicação | n8n | Mensagem a pessoa, depois do fato |
| Aviso aos buscadores (IndexNow) | n8n | Chamada a terceiros, com nova tentativa |
| Medição periódica de desempenho dos sites | n8n | Agendado, usa API de terceiros |
| Aviso à equipe quando um site é criado | n8n | Criação é aberta; alguém precisa olhar |
| Tarefa no Todoist quando algo precisa de gente | n8n | Já é o destino dos erros da casa |
| Relatório periódico ao dono do site | n8n | Agendado, junta dados de fora |

Com isso o CMS não tem cliente de SMTP, não tem modelo de e-mail e não tem
cliente do IndexNow.

Todo e-mail sai de `noreply@avilaops.com`. O nome de exibição é o do site, e o
`Reply-To` é o e-mail de contato do site, quando houver.

### Contrato

- **Saída.** O CMS grava o evento na tabela `evento`, na mesma transação do
  fato, com a chave do fato. A rotina `eventos.entregar` faz
  `POST https://n8n.avilaops.com/webhook/cms-eventos` com `authorization`, e o
  n8n responde na hora. É o desenho do `lojas-eventos`.
- **Corpo.** `{ id, tipo, chave, ocorridoEm, site, dados }`. `dados` é projeção
  explícita por tipo, nunca a linha do banco.
- **Entrega.** Pelo menos uma vez. O mesmo fato tem o mesmo `id`, e o workflow
  descarta repetido.
- **Volta.** O n8n encerra o evento em
  `POST /api/admin/eventos/<id>/encerrar`, com o resultado. É assim que
  "buscadores avisados" só aparece no painel com resposta 2xx de verdade.
- **Autenticação da volta.** Token próprio do n8n, comparado em tempo
  constante. Não é o token de administração.

Eventos: `site.criado`, `convite.criado`, `conteudo.enviado_para_revisao`,
`conteudo.publicado`, `conteudo.despublicado`, `dominio.ativado`,
`site.suspenso`.

### O que o CMS não pode depender do n8n para fazer

- Nada no caminho de editar, validar e publicar espera o n8n. Com o n8n fora
  do ar, o site publica igual e os eventos ficam na fila.
- O convite funciona sem e-mail: o painel mostra o link para o Dono copiar. O
  e-mail é conveniência.
- O `dados` de `convite.criado` leva o link pronto, com token de uso único e
  validade curta. As execuções do n8n guardam o corpo recebido, então esse
  token fica legível lá até expirar: por isso a validade é de 48 horas, e não
  de semanas.

### Workflows como código

Os workflows do CMS ficam versionados em `n8n/` neste repositório e são
criados e atualizados pelo conector do n8n, a partir do código. Editar direto
na tela do n8n deixa o repositório para trás; quem editar lá exporta de volta.

- Um workflow por produto, `CMS - Operação`, no padrão do
  `Lojas - Operação Completa`: um webhook de entrada, os agendamentos dentro.
- Todo workflow aponta o erro para o `Handler de Erro Central → Todoist`.
- Cada tipo de evento tem um teste com dados fixados, rodado pelo conector
  antes de publicar o workflow.

### Custo assumido

O que vai para o n8n sai do compilador e da suíte de testes do Rust. Um campo
renomeado no evento não quebra o build: quebra o workflow, em produção. O que
segura isso é o corpo ser projeção explícita, com teste de referência no CMS, e
o teste com dados fixados de cada tipo no n8n.

## Rotinas

| Rotina | Cadência |
|---|---|
| `publicacao.agendada` | 1 min |
| `tarefas.imagens` | 1 min |
| `eventos.entregar` | 1 min |
| `dominios.conferir` | 10 min |
| `metricas.descarregar` | 1 min |
| `historico.limpar` | diária |

## Erros

- Problema de validação nunca é erro de servidor: volta para a tela, no campo.
- Site desconhecido, suspenso ou em montagem tem resposta própria (404, 410,
  página de "em montagem" com `noindex`).
- Falha ao gerar variante: três tentativas; depois, a mídia fica marcada e o
  painel pede novo envio.
- Falha que precisa de gente sai por alerta, não só por registro.

## Site de demonstração

Empresa fictícia, a mesma da fixture do motor: home, três páginas, cinco posts
(um guia técnico, um com perguntas), dois autores. É criado por um comando de
semente, pelo mesmo caminho que o painel usa, e fica no ar em um subdomínio.

O que ele prova:

- Lighthouse no celular: desempenho, acessibilidade, boas práticas e SEO. O
  alvo é 100 em SEO e boas práticas e 95 ou mais nas outras duas, medido na
  home, em um post e na listagem. Abaixo disso, o build falha.
- Teste de dados estruturados do Google sem erro em home, post e guia.
- Sitemaps válidos; `robots.txt` e `llms.txt` no ar.
- Um post criado, validado, corrigido e enviado para revisão só pelo conector.

O alvo de desempenho é do tema padrão sem script de terceiros. Com GTM, a nota
é do cliente.

## Testes

- Unitários das regras de permissão e do fluxo de publicação.
- Integração contra Postgres em container: criar site, convidar, escrever,
  barrar publicação inválida, publicar, trocar slug com redirecionamento,
  agendar, despublicar, domínio próprio, limites de criação.
- Isolamento entre sites: toda rota de leitura e de escrita testada com conta
  de outro site, esperando recusa.
- Conector: fluxo de autorização e cada ferramenta com escopo certo e errado.
- HTML público comparado com referência.
- Lighthouse do site de demonstração no build.

## Critério de pronto

1. Site de demonstração no ar, com os alvos de medição atingidos.
2. Uma pessoa de fora da equipe cria um site, publica a home e um post, sem
   ajuda, pelo celular.
3. Um assistente de IA cria um rascunho pelo conector, recebe os problemas,
   corrige e envia para revisão.
4. Nenhuma rota entrega dado de um site a conta de outro.

## Pendências que dependem de Nicolas

- **Cobrança.** A criação é aberta, mas esta versão não cobra. Enquanto não
  houver plano e preço definidos, o que segura abuso são os limites. Planos e
  cobrança pelo Mercado Pago ganham spec própria.
- **Nome do domínio-base** dos sites (`sites.avilaops.com` é proposta).
- **Cadastro aberto no Auth**, de que a criação pelo cliente depende.
- **Onde fica o DNS** da zona `avilaops.com` ao sair da Cloudflare, para o
  registro curinga dos subdomínios.

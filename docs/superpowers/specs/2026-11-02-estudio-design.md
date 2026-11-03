# Estúdio: desenho

Data: 02/11/2026. Módulo do CMS para desenhar telas num canvas, com sistema
de design e uma biblioteca de componentes de documentação. Fica atrás do
painel que já existe, no site de quem entrou.

Convenções comuns: `2026-10-09-base-rust-design.md` e
`2026-10-09-cms-design.md`.

## Decisão

O estúdio é do CMS, não um produto à parte. Arquivo, página, nó, biblioteca,
versão e prévia pertencem a um `site_id`. Quem não participa do site recebe o
mesmo 404 de um site que não existe.

A referência de fluxo é um estúdio de canvas com prompt, seletor de sistema,
seletor de arquivo, painel de componentes e prévia por link. Não se copia
código, marca, nome ou asset dessa referência. Os sistemas de design da casa
têm nome e paleta próprios (Padrão, Tinta, Papel, Noite, Oficina, Campo,
Costa, Trama).

Não entra Node, React, Next.js, Cloudflare nem Twilio. Template é Askama.
Interação que não é desenho no canvas é formulário, com htmx só para trocar
a lista de arquivos sem recarregar o resto da home. O canvas (pan, zoom,
seleção e arraste) é um script estático em `crates/web/recursos/estudio.js`,
no mesmo padrão do App Builder. O htmx 2.0.7 vendido em
`crates/web/recursos/htmx-2.0.7.min.js` é o único JavaScript de terceiro, com
a licença BSD-2 em `htmx-2.0.7.LICENCA.txt`.

## O que as doze referências pedem, e onde cada coisa fica

| Referência | O que mostra | Nesta versão | Por quê |
|---|---|---|---|
| 1. Home com prompt, atalhos e abas | Caixa "o que desenhar", atalhos de página, painel, aplicativo e biblioteca, abas Recentes, Favoritos, Bibliotecas e Compartilhados | Dentro | É a porta do módulo. O atalho só escolhe a medida da página e o nome; não gera layout |
| 2. Seletor de sistema com amostras | Lista de sistemas e cores | Dentro, com os oito sistemas da casa | A lista da referência usa marcas de terceiros. Aqui a escolha grava o slug no arquivo e pinta o quadro com os tokens |
| 3. Destino do arquivo | Criar arquivo novo ou usar um existente | Dentro | Arquivo novo cria a página. Arquivo existente guarda o pedido e o sistema; não redesenha o que já está lá |
| 4. Pedido com o sistema em chip | O sistema escolhido aparece no arquivo | Dentro | O chip é o sistema gravado. Trocar é outro formulário no canvas |
| 5. Gerir sistemas: Tailwind, DESIGN.md, importar da web | Criar sistema a partir de arquivo ou de um site | Fora | Não há parser de Tailwind, de Markdown de spec nem extrator de site. A home diz que isso ainda não existe |
| 6. O mesmo modal com "Import from CSS" | Importar CSS externo | Fora | Mesma razão. CSS de fora não entra no processo |
| 7. Canvas vazio, toolbar, Share, zoom | Ferramentas à esquerda, publicar, 100% | Dentro, parcial | Cursor, mão, frame, retângulo e texto funcionam. Mídia e lápis aparecem desabilitados, com o texto de que não estão nesta versão. Pan e zoom ficam no navegador e não vão para o banco |
| 8. Painel do projeto, aba de componentes vazia | Lista do que está no quadro | Dentro | A aba lista os nós do arquivo. Vazio só quando não há nó; o arquivo novo já nasce com o texto do pedido |
| 9. Aba de bibliotecas sem nenhuma ativa | Estado vazio | Dentro | A biblioteca `docs` existe no catálogo e só aparece no canvas depois de ativada no site |
| 10. Menu do plugue: conectar agente e importar da web | Atalhos para agente e captura | Fora, com o controle desabilitado | O conector MCP do estúdio e a extensão de captura não existem. O menu não finge que instala nada |
| 11. Modal de instalar o estúdio num agente de código | Abas Cursor, Codex, Claude Code, Grok, plugin e login | Fora | Não há plugin, MCP nem fluxo de instalação do estúdio. O CMS já tem o conector de conteúdo, que é outra coisa |
| 12. Importar da web com extensão | Captura de elemento e colar URL no chat | Fora | Não há extensão. Colar um endereço no pedido só guarda texto |

Favorito, nesta versão, é uma marca do arquivo no site, não uma preferência
por conta. Preferência por pessoa fica para depois.

## Modelo de dados

Migração `crates/dados/migrations/20261102000018_estudio.sql`. Toda tabela de
conteúdo tem `site_id`. Página, nó, versão e publicação apontam para o
arquivo pela chave `(site_id, arquivo_id, …)`, então um nó não referencia a
página de outro arquivo nem de outro site.

| Tabela | O que guarda |
|---|---|
| `estudio_arquivo` | Nome, pedido, quem criou, favorito do site, slug do sistema |
| `estudio_pagina` | A prancha. Nesta versão cada arquivo tem uma. Largura e altura vêm do atalho |
| `estudio_no` | Frame, retângulo, texto ou componente. Posição, tamanho, texto, preenchimento opcional e, no componente, biblioteca, variante, versão do catálogo e `props` |
| `estudio_biblioteca` | A biblioteca `docs` ativada ou não no site, com a versão do catálogo |
| `estudio_versao` | Retrato imutável do arquivo no momento da publicação. `UPDATE` é recusado pelo gatilho. `DELETE` só acompanha o arquivo ou o site, senão apagar o site quebraria |
| `estudio_publicacao` | Uma prévia por arquivo. O token de 256 bits fica gravado para o editor copiar o link de novo. Republicar troca a versão e mantém o token |

O nó tem de caber na página. Isso é gatilho, não só regra de Rust. Tokens de
cor que um cliente manda só entram se forem `#` e seis dígitos hexadecimais
minúsculos. Fonte é um slug de lista fechada (`sem-serifa`, `serifa`,
`condensada`, `mono`); a família CSS mora na folha estática.

Limites desta versão: 200 arquivos por site, 300 nós por arquivo, pedido de
até 4.000 caracteres, nome de até 80.

## Quem pode o quê

A participação continua sendo a do CMS: Autor, Editor, Dono. A equipe da
Ávila Ops entra como Dono, pelo mesmo `Ator`.

| Ação | Autor | Editor | Dono |
|---|---|---|---|
| Ver a home, os arquivos e o canvas | sim | sim | sim |
| Criar arquivo | sim | sim | sim |
| Editar arquivo, inclusive mover nó | só o que criou | sim | sim |
| Favoritar | só o que criou | sim | sim |
| Publicar ou republicar a prévia | não | sim | sim |
| Ativar ou desativar a biblioteca `docs` | não | sim | sim |
| Inserir componente `docs` | se o arquivo é dele e a biblioteca está ativa | sim, com a biblioteca ativa | sim |

A prévia pública não pede sessão. O endereço é `/estudio/p/<token>` no host
do painel. Não leva slug do site, não lista outros arquivos e não mostra o
rascunho: só a versão publicada. Token errado ou retrato que não passa na
validação responde 404. A resposta é `no-store` e `noindex`.

Escrita no painel continua exigindo `Origin` do próprio painel, no mesmo
ponto das outras rotas.

## Onde entra a geração por chat

O pedido mora em `estudio_arquivo.pedido`. A home e o canvas deixam isso
explícito: a geração não roda.

Não há cliente de modelo no repositório. `crates/integracoes` fala com o
Auth, com a busca e com o n8n. O Langfuse do CMS é rastreio OTLP das
chamadas MCP, não um provedor de texto. O conector MCP deixa um agente de
fora chamar ferramentas de conteúdo; o modelo fica no agente, não aqui.

A interface nova está em `cms_integracoes::geracao`:

- `ConfiguracaoDeGeracao::do_ambiente` lê `ESTUDIO_GERACAO_PROVEDOR`,
  `ESTUDIO_GERACAO_URL` (só `https`) e `ESTUDIO_GERACAO_MODELO`.
- `estado` distingue "não configurado" de "URL e modelo presentes, sem
  adaptador".
- `ProvedorDeGeracao` é o traço. `AdaptadorDesligado` é o único adaptador
  ligado no processo e devolve `NaoConfigurado`.
- `adaptador_inicial` ignora a URL de propósito. Esta versão não chama a
  rede mesmo que a variável exista.

A fase seguinte implementa o adaptador HTTP, grava nós a partir da proposta
e passa a chamar o traço no `POST` do pedido. Até lá o binário não finge que
gera tela.

## Biblioteca de documentação

Pedido: a biblioteca de UI do Fumadocs disponível nas bibliotecas do
estúdio, sem Next.js e sem React no runtime.

O Fumadocs (https://github.com/fuma-nama/fumadocs) é um framework de
documentação em React. Os pacotes oficiais puxam Next, Vite, Radix, MDX e
Tailwind. Nenhum desses entra no processo do CMS.

O que dá para trazer dentro do padrão é o catálogo e os tokens, não o
runtime:

- Licença do upstream: MIT, Copyright (c) 2023 Fuma. O texto integral e a
  revisão lida (`76408e07e37cbe9708c194ad4f0ebd49a974a236`, branch `dev`, em
  02/11/2026, arquivo `packages/radix-ui/css/lib/default-colors.css`) estão
  em `crates/web/recursos/biblioteca-docs/AVISO.txt`.
- Os valores de cor neutros (fundo, texto, cartão, borda, primária e os tons
  info, aviso, erro, sucesso e ideia) foram reescritos como variáveis CSS
  nossas na folha do estúdio. Não se importa o pacote npm nem o preset
  Tailwind.
- O código React dos componentes não foi copiado. Cada peça é HTML escapado
  pelo Askama.

Catálogo `docs`, versão `2026-11-02`, em `cms_dominio::estudio::pecas_docs`:

| Variante | Peça | O que faz nesta versão |
|---|---|---|
| `lateral` | Barra lateral | Lista de rótulos, sem endereço |
| `sumario` | Sumário | Lista ordenada |
| `navegacao` | Anterior / próxima | Dois rótulos |
| `chamada` | Callout | Tom info, aviso, erro, sucesso ou ideia |
| `cartao` | Cartão | Título e descrição |
| `abas` | Abas | Dois painéis; o script troca o visível. Sem script, os dois aparecem |
| `busca` | Busca | Campo somente leitura. Não há índice |

Fora deste catálogo, e por isso fora desta fatia: acordeão, banner, bloco de
código, árvore de arquivos, passos, tabela de tipos, zoom de imagem e o tema
escuro do Fumadocs. O tema neutro claro é o único aplicado.

Versionamento: a constante `VERSAO_DO_CATALOGO_DOCS` é a revisão do catálogo
no código. Ativar a biblioteca grava essa versão em `estudio_biblioteca`.
Inserir um componente grava a mesma versão e uma cópia das `props` no nó.
Mudar o catálogo depois não reescreve nó já inserido. A prévia publica essa
cópia dentro do JSON da versão.

Aplicar a um frame: a página do arquivo é a prancha. Inserir cria um
`estudio_no` com `tipo = componente`, `biblioteca = docs` e a variante, na
página daquele arquivo. O quadro usa os tokens do sistema de design do
arquivo; o miolo do componente usa as variáveis da biblioteca. São duas
camadas de propósito.

A biblioteca nasce desligada. Editor ou Dono ativa em Estúdio → Bibliotecas.
Aí o canvas lista as sete peças e o formulário de inserir.

## Rotas

No host do painel, com a mesma sessão do resto:

| Método e caminho | Quem |
|---|---|
| `GET /painel/sites/{slug}/estudio` | Participante. `?aba=` filtra a lista |
| `POST /painel/sites/{slug}/estudio` | Cria ou atualiza o arquivo de destino |
| `POST /painel/sites/{slug}/estudio/bibliotecas/docs` | Editor ou Dono |
| `GET /painel/sites/{slug}/estudio/{id}` | Participante. Quem não edita vê o quadro sem formulário |
| `POST …/{id}/sistema` | Quem edita o arquivo |
| `POST …/{id}/favorito` | Quem edita o arquivo |
| `POST …/{id}/nos` | Quem edita o arquivo |
| `POST …/{id}/nos/{no}` | Move. Responde 204 |
| `POST …/{id}/publicar` | Editor ou Dono |
| `GET /estudio/p/{token}` | Qualquer um, sem sessão |
| `GET /recursos/estudio-*.js` e `/recursos/htmx-*.js` | Arquivo estático, com o resumo do conteúdo no nome |

A entrada está na lateral do site e na lista de sites.

## O que esta fatia não promete

- Gerar tela a partir do pedido.
- Importar Tailwind, CSS, DESIGN.md ou um site.
- Extensão de navegador.
- Conector MCP, plugin ou login de agente para o estúdio.
- Várias pranchas por arquivo, edição de texto do nó, histórico do site,
  favorito por conta, tema escuro da biblioteca `docs` e as peças que ficaram
  de fora do catálogo.

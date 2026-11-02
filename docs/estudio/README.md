# Estúdio

Módulo do painel em `/painel/sites/{slug}/estudio`. A spec está em
`docs/superpowers/specs/2026-11-02-estudio-design.md`.

## O que funciona

- Home com o pedido, o destino (arquivo novo ou existente), os oito sistemas
  de design da casa e as abas Recentes, Favoritos, Bibliotecas e
  Compartilhados.
- Criar e abrir um arquivo. O atalho escolhe a medida da página (página de
  entrada, painel, aplicativo ou biblioteca) e o texto inicial. Não desenha
  uma tela sozinho.
- Canvas com pan, zoom, seleção e arraste. Dá para colocar frame, retângulo
  e texto. A posição vai para o Postgres.
- Aplicar um sistema. Cor e fonte do quadro saem dos tokens desse sistema.
- Publicar uma prévia somente leitura em `/estudio/p/{token}`. O link não
  mostra edição feita depois da publicação. Republicar atualiza o retrato e
  mantém o token. Quem não é editor nem dono não publica.
- Biblioteca `docs`, desligada até um editor ou dono ativar. São sete peças
  de documentação (barra lateral, sumário, navegação, chamada, cartão, abas
  e busca visual) em HTML. A busca não indexa. A licença do tema neutro do
  Fumadocs está em `crates/web/recursos/biblioteca-docs/AVISO.txt`.

Quem não participa do site não vê o estúdio. Autor edita só o arquivo que
criou.

## O que não existe

- Geração de telas por chat. O pedido fica gravado. O adaptador em
  `cms_integracoes::geracao` está desligado e não chama modelo nenhum.
- Importar Tailwind, CSS, DESIGN.md ou um site.
- Extensão de navegador.
- Conector MCP, plugin ou instalação do estúdio num agente de código.
- Mídia, lápis, várias pranchas, edição do texto de um nó, favorito por
  conta e o restante do catálogo de documentação (acordeão, código, passos,
  tema escuro).

Os botões dessas funções, quando aparecem, estão desabilitados e dizem que
não estão nesta versão.

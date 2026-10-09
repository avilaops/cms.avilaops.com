# CMS Ávila Ops

Plataforma própria de sites da Ávila Ops, ao lado do
[Lojas](https://lojas.avilaops.com). O site já nasce correto em SEO técnico,
desempenho, dados estruturados, imagens, `llms.txt` e sinais de confiança, e o
painel impede quem edita de estragar isso.

## Estado

O motor está implementado e publicado como biblioteca. CMS e Lojas em Rust
têm spec aprovada e nenhum código: nenhum site está no ar.

| Frente | Entrega | Situação |
|---|---|---|
| Base | Convenções comuns em Rust: servidor, banco, login, publicação, n8n | Spec aprovada |
| Motor | Crate `motor-web`: validação, cabeçalho, JSON-LD, imagens, sitemaps, `robots.txt`, `llms.txt` | Implementado e publicado em [avilaops/motor-web](https://github.com/avilaops/motor-web) |
| CMS | Painel, conector MCP e renderização por host | Spec aprovada, código não iniciado |
| Lojas | Reescrita do Lojas em Rust, por fatias de rota | Spec aprovada, código não iniciado |

Este repositório hoje só tem documentos. O código do CMS vai morar aqui.

## Pilha

Rust, com Axum no servidor, SQLx sobre Postgres, Askama nos templates e htmx
na interação. Um binário por aplicação, publicado em container no servidor de
aplicações da Ávila Ops. Sem Node em produção. O n8n cuida do que acontece
depois do fato e fala com terceiros.

## Como se organiza

- **Motor (`motor-web`)**: crate sem banco e sem rede. Recebe conteúdo e
  devolve os problemas que impedem a publicação e os artefatos da página. É
  usado pelo CMS e pelo Lojas, fixado por commit.
- **CMS (este repositório)**: uma aplicação que serve vários sites por host,
  com painel, login pelo Auth e conector para assistentes de IA.
- **Lojas**: dono de catálogo, carrinho e checkout. Passa do Next.js para Rust
  uma fatia de rotas por vez, no mesmo domínio e no mesmo banco.
- **n8n**: recebe os eventos de cada produto por webhook e cuida de e-mail,
  aviso a buscadores, medição agendada e tarefas para a equipe. Os workflows
  ficam versionados no repositório do produto.

## Decisões que valem para tudo

Tomadas por Nicolas em 08 e 09/10/2026. O detalhe de cada uma está nas specs.

- A plataforma é Rust desde o dia 0, com painel renderizado por template.
- SQLx é o dono do esquema do banco.
- Sem Cloudflare e sem Twilio.
- O que decide se algo pode acontecer fica no Rust; o que acontece depois do
  fato e fala com terceiros vai para o n8n.
- Todo e-mail sai de `noreply@avilaops.com`.
- Nenhuma regra mora na tela: painel, API e conector passam pela mesma
  validação do motor.
- O Lojas migra por fatias de rota, e o envio de mensagens dele continua no
  servidor.
- Este repositório e o do motor são públicos: nada de dado, nome ou domínio de
  cliente em código, teste ou documento.

## Documentos

- [Base Rust](docs/superpowers/specs/2026-10-09-base-rust-design.md)
- [Motor](docs/superpowers/specs/2026-10-09-motor-web-design.md)
- [CMS](docs/superpowers/specs/2026-10-09-cms-design.md)
- [Lojas em Rust](docs/superpowers/specs/2026-10-09-lojas-rust-design.md)
- [Modo de trabalho dos agentes](AGENTS.md)

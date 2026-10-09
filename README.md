# CMS Ávila Ops

Plataforma própria de sites da Ávila Ops, ao lado do
[Lojas](https://lojas.avilaops.com). O site já nasce correto em SEO técnico,
desempenho, dados estruturados, imagens, `llms.txt` e sinais de confiança, e o
painel impede quem edita de estragar isso.

## Estado

O motor está implementado e publicado como biblioteca. CMS e Lojas em Rust
ainda são desenho: nenhum site está no ar.

| Frente | Entrega | Situação |
|---|---|---|
| Base | Convenções comuns em Rust: servidor, banco, login, publicação | Spec escrita, aguardando revisão |
| Motor | Crate `motor-web`: validação, cabeçalho, JSON-LD, imagens, sitemaps, `robots.txt`, `llms.txt` | Implementado em [avilaops/motor-web](https://github.com/avilaops/motor-web), commit `0f959b6` |
| CMS | Painel, conector MCP e renderização por host | Spec escrita, aguardando revisão |
| Lojas | Reescrita do Lojas em Rust, por fatias de rota | Spec escrita, aguardando revisão |

## Pilha

Rust, com Axum no servidor, SQLx sobre Postgres, Askama nos templates e htmx
na interação. Um binário por aplicação, publicado em container no servidor de
aplicações da Ávila Ops. Sem Node em produção.

## Como se organiza

- **Motor (`motor-web`)**: crate sem banco e sem rede. Recebe conteúdo e
  devolve os problemas que impedem a publicação e os artefatos da página. É
  usado pelo CMS e pelo Lojas, fixado por commit.
- **CMS (este repositório)**: uma aplicação que serve vários sites por host,
  com painel, login pelo Auth e conector para assistentes de IA.
- **Lojas**: dono de catálogo, carrinho e checkout. Passa do Next.js para Rust
  uma fatia de rotas por vez, no mesmo domínio e no mesmo banco.

## Documentos

- [Base Rust](docs/superpowers/specs/2026-10-09-base-rust-design.md)
- [Motor](docs/superpowers/specs/2026-10-09-motor-web-design.md)
- [CMS](docs/superpowers/specs/2026-10-09-cms-design.md)
- [Lojas em Rust](docs/superpowers/specs/2026-10-09-lojas-rust-design.md)
- [Regras para agentes](AGENTS.md)

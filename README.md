# CMS Ávila Ops

Plataforma própria de sites da Ávila Ops, ao lado do
[Lojas](https://lojas.avilaops.com). O site já nasce correto em SEO técnico,
desempenho, dados estruturados, imagens, `llms.txt` e sinais de confiança, e o
painel impede quem edita de estragar isso.

## Estado

Em desenho. Não há código nem nada publicado.

| Ciclo | Entrega | Situação |
|---|---|---|
| 1 | `avilaops/motor-web`: validação, cabeçalho, JSON-LD, imagens, sitemaps, `robots.txt`, `llms.txt` | Spec escrita, aguardando revisão |
| 2 | `avilaops/cms.avilaops.com`: painel, conector MCP e renderização por host | Não iniciado |
| 3 | Lojas passa a usar o motor | Não iniciado |

## Como se organiza

- **Motor (`motor-web`)**: pacote TypeScript sem banco e sem rede. Recebe
  conteúdo e devolve os problemas que impedem a publicação e os artefatos da
  página. É usado pelo CMS e pelo Lojas, fixado por commit.
- **CMS (este repositório)**: uma aplicação Next.js que serve vários sites por
  host, com painel, login pelo Auth e conector para assistentes de IA.
- **Lojas**: continua dono de catálogo, carrinho e checkout.

## Documentos

- [Desenho do motor-web](docs/superpowers/specs/2026-10-09-motor-web-design.md)
- [Regras para agentes](AGENTS.md)

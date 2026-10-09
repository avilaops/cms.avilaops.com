# CMS Ávila Ops: regras para agentes

<!-- avilaops:contexto:inicio (versão 2026-10-03; gerado a partir de avilaops/contexto, não editar aqui) -->
## Contexto Ávila Ops (vale para todos os projetos)

Este repositório pertence à Ávila Ops Tecnologia, que ajuda pequenas empresas a construir presença digital, organizar a operação e crescer. As contas `avilaops` e `avilainc` no GitHub são a mesma empresa. Nicolas Avila (Nicolas sem acento) é o fundador e quem decide.

### Como trabalhar

- Comunicar em português natural, com resposta direta e evidência. Sem tom de coach, promessa vaga ou jargão comercial. O idioma da interface e do conteúdo acompanha o site, não a conversa.
- Identificar o projeto, o domínio, o repositório e o ambiente antes de alterar qualquer coisa. Não presumir que todos os projetos usam o mesmo deploy.
- Ter iniciativa dentro do pedido e levar a tarefa até um resultado verificado. Plano, código, publicação e funcionamento comprovado são coisas diferentes: não declarar sucesso só porque um build terminou ou um workflow foi ativado.
- Proteger dados, acessos e a separação entre clientes. Nunca gravar segredo em arquivo versionado, issue, PR ou memória.
- Não iniciar comunicação externa nem ação irreversível sem autorização do Nicolas.
- Preservar trabalho em andamento de outra pessoa ou de outro agente. Trabalho não commitado vai para uma branch `resgate/*`.

### Decisões vigentes

- Pagamentos: Mercado Pago no Brasil e PayPal para clientes de fora. Não usar Stripe nem Éfi, mesmo que material antigo diga o contrário.
- Automações em n8n, infraestrutura em Cloudflare e canais em Twilio, preservando integrações existentes.
- Ofertas com três planos: entrada limitada, intermediário como escolha principal e premium como referência. Consultar preços vigentes antes de publicar.
- Build de aplicação roda no GitHub Actions, não no servidor de produção.
- Versão antiga de código fica no GitHub. Não criar `.tgz`, `.tar`, `*-before-*` nem pastas `rollback/`, `releases/` ou `backups/` com código no servidor; voltar versão é republicar o commit. Antes de mexer em dado, fazer dump do banco.

### Sessões na nuvem

- Uma sessão de nuvem não tem acesso à máquina do Nicolas, aos servidores nem à memória compartilhada. Não presumir o estado de produção: buscar evidência ou dizer que não foi verificado.
- Decisão durável tomada na sessão deve ficar registrada na descrição do PR e, quando for do projeto, neste arquivo, fora deste bloco.
- A memória compartilhada completa e as regras corporativas ficam no repositório privado `avilaops/contexto`.
<!-- avilaops:contexto:fim -->

## O que prevalece sobre o bloco acima neste projeto

Decisão de Nicolas em 09/10/2026, para a plataforma em Rust (motor, CMS e
Lojas):

- **Sem Cloudflare e sem Twilio.** O bloco corporativo cita os dois; aqui não
  valem. DNS direto para o servidor de aplicações, TLS pelo Caddy, WhatsApp
  pela Cloud API da Meta, e-mail por SMTP próprio.
- **n8n continua** nas automações de negócio: o servidor emite o evento e o n8n
  reivindica o que espera ou depende de terceiros.

## Estado em 09/10/2026

Só existe desenho. Nenhum código, nenhum repositório no GitHub, nada publicado.
Rust não está instalado na máquina do Nicolas.

| Spec | Arquivo em `docs/superpowers/specs/` | Situação |
|---|---|---|
| Base Rust | `2026-10-09-base-rust-design.md` | Escrita, aguarda revisão |
| Motor | `2026-10-09-motor-web-design.md` | Escrita, aguarda revisão |
| Lojas em Rust | `2026-10-09-lojas-rust-design.md` | Escrita, aguarda revisão |
| CMS | `2026-10-09-cms-design.md` | Escrita, aguarda revisão |

Não implementar antes de a spec correspondente e o plano dela serem aprovados.

## Decisões do projeto (Nicolas, 08 e 09/10/2026)

- **A base é Rust desde o dia 0:** Axum, SQLx, Askama. A primeira versão do
  desenho era Next.js 16 com Prisma e foi abandonada em 09/10/2026 antes de
  qualquer código. Não propor Node, React nem Prisma para a plataforma nova.
- **O painel também é Rust**, renderizado por template, com htmx e JavaScript
  pontual servido como arquivo estático. Sem empacotador.
- **SQLx é dono do esquema.** Nenhuma outra ferramenta migra o banco.
- **Três frentes:** `avilaops/motor-web` (crate de regras),
  `avilaops/cms.avilaops.com` (painel, conector e renderização) e a reescrita
  do Lojas por fatias de rota.
- **O motor é compartilhado e mora em repositório próprio.** CMS e Lojas o
  consomem por commit fixo (`rev` no `Cargo.toml`). Mudou o motor, o `rev`
  precisa ser atualizado em quem consome.
- **O CMS não tem catálogo nem checkout.** Produto, preço, estoque e cobrança
  são do Lojas. O tipo `Product` do motor é só o contrato para SEO e dados
  estruturados.
- **Uma aplicação serve N sites, resolvidos por host.** Nada por site em
  código: se a mudança pede `if site == "x"`, vira dado.
- **Regra nunca mora na tela.** Editam o conteúdo o cliente, a equipe da Ávila
  Ops e agentes de IA pelo conector; painel, API e conector passam pela mesma
  validação do motor.
- **Corpo é lista de blocos tipados, sem HTML livre.** O `<h1>` sai do título
  do documento, não do corpo.
- **Rascunho salva sempre; a trava é na publicação.** Problema que bloqueia
  impede publicar; o que avisa não.
- **`aggregateRating` só com avaliação do próprio site.**
- **"Atualizado em" só muda quando o conteúdo muda.**
- **Site é criado pelo cliente, pela equipe ou por agente de IA pelo conector**,
  sempre pela mesma função. Papéis: Dono, Editor e Autor. Todo site nasce em
  subdomínio da Ávila Ops e pode ganhar domínio próprio.
- **A primeira versão do CMS é provada com um site novo de demonstração**, de
  empresa fictícia. Nome de cliente real não entra em fixture, exemplo ou
  teste.
- **Nada de promessa de desempenho sem medição.** Nota do Lighthouse e tempo de
  resposta são medidos, não declarados. "Menos de 5 ms" e "ordens de grandeza"
  não entram em spec, site ou proposta.

## Convenções

- Português nos nomes e comentários.
- Dinheiro em centavos, `i64`.
- Sem `unwrap`, `expect` nem `panic!` fora de teste.
- `cargo fmt`, `cargo clippy -- -D warnings` e `cargo test` antes de entregar.
- Build de aplicação nunca roda no servidor de produção. O caminho em uso é o
  build manual do `avilaops/infra` (`scripts/publicar-manual.ps1`).
- Esta pasta (`D:\Projetos\CMS`) vira o repositório `avilaops/cms.avilaops.com`.
  O motor terá pasta e repositório próprios. O Lojas em Rust mora no
  repositório do Lojas, em `servidor/`.

## Pendências que dependem do Nicolas

- CMS: cobrança e planos, nome do domínio-base dos sites, e cadastro aberto no
  Auth.
- Quantas lojas estão em produção no Lojas (o repositório registra três; outros
  documentos citam mais).
- Onde fica o DNS da zona `avilaops.com` ao sair da Cloudflare.
- Etiqueta pela CepCerto: portar ou aposentar.

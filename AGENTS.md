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

## Estado em 09/10/2026

Só existe desenho. Nenhum código, nenhum repositório no GitHub, nada publicado.
A spec do primeiro ciclo está em
`docs/superpowers/specs/2026-10-09-motor-web-design.md` e aguarda a revisão do
Nicolas. Não implementar antes da aprovação da spec e do plano.

## Decisões do projeto (Nicolas, 08 e 09/10/2026)

- **Três ciclos, nesta ordem:** `avilaops/motor-web` (regras e componentes),
  `avilaops/cms.avilaops.com` (painel, conector e renderização) e adoção do
  motor pelo Lojas. Cada ciclo tem spec e plano próprios.
- **O motor é compartilhado e mora em repositório próprio.** CMS e Lojas o
  consomem por commit fixo (`github:avilaops/motor-web#<commit>`). Mudou o
  motor, o pin precisa ser atualizado em quem consome.
- **O CMS não tem catálogo nem checkout.** Produto, preço, estoque e cobrança
  são do Lojas. O tipo `Product` do motor é só o contrato para SEO e dados
  estruturados.
- **Uma aplicação serve N sites, resolvidos por host**, como o Lojas. Nada por
  site em código: se a mudança pede `if (site === "x")`, vira dado.
- **Regra nunca mora na tela.** Editam o conteúdo o cliente, a equipe da Ávila
  Ops e agentes de IA pelo conector; painel, API e conector passam pela mesma
  validação do motor.
- **Corpo é lista de blocos tipados, sem HTML livre.** O `<h1>` sai do título
  da página, não do corpo.
- **Rascunho salva sempre; a trava é na publicação.** Problema com gravidade
  `bloqueia` impede publicar, `avisa` não.
- **`aggregateRating` só com avaliação do próprio site.** Nota copiada de outro
  lugar não entra no JSON-LD.
- **"Atualizado em" só muda quando o conteúdo muda.**
- **A primeira versão do CMS é provada com um site novo de demonstração**, de
  empresa fictícia. Nome de cliente real não entra em fixture, exemplo ou
  teste.
- **Nota do Lighthouse não é prometida por construção.** É medida no site de
  demonstração a cada build, no ciclo 2.

## Convenções

- Português nos nomes e comentários; TypeScript estrito.
- Dinheiro em centavos, inteiro.
- Mesma base do Lojas: Next.js 16, Prisma e Postgres, login pelo Auth, deploy
  pelo `avila-deploy` do `avilaops/infra`. Antes de escrever código de
  roteamento do Next 16, ler `node_modules/next/dist/docs/`.
- Esta pasta (`D:\Projetos\CMS`) vira o repositório `avilaops/cms.avilaops.com`.
  O motor terá pasta e repositório próprios.

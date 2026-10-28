# Desenvolvimento MLOps do módulo CMS

## Responsabilidade e ciclo

Codex responde pelo módulo CMS e assume suas demandas sem responsável, conforme
instrução do proprietário em 28/10/2026. O titular humano do projeto no Linear
continua sendo a identidade da conta; não é necessário inventar outro usuário.

Cada incremento define objetivo e critérios de aceite antes da implementação,
vincula regras às fontes e testes, passa pelo workflow `Validar` e conserva
evidência do commit exato. Implementação, validação e implantação são estados
distintos. Builds e testes acontecem exclusivamente no GitHub CI.

## Base implementada

- [Catálogo](cms-policies.json): seis fichas adicionais no formato `PolicyDraft`
  já usado pelo laboratório, com responsável, aplicação, fontes e testes.
- [Manifesto](sources.json): revisão Git, hashes SHA-256 de 12 arquivos e oito
  vínculos de testes. A integridade cobre somente essas fontes.
- [Gate](../../scripts/validar-mlops.mjs): recusa duplicação, referência sem cobertura,
  hash divergente, fonte extra, seletor ausente e promoção indevida da situação.
- [Workflow](../../.github/workflows/validar.yml): executa os testes do gate,
  valida o catálogo e publica `cms-mlops-evidence`, inclusive em falha posterior.

O relatório inclui commit executado, revisão das fontes, hash do catálogo,
referências de testes, URL da execução e resultados observados das etapas Rust
e do gate. `sourceIntegrityVerified` afirma somente integridade dos arquivos.
`productRuntimeVerified` permanece falso: nenhuma execução promove fichas
automaticamente nem certifica operação em produção. `success` em `rustTestsOutcome`
significa sucesso da etapa da suíte; não é relatório individual por regra.

## Cobertura inicial

| Ficha | Capacidade e aceite representativo |
| --- | --- |
| CMS-ACCESS-002 | Autor não publica e não altera documento alheio |
| CMS-ACCESS-003 | Escrita editorial recusa origem ausente ou divergente |
| CMS-ACCESS-004 | Ferramenta MCP recusa falta de escopo e site de outra conta |
| CMS-PUBLICATION-002 | Publicação inválida não produz evento; caminho ocupado é recusado |
| CMS-INTEGRATION-001 | Callback não autorizado é recusado; repetição preserva resultado |
| CMS-ACCESS-005 | Runtime exige credencial própria e participação antes de considerar ETag |

As fichas CMS-ACCESS-001 e CMS-PUBLICATION-001 já existem no inventário parcial
do laboratório; este lote usa outros IDs para preservar as edições existentes.
O aceite do runtime reúne testes de autenticação/ETag e revogação separados;
revogação com ETag no mesmo pedido ainda precisa de cenário específico.

## Atualizar uma regra

1. Definir o comportamento, responsável e teste que pode refutá-lo.
2. Implementar e enviar a alteração ao GitHub. Não executar o produto no servidor.
3. Fixar as referências das fichas na revisão que contém a implementação e
   atualizar os hashes do manifesto com os bytes dos blobs Git dessa revisão.
   Fontes do checkout diferentes da revisão fixada bloqueiam a CI.
   Se o commit não estiver no clone após squash, o gate busca o SHA no origin
   sem alterar o checkout. Revisão indisponível ou arquivo divergente é falha.
4. Conferir os seletores e a relação com o comportamento; existência de nome
   não comprova que o teste é suficiente ou que cobre todos os caminhos.
5. Conferir a CI e o artefato no commit submetido. Registrar falhas e limitações.
6. Importar o JSON pelo caderno privado com motivo e revisão. Códigos existentes
   são preservados pelo importador; atualizar uma ficha exige edição com ETag.

A integração desta base não importa automaticamente o catálogo no serviço
privado. A importação e conferência de histórico precisam de evidência própria.
O lote contém referências de código do repositório, sem dados operacionais;
não acrescenta conteúdo à seleção pública de evidências do laboratório.

## Lacunas e sequência

- Publicação agendada reconstrói ator Editor; autorização vigente não é
  reconsultada nesse caminho. Não foi alterada nesta entrega.
- Eventos aceitos sem encerramento precisam de recuperação e reconciliação;
  a fila não garante efeito externo exatamente uma vez.
- Contratos e compatibilidade de API seguem na AVL-343; este gate verifica
  referências e não substitui comparação semântica de contratos.
- Instrumentação do executor CMS deve adotar o contrato da AVL-342; logs MCP
  atuais não equivalem a traces completos com efeito confirmado.
- Inventário parcial: não certifica todas as rotas, backups, retenção, domínios,
  processamento de mídia ou todas as capacidades do App Builder.

Referências: [AVL-355](https://linear.app/avilaops/issue/AVL-355),
[arquitetura](../arquitetura-cms.md) e
[plano de execução](../superpowers/plans/2026-10-28-mlops-foundation.md).

# Contratos executáveis do módulo CMS

## Escopo e responsabilidade

Codex responde pelo módulo CMS. Esta entrega implementa o recorte CMS da
AVL-343: todas as operações da API de apps v1 e a leitura do runtime móvel.
OAuth e ferramentas MCP continuam módulos separados na sequência; não serão
declarados cobertos por contratos de apps.

## Projeto

Derivar JSON Schema 2020-12 dos DTOs Rust reais, usando Schemars em feature
`contracts`, sem dependência dessa feature na imagem de produção. Os handlers
serializam DTOs concretos, incluindo erros e respostas hoje montadas por JSON.
Um catálogo de operações controla o despacho da API e a exportação, evitando
uma lista de rotas independente do código que atende pedidos.

O contrato exportado tem operação, método, caminho, credencial, cabeçalhos,
schemas de entrada e respostas. Rascunhos aceitam JSON ainda inválido para AAD;
versões publicadas usam o contrato AAD. Regras contextuais de autorização e
integridade permanecem no backend e nos testes com Postgres.

Na CI existente: exportar o contrato do candidato; ler o baseline do commit
base do PR; comparar entradas e saídas por direção; validar exemplos e um
consumidor independente; publicar contratos e relatório. Uma mudança não
suportada pelo comparador deve exigir revisão, nunca passar silenciosamente.
O primeiro contrato é declarado inicial quando o baseline ainda não existe.

## Execução

- [ ] Tipar respostas e derivar schemas dos DTOs e do AAD.
- [ ] Compartilhar catálogo de operações com o despacho real.
- [ ] Exportar contrato pelo código Rust da versão testada.
- [ ] Implementar comparação direcional com testes de quebras de payload.
- [ ] Validar exemplos e consumidor independente, inclusive respostas reais.
- [ ] Integrar e publicar artefatos na CI existente; conferir a revisão exata.
- [ ] Atualizar evidências, tarefas e limites no Linear.

Testes e builds exclusivamente no GitHub CI. Sem deploy. Não reescrever o
workspace, instalar infraestrutura ML ou adicionar o laboratório ao caminho
síncrono dos pedidos.

# Arquitetura e roadmap do CMS Ávila Ops

## Objetivo

Documentar a arquitetura verificável do CMS Ávila Ops e definir um roadmap
incremental para aproximá-lo das capacidades de um WCMS maduro sem importar o
modelo inseguro de extensões arbitrárias dos CMS tradicionais.

## Escopo

O documento cobre o CMS multi-site, o `motor-web`, o Auth, o Lojas, os
conectores MCP, os plugins, o n8n e o Tester. Ele descreve responsabilidades,
fontes de verdade, contratos, fluxos, segurança, observabilidade e evolução.

O roadmap cobre apenas capacidades do CMS e sua arquitetura de integração. Não
transforma o CMS em plataforma de pagamentos: catálogo transacional, carrinho,
checkout, pedidos, estoque e pagamentos continuam pertencendo ao Lojas.

## Princípios obrigatórios

- O código e os documentos devem refletir somente comportamento verificado no
  repositório; intenção futura deve ser marcada como alvo ou roadmap.
- O CMS é a fonte de verdade de sites institucionais e conteúdo editorial.
- O Lojas é a fonte de verdade de comércio transacional.
- Regras síncronas de autorização, validação e consistência ficam no domínio
  Rust; efeitos posteriores e integrações externas ficam no n8n.
- Painel, HTTP e MCP passam pelas mesmas regras de domínio.
- Configuração compartilhada deve ser declarativa, versionada, validada pelo
  Tester e distribuída sem cópia manual.
- Extensibilidade deve usar contratos, eventos, componentes aprovados e versões;
  clientes não instalam código arbitrário no processo do CMS.
- Nenhum segredo, cliente real ou domínio de cliente entra nos documentos.
- Publicação em produção, push e merge não fazem parte desta entrega.

## Documento de arquitetura

Criar `docs/arquitetura-cms.md` em português, com:

1. contexto e limites do sistema;
2. diagrama Mermaid de contexto;
3. diagrama Mermaid dos componentes internos do CMS;
4. tabela de responsabilidades e fontes de verdade;
5. fluxos de leitura pública, edição/publicação e propagação de configuração;
6. modelo multi-tenant e resolução por host;
7. autenticação, papéis, OAuth/MCP e limites de confiança;
8. conteúdo, mídia, versões, eventos, cache e observabilidade;
9. contratos entre CMS, Lojas, plugins, n8n e Tester;
10. estado atual versus arquitetura-alvo;
11. decisões e antiobjetivos explícitos.

## Documento de roadmap

Criar `docs/roadmap-cms.md` em português. O roadmap deve priorizar resultados
para clientes e usar fases com critérios de aceite verificáveis, dependências e
indicadores. As fases são:

1. fundação de governança e catálogo canônico;
2. recuperação e segurança editorial;
3. distribuição e descoberta de conteúdo;
4. multilinguismo;
5. colaboração editorial;
6. extensibilidade governada;
7. portais e conteúdo restrito, condicionado a demanda comprovada.

Cada item deve indicar estado (`existente`, `lacuna`, `alvo`), benefício ao
cliente, entregáveis, critérios de aceite e dependências. O roadmap não deve
inventar datas; deve usar ordem e gates de saída. Deve distinguir prioridade
obrigatória de capacidade opcional condicionada a cliente real.

## Navegação

O `README.md` deve apontar para os dois documentos numa seção de documentação,
sem reescrever seu conteúdo.

## Verificação

- Markdown sem links locais quebrados.
- Diagramas Mermaid sintaticamente coerentes.
- Nomes de crates, rotas, integrações e responsabilidades conferidos no código.
- Ausência de segredos e identificadores de clientes.
- `cargo fmt --check` continua passando; a entrega documental não altera Rust.

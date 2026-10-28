# Contratos OAuth do CMS

Responsável: Codex. Referência: AVL-343.

O catálogo OAuth descreve as 12 operações de descoberta, registro, autorização
e troca/renovação de tokens para MCP e runtime móvel. O despacho do painel usa
esse catálogo. DTOs compartilhados são usados pela serialização real e pela
exportação Rust, com Schemars na feature `contracts` já existente.

## Transporte e segurança

Cada operação declara query, formulário ou JSON, credencial, resposta,
cabeçalhos e status de sucesso. Pedidos form/query representam os campos de
transporte aceitos pelo parser; registro do retorno, PKCE S256, participação,
consentimento e consumo/rotação continuam verificados pelo backend.

O registro dinâmico pertence ao MCP. O runtime usa clientes previamente
cadastrados e `runtime:read`. Os tokens não passam a ser intercambiáveis.
As telas mantêm a checagem de origem antes do consentimento e o login antes
de exibir ou autorizar. As operações do conector mantêm sua auditoria anterior.

O schema de uma tela descreve apenas sua representação textual. Os testes de
integração verificam o conteúdo e a navegação originais; o artefato descarta o
HTML e os valores de `Location`. Os testes de troca/renovação usam o token real
em memória. O artefato suprime tokens, códigos, verificadores, state e retornos
registrados antes de gravar, preservando os tipos para detectar valor nulo,
objeto ou número onde o contrato exige string. Não comprova o valor secreto.

O coletor lê apenas `grant_type` na requisição e persiste somente os valores
permitidos `authorization_code` ou `refresh_token`, para declarar a cobertura.
Nunca grava o formulário ou os demais campos. Cada captura exige o commit,
run e tentativa atuais, sem reutilizar evidências do cache.

## Verificação

O workflow existente exporta `oauth-v1.json` junto de `apps-v1.json`. O gate
compara o baseline OAuth do commit base com a revisão testada, valida exemplos
e executa um consumidor independente sobre as respostas sanitizadas. Todas
as 12 operações precisam de sucesso exercitado. As duas concessões precisam
de sucesso e recusa para cada recurso; replay é conferido pelos testes Rust.

`cms-contracts` inclui `oauth-compatibility.json`, baseline, contrato e capturas
sanitizadas. A primeira versão declara `initialContract`; não presume histórico.
O baseline versionado do candidato deve coincidir com a exportação atual.

Código e schema não certificam operação em produção. O módulo seguinte cobre
JSON-RPC MCP e suas ferramentas; esta entrega não declara esses contratos
concluídos, nem encerra o objetivo geral.

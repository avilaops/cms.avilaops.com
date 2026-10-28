# Contratos MCP do CMS

O catálogo `FERRAMENTAS` em `crates/web/src/conector.rs` define as 37 ferramentas,
escopos, argumentos e descrições. O servidor e o exportador usam esse catálogo.
Os DTOs de `mcp_types.rs` são serializados pelo servidor e geram os schemas de
respostas JSON-RPC, descritores, inicialização e resultados de ferramentas.

## Verificação em implementação

O workflow existente exporta `mcp-v1.json` da revisão testada, executa o consumidor
JavaScript independente e conserva capturas sintéticas identificadas por commit,
execução e tentativa. A coleta recebe método, parâmetros e resposta da fixture;
não recebe cabeçalhos, cookies ou credenciais OAuth. Nunca reutilizar esse coletor
em produção. `target/contracts` é limpo antes de cada exportação.

O gate compara escopos, anotações, schemas de entrada e transporte com o baseline
do commit base. Não lê a cópia editável do candidato como histórico. Confere a
lista real contra o catálogo exportado, correlação da resposta, exclusividade de
result/error e JSON textual nas chamadas de sucesso. Entradas de sucesso são
validadas contra o inputSchema anunciado. Recusas e erros de ferramenta podem
ter entradas inválidas deliberadamente; não são exemplos de entrada válida.

## Aceite ainda aberto

O transporte textual não define o payload interno de uma ferramenta. Por isso
o relatório mantém `payloadsVerified=false` e `moduleComplete=false` enquanto
faltarem contratos e consumidor para cada payload, sucesso de todas as ferramentas
e verificação de autorização/participação pertinente. O teste de falta de escopo
das 37 ferramentas prova a recusa antes do efeito nesse cenário; não substitui
os testes de participação e política editorial. O módulo não está concluído.

Plano: [execução MCP](../superpowers/plans/2026-10-28-mcp-contracts.md).
Referência: [AVL-343](https://linear.app/avilaops/issue/AVL-343).

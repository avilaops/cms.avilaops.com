# Contratos executáveis MCP do CMS

Responsável: Codex. Referência: AVL-343. Continuação do objetivo de concluir cada módulo.

## Resultado exigido

Exportar da revisão testada os contratos JSON-RPC e das 37 ferramentas reais,
comparar com baseline Git imutável e validar consumidores e respostas do
roteador com Postgres. A evidência deve distinguir transporte, payload interno
e autorização editorial. Não concluir pelo snapshot do catálogo.

## Execução

- [ ] Compartilhar tipos de respostas JSON-RPC e descritores com o servidor.
- [ ] Exportar catálogo, escopos, anotações e entradas reais das 37 ferramentas.
- [ ] Validar transporte de initialize, ping, tools/list, tools/call e erros.
- [ ] Validar exemplos de entrada e consumidor independente.
- [ ] Exercitar sucesso de cada ferramenta e recusa por escopo/participação.
- [ ] Contratar os payloads JSON dentro dos resultados textuais, incluindo lote parcial.
- [ ] Comparar entradas, saídas, escopos e anotações com baseline histórico.
- [ ] Vincular capturas ao commit/execução e conferir artefatos na CI.
- [ ] Registrar baseline, evidência e limites no Linear e controle da equipe.

## Regras de implementação

O catálogo existente continua a definir ferramentas disponíveis e argumentos.
Tipos compartilhados definem a serialização real dos envelopes. JSON dentro de
content.text exige uma segunda validação; schema string não comprova seu conteúdo.
Testes usam contas, sites e mídia sintéticos em banco isolado. Credenciais não
entram nos artefatos. Não alterar o fluxo editorial para satisfazer um snapshot.
Builds e testes permanecem no workflow GitHub existente. Sem merge ou deploy.

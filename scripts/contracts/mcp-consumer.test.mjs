import assert from 'node:assert/strict';
import test from 'node:test';
import { compareMcp, consumeMcpInput, consumeMcpResponse } from './mcp-consumer.mjs';

const catalog = () => ({ schemaVersion: 1, module: 'cms-mcp-v1', protocolVersion: '2025-06-18',
  path: '/mcp', method: 'POST', authentication: 'oauth_mcp',
  tools: Array.from({ length: 37 }, (_, index) => ({ scope: 'conteudo:ler', descriptor: {
    name: `ver_${index}`, description: 'Consulta de teste', annotations: { readOnlyHint: true, destructiveHint: false },
    inputSchema: { type: 'object', properties: { site: { type: 'string' } }, required: ['site'] },
  } })), transport: { toolResult: { type: 'object' }, error: { type: 'object' } } });

test('MCP recusa remoção, restrição de entrada, troca de escopo e anotações perigosas', () => {
  const baseline = catalog();
  assert.equal(compareMcp(baseline, catalog()).compatible, true);
  for (const mutate of [
    value => value.tools.pop(),
    value => { value.tools[0].scope = 'conteudo:publicar'; },
    value => { value.tools[0].descriptor.annotations.destructiveHint = true; },
    value => { value.tools[0].descriptor.inputSchema.properties.site.enum = ['unico']; },
    value => { value.protocolVersion = 'desconhecida'; },
    value => { value.transport.toolResult = { type: 'string' }; },
  ]) {
    const candidate = structuredClone(baseline);
    mutate(candidate);
    assert.equal(compareMcp(baseline, candidate).compatible, false);
  }
});

test('lote parcial exige recusa individual de cada documento inválido', () => {
  const descriptor = { name: 'criar_rascunhos', inputSchema: {
    type: 'object', required: ['site', 'conteudos'], properties: {
      site: { type: 'string' },
      conteudos: { type: 'array', minItems: 1, maxItems: 25,
        items: { type: 'object', required: ['dados'], properties: { dados: { type: 'object' } } } },
    },
  } };
  const args = { site: 'padaria', conteudos: [{ dados: {} }, {}] };
  const outcomes = [{ indice: 0, documento: 'id', problemas: [] }, { indice: 1, erro: 'Contrato inválido' }];
  const response = (items) => ({ content: [{ text: JSON.stringify({ rascunhos: items }) }] });
  consumeMcpInput(descriptor, args, response(outcomes));
  for (const invalid of [
    [outcomes[0], { indice: 1, documento: 'salvo' }],
    [outcomes[0], { indice: 1, erro: 'erro', documento: 'salvo' }],
    [outcomes[0], { indice: 1, erro: '' }],
    [outcomes[0]],
    [outcomes[1], outcomes[0]],
  ]) assert.throws(() => consumeMcpInput(descriptor, args, response(invalid)));
  assert.throws(() => consumeMcpInput(descriptor, { ...args, site: 42 }, response(outcomes)));
  assert.throws(() => consumeMcpInput(descriptor, { ...args, conteudos: [] }, response([])));
  assert.throws(() => consumeMcpInput(descriptor, { ...args, conteudos: Array(26).fill({}) }, response([])));
  assert.throws(() => consumeMcpInput({ ...descriptor, name: 'criar_rascunho' }, args, response(outcomes)));
});

test('consumidor MCP confere correlação, exclusividade e JSON textual de sucesso', () => {
  const contract = catalog();
  const response = { jsonrpc: '2.0', id: 'pedido', result: { content: [{ type: 'text', text: '{"sites":[]}' }], isError: false } };
  consumeMcpResponse(contract, 'tools/call', response, 'pedido');
  assert.throws(() => consumeMcpResponse(contract, 'tools/call', response, 'outro'));
  assert.throws(() => consumeMcpResponse(contract, 'tools/call', { ...response, error: {} }, 'pedido'));
  response.result.content[0].text = 'conteúdo que não é JSON';
  assert.throws(() => consumeMcpResponse(contract, 'tools/call', response, 'pedido'));
  response.result.isError = true;
  consumeMcpResponse(contract, 'tools/call', response, 'pedido');
});

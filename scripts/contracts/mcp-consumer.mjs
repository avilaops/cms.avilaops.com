import assert from 'node:assert/strict';
import { isSubset, validator } from './compatibility.mjs';

export function consumeMcpCatalog(contract) {
  assert.equal(contract.module, 'cms-mcp-v1');
  assert.equal(contract.schemaVersion, 1);
  assert.equal(contract.path, '/mcp');
  assert.equal(contract.method, 'POST');
  assert.equal(contract.authentication, 'oauth_mcp');
  assert.equal(contract.protocolVersion, '2025-06-18');
  assert.equal(contract.tools.length, 37);
  assert.equal(new Set(contract.tools.map(tool => tool.descriptor.name)).size, 37);
  const ajv = validator();
  for (const { descriptor, scope } of contract.tools) {
    assert.match(descriptor.name, /^[a-z_0-9]+$/);
    assert.equal(typeof descriptor.description, 'string');
    assert.ok(descriptor.description.length > 0);
    assert.match(scope, /^[a-z]+:[a-z]+$/);
    assert.equal(typeof descriptor.annotations.readOnlyHint, 'boolean');
    assert.equal(typeof descriptor.annotations.destructiveHint, 'boolean');
    assert.equal(descriptor.inputSchema.type, 'object');
    ajv.compile(descriptor.inputSchema);
  }
  for (const schema of Object.values(contract.transport)) ajv.compile(schema);
  return contract.tools.map(tool => tool.descriptor.name).sort();
}

export function compareMcp(baseline, candidate) {
  const differences = [];
  try { consumeMcpCatalog(candidate); } catch { differences.push('Catálogo MCP inválido.'); }
  if (baseline) {
    for (const field of ['schemaVersion', 'module', 'protocolVersion', 'path', 'method', 'authentication']) {
      if (candidate[field] !== baseline[field]) differences.push(`MCP: ${field} alterado.`);
    }
    for (const previous of baseline.tools) {
      const name = previous.descriptor.name;
      const current = candidate.tools.find(tool => tool.descriptor.name === name);
      if (!current) { differences.push(`Ferramenta removida: ${name}.`); continue; }
      if (previous.scope !== current.scope) differences.push(`${name}: escopo alterado.`);
      for (const hint of ['readOnlyHint', 'destructiveHint']) {
        if (previous.descriptor.annotations[hint] !== current.descriptor.annotations[hint]) differences.push(`${name}: ${hint} alterado.`);
      }
      if (!isSubset(previous.descriptor.inputSchema, current.descriptor.inputSchema)) differences.push(`${name}: entrada incompatível.`);
    }
    for (const [name, schema] of Object.entries(baseline.transport)) {
      if (!candidate.transport[name] || !isSubset(candidate.transport[name], schema)) differences.push(`Transporte incompatível: ${name}.`);
    }
  }
  return { compatible: differences.length === 0, initialContract: baseline === null, differences };
}

export function consumeMcpResponse(contract, method, response, requestId) {
  assert.equal(response.jsonrpc, '2.0');
  assert.deepEqual(response.id, requestId);
  assert.notEqual(Object.hasOwn(response, 'result'), Object.hasOwn(response, 'error'));
  const ajv = validator();
  if (Object.hasOwn(response, 'error')) {
    assert.ok(ajv.compile(contract.transport.error)(response));
    assert.ok([-32700, -32601].includes(response.error.code));
    return;
  }
  if (method === 'tools/list') {
    assert.ok(ajv.compile(contract.transport.toolsList)(response));
    const expected = contract.tools.map(tool => tool.descriptor);
    assert.deepEqual(response.result.tools, expected);
  } else if (method === 'tools/call') {
    assert.ok(ajv.compile(contract.transport.toolResult)(response));
    assert.equal(response.result.content.length, 1);
    assert.equal(response.result.content[0].type, 'text');
    if (!response.result.isError) JSON.parse(response.result.content[0].text);
  } else if (method === 'ping') {
    assert.deepEqual(response.result, {});
  } else if (method === 'initialize') {
    assert.equal(response.result.protocolVersion, contract.protocolVersion);
    assert.equal(response.result.serverInfo.name, 'cms-avila-ops');
    assert.equal(typeof response.result.serverInfo.version, 'string');
    assert.deepEqual(response.result.capabilities, { tools: {} });
    assert.equal(typeof response.result.instructions, 'string');
  } else {
    assert.fail('Método MCP sem consumidor.');
  }
}

import assert from 'node:assert/strict';
import test from 'node:test';
import { compareContracts, isSubset } from './compatibility.mjs';

const object = (properties, required = [], additionalProperties = false) =>
  ({ type: 'object', properties, required, additionalProperties });
const string = { type: 'string' };
const fixture = () => ({ schemaVersion: 1, module: 'cms-apps-v1', limits: { definitionBytes: 524288 },
  aadValidation: true, operations: [{ id: 'createApp', method: 'POST', path: '/apps',
    authentication: 'avila_sso', requiredHeaders: ['origin'], responseHeaders: { 201: ['x-correlation-id'] },
    request: object({ name: string }, ['name']), responses: { 201: object({ id: string }, ['id']) } }] });

test('entrada pode ampliar valores aceitos; saída deve continuar aceita pelo consumidor antigo', () => {
  const old = fixture();
  const compatible = structuredClone(old);
  compatible.operations[0].request.properties.description = string;
  assert.equal(compareContracts(old, compatible).compatible, true);
  const required = structuredClone(compatible);
  required.operations[0].request.required.push('description');
  assert.equal(compareContracts(old, required).compatible, false);
  const removed = fixture();
  removed.operations[0].responses[201] = object({});
  assert.equal(compareContracts(old, removed).compatible, false);
});

test('recusa nova enumeração de saída, restrição de entrada e remoção de operação ou status', () => {
  const old = fixture();
  old.operations[0].responses[201].properties.role = { enum: ['owner', 'editor'] };
  old.operations[0].responses[201].required.push('role');
  for (const mutate of [
    value => { value.operations[0].responses[201].properties.role.enum.push('admin'); },
    value => { value.operations[0].request.properties.name.maxLength = 10; },
    value => { value.operations = []; },
    value => { delete value.operations[0].responses[201]; },
    value => { value.operations[0].requiredHeaders.push('new-header'); },
    value => { value.operations[0].responseHeaders[201] = []; },
  ]) {
    const candidate = structuredClone(old);
    mutate(candidate);
    assert.equal(compareContracts(old, candidate).compatible, false);
  }
});

test('tipos, campos desconhecidos, limites e formatos respeitam inclusão', () => {
  assert.equal(isSubset({ type: 'integer' }, { type: 'number' }), true);
  assert.equal(isSubset({ type: 'number' }, { type: 'integer' }), false);
  assert.equal(isSubset(object({ id: string }, ['id']), object({ id: string })), true);
  assert.equal(isSubset(object({ id: string }), object({ id: string }, ['id'])), false);
  assert.equal(isSubset(object({}, [], true), object({})), false);
  assert.equal(isSubset({ type: 'string', minLength: 3 }, { type: 'string', minLength: 2 }), true);
  assert.equal(isSubset({ type: 'array', items: string, maxItems: 4 }, { type: 'array', items: string, maxItems: 3 }), false);
  assert.equal(isSubset({ type: 'string' }, { type: 'string', format: 'uuid' }), false);
  assert.equal(isSubset({ type: 'string', pattern: '^a' }, { type: 'string', pattern: '^a' }), true);
  assert.equal(isSubset({ type: 'integer', maximum: 1e20 }, { type: 'integer', maximum: 1e20 }), false);
});

test('compara uniões discriminadas e referências locais; mudanças não suportadas exigem revisão', () => {
  const branch = tag => object({ type: { const: tag }, value: string }, ['type', 'value']);
  assert.equal(isSubset({ oneOf: [branch('text')] }, { oneOf: [branch('text'), branch('button')] }), true);
  assert.equal(isSubset({ oneOf: [branch('text'), branch('new')] }, { oneOf: [branch('text')] }), false);
  assert.equal(isSubset({ $defs: { name: string }, $ref: '#/$defs/name' }, string), true);
  assert.equal(isSubset({ type: 'object', dependentRequired: { a: ['b'] } }, { type: 'object', dependentRequired: { a: ['c'] } }), false);
  assert.equal(isSubset({ $ref: 'https://invalid.test/schema' }, string), false);
  assert.equal(isSubset({ oneOf: [string, { type: 'string', minLength: 3 }] }, string), false);
  const referenced = { $defs: { value: string }, allOf: [{ $ref: '#/$defs/value' }] };
  const changed = structuredClone(referenced);
  changed.$defs.value = { type: 'integer' };
  assert.equal(isSubset(referenced, changed), false);
});

test('baseline inicial é explícito, mudanças de credencial e limites são recusadas', () => {
  assert.equal(compareContracts(null, fixture()).initialContract, true);
  for (const mutate of [
    value => { value.limits.definitionBytes = 10; },
    value => { value.operations[0].authentication = 'public'; },
    value => { value.operations.push(structuredClone(value.operations[0])); },
  ]) {
    const candidate = fixture();
    mutate(candidate);
    assert.equal(compareContracts(fixture(), candidate).compatible, false);
  }
});

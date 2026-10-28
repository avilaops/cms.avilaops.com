import assert from 'node:assert/strict';
import test from 'node:test';
import { consumeOAuth } from './oauth-consumer.mjs';
import { compareContracts } from './compatibility.mjs';

test('consumidor OAuth recusa exposição e troca de recurso, preservando a separação móvel', () => {
  const headers = { 'content-type': 'application/json', 'cache-control': 'no-store' };
  const body = { access_token: '[REDACTED]', refresh_token: '[REDACTED]', token_type: 'Bearer', expires_in: 3600, scope: 'runtime:read' };
  consumeOAuth('runtimeToken', body, 200, headers);
  assert.throws(() => consumeOAuth('runtimeToken', { ...body, access_token: 'synthetic-secret' }, 200, headers), /suprimida/);
  assert.throws(() => consumeOAuth('runtimeToken', { ...body, scope: 'sites:ler' }, 200, headers));
  consumeOAuth('mcpConsent', null, 303, { location: '[REDACTED]' });
});

test('compatibilidade OAuth não depende do contrato AAD e detecta mudança de encoding', () => {
  const before = { schemaVersion: 1, module: 'cms-oauth-v1', operations: [{ id: 'token', method: 'POST', path: '/token', authentication: 'public_oauth_client', requestEncoding: 'form', redacted: true, successStatuses: ['200'], request: true, responses: { 200: true }, requiredHeaders: [], responseHeaders: {} }] };
  assert.equal(compareContracts(before, structuredClone(before)).compatible, true);
  const after = structuredClone(before); after.operations[0].requestEncoding = 'json';
  assert.equal(compareContracts(before, after).compatible, false);
});

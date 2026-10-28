import assert from 'node:assert/strict';

export function consumeOAuth(id, body, status, headers) {
  if ([302, 303].includes(status)) {
    if (headers.location !== '[REDACTED]') throw new Error('Redirect não foi suprimido.');
    assert.equal(body, null);
    return;
  }
  if (typeof body === 'string') {
    if (body !== '[CONTENT REDACTED]') throw new Error('Conteúdo de navegador não foi suprimido.');
    return;
  }
  assert.ok(body && typeof body === 'object');
  if (status >= 400) {
    assert.equal(typeof body.error, 'string');
    assert.equal(typeof body.error_description, 'string');
    return;
  }
  assert.ok(headers['content-type']?.includes('application/json'));
  assert.ok(headers['cache-control']?.includes('no-store'));
  if (id.endsWith('Token')) {
    if (body.access_token !== '[REDACTED]' || body.refresh_token !== '[REDACTED]') throw new Error('Credencial não foi suprimida.');
    assert.equal(body.token_type, 'Bearer');
    assert.ok(Number.isSafeInteger(body.expires_in) && body.expires_in > 0);
    assert.equal(typeof body.scope, 'string');
    if (id === 'runtimeToken') assert.equal(body.scope, 'runtime:read');
  } else if (id.endsWith('Metadata')) {
    const native = id === 'runtimeMetadata';
    assert.ok(body.authorization_endpoint.endsWith(native ? '/oauth/runtime/authorize' : '/oauth/authorize'));
    assert.ok(body.token_endpoint.endsWith(native ? '/oauth/runtime/token' : '/oauth/token'));
    assert.deepEqual(body.code_challenge_methods_supported, ['S256']);
    assert.deepEqual(body.token_endpoint_auth_methods_supported, ['none']);
    assert.deepEqual(body.response_types_supported, ['code']);
    assert.deepEqual(body.grant_types_supported, ['authorization_code', 'refresh_token']);
    if (native) { assert.ok(!Object.hasOwn(body, 'registration_endpoint')); assert.deepEqual(body.scopes_supported, ['runtime:read']); }
    else assert.ok(body.registration_endpoint.endsWith('/oauth/register'));
  } else if (id === 'register') {
    assert.equal(typeof body.client_id, 'string');
    assert.ok(Array.isArray(body.redirect_uris) && body.redirect_uris.length > 0);
    if (body.redirect_uris.some(uri => uri !== '[REDACTED]')) throw new Error('Retorno registrado não foi suprimido.');
    assert.equal(body.token_endpoint_auth_method, 'none');
  } else {
    assert.ok(body.resource.endsWith(id === 'runtimeResource' ? '/api/runtime' : '/mcp'));
    assert.ok(Array.isArray(body.authorization_servers) && body.authorization_servers.length > 0);
    if (id === 'runtimeResource') assert.deepEqual(body.scopes_supported, ['runtime:read']);
  }
}

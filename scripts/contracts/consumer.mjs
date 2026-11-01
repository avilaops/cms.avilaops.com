// Consumidor independente dos DTOs Rust: interpreta os campos utilizados pelo painel/runtime.
import assert from 'node:assert/strict';

function fields(value, specification) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), 'objeto esperado');
  for (const [key, type] of Object.entries(specification)) {
    if (type === 'integer') assert.ok(Number.isSafeInteger(value[key]), `${key}: inteiro seguro esperado`);
    else assert.equal(typeof value[key], type, `${key}: tipo divergente`);
  }
  return value;
}

function app(value) {
  fields(value, { id: 'string', slug: 'string', name: 'string', bundleId: 'string', draftRevision: 'integer', role: 'string' });
  assert.ok(['owner', 'editor', 'viewer'].includes(value.role), 'papel desconhecido');
  return value;
}

function definition(value) {
  fields(value, { schemaVersion: 'string', appId: 'string', minimumRuntimeVersion: 'string', navigation: 'object' });
  fields(value.navigation, { initialRoute: 'string' });
  assert.ok(Array.isArray(value.screens), 'telas esperadas');
  for (const screen of value.screens) {
    fields(screen, { id: 'string', route: 'string', title: 'string' });
    assert.ok(Array.isArray(screen.components), 'componentes esperados');
    for (const component of screen.components) {
      fields(component, { id: 'string', type: 'string' });
      assert.ok(['text', 'button', 'spacer'].includes(component.type), 'componente desconhecido');
      if (component.type === 'text') fields(component, { value: 'string' });
      if (component.type === 'button') {
        fields(component, { label: 'string', action: 'object' });
        fields(component.action, { type: 'string', target: 'string' });
        assert.equal(component.action.type, 'navigate');
      }
    }
  }
}

export function consume(operation, body, status = 200) {
  if (status === 304) { assert.equal(body, null); return; }
  if (status >= 400) {
    if (body.valid === false) {
      assert.ok(Array.isArray(body.errors) && body.errors.length > 0);
      for (const error of body.errors) fields(error, { code: 'string', message: 'string' });
    } else fields(body.error, { code: 'string', message: 'string' });
    return;
  }
  switch (operation) {
    case 'listApps': assert.ok(Array.isArray(body)); body.forEach(app); break;
    case 'getApp': case 'createApp': app(body); break;
    case 'getDraft': fields(body, { draftRevision: 'integer' }); assert.ok(Object.hasOwn(body, 'definition')); break;
    case 'saveDraft': fields(body, { draftRevision: 'integer' }); break;
    case 'validateDraft': fields(body, { valid: 'boolean', draftRevision: 'integer' }); assert.equal(body.valid, true); assert.deepEqual(body.errors, []); break;
    case 'createVersion': fields(body, { id: 'string', number: 'integer' }); break;
    case 'listVersions':
      assert.ok(Array.isArray(body));
      body.forEach(item => fields(item, { id: 'string', number: 'integer', schemaVersion: 'string', minimumRuntimeVersion: 'string', createdBy: 'string', createdAt: 'string' })); break;
    case 'getVersion': fields(body, { number: 'integer' }); definition(body.definition); break;
    case 'listEnvironments':
      assert.ok(Array.isArray(body));
      body.forEach(item => fields(item, { name: 'string', versionId: 'string', version: 'integer', updatedBy: 'string', updatedAt: 'string' })); break;
    case 'publishVersion': fields(body, { environment: 'string', version: 'integer', versionId: 'string' }); break;
    case 'listMembers':
      assert.ok(Array.isArray(body));
      body.forEach(item => fields(item, { account: 'string', role: 'string', createdAt: 'string' })); break;
    case 'putMember': case 'deleteMember': fields(body, { updated: 'boolean' }); assert.equal(body.updated, true); break;
    case 'listAudit':
      assert.ok(Array.isArray(body));
      body.forEach(item => {
        fields(item, { id: 'integer', account: 'string', action: 'string', correlationId: 'string', createdAt: 'string' });
        assert.ok(Object.hasOwn(item, 'data'));
      }); break;
    case 'getRuntime': fields(body, { appId: 'string', environment: 'string', version: 'integer' }); definition(body.definition); break;
    default: throw new Error(`Consumidor ausente: ${operation}`);
  }
}

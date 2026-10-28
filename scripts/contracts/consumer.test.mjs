import assert from 'node:assert/strict';
import test from 'node:test';
import { consume } from './consumer.mjs';

const app = { id: 'id', slug: 'app', name: 'App', bundleId: 'com.example.app', draftRevision: 2, role: 'owner' };
test('consumidor interpreta respostas e rejeita remoção ou troca de tipo', () => {
  consume('createApp', app, 201);
  consume('listApps', [app]);
  assert.throws(() => consume('createApp', { ...app, draftRevision: '2' }));
  assert.throws(() => consume('createApp', { ...app, role: 'admin' }));
  assert.throws(() => consume('saveDraft', {}));
  consume('getRuntime', null, 304);
  consume('createApp', { error: { code: 'invalid_app', message: 'Inválido.' } }, 422);
});

test('consumidor móvel recusa componente desconhecido e navegação incompleta', () => {
  const body = { appId: 'app', environment: 'production', version: 1, definition: {
    schemaVersion: '1.0.0', appId: 'app', minimumRuntimeVersion: '1.0.0',
    navigation: { initialRoute: 'home' }, screens: [{ id: 'home', route: 'home', title: 'Início', components: [] }],
  } };
  consume('getRuntime', body);
  body.definition.screens[0].components.push({ id: 'remote', type: 'remote-code' });
  assert.throws(() => consume('getRuntime', body));
});

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { validarInventario } from './validar-mlops.mjs';

const revision = 'a'.repeat(40);
const prefix = `https://github.com/avilaops/cms.avilaops.com/blob/${revision}/`;
const bytes = Buffer.from('fn autor_nao_publica() {}\n');
const sha256 = createHash('sha256').update(bytes).digest('hex');

function fixture() {
  const policies = [{
    id: 'CMS-ACCESS-002', product: 'cms', title: 'Autorização editorial',
    category: 'access', state: 'implemented', rule: 'Autor não publica.',
    owner: 'Codex', enforcement: 'src/fluxo.rs', verificationDate: '',
    sources: [{ kind: 'code', location: `${prefix}src/fluxo.rs`, note: 'Inspeção.' },
      { kind: 'test', location: `${prefix}tests/fluxo.rs`, note: 'Teste existente.' }],
  }];
  const manifest = {
    schemaVersion: 1, module: 'cms', sourceRevision: revision,
    sources: ['src/fluxo.rs', 'tests/fluxo.rs'].map(path => ({ path, sha256 })),
    checks: [{ policyId: policies[0].id, path: 'tests/fluxo.rs', selectors: ['autor_nao_publica'] }],
  };
  return { policies, manifest, readPinned: () => bytes, readCurrent: () => bytes };
}

test('integridade e vínculo de testes não promovem fichas a verificadas', () => {
  const result = validarInventario(fixture());
  assert.equal(result.policies, 1);
  assert.equal(result.sourceIntegrityVerified, true);
  assert.equal(result.productRuntimeVerified, false);
  assert.equal(result.checks[0].policyId, 'CMS-ACCESS-002');
});

test('recusa duplicação e referências sem cobertura', () => {
  const duplicate = fixture();
  duplicate.policies.push(structuredClone(duplicate.policies[0]));
  assert.throws(() => validarInventario(duplicate), /duplicad/);
  const uncovered = fixture();
  uncovered.manifest.sources.pop();
  assert.throws(() => validarInventario(uncovered), /cobertura/);
});

test('recusa bytes divergentes tanto na revisão fixada quanto no checkout', () => {
  for (const reader of ['readPinned', 'readCurrent']) {
    const value = fixture();
    value[reader] = () => Buffer.from('alterado');
    assert.throws(() => validarInventario(value), /SHA-256/);
  }
});

test('recusa caminho externo, revisão inválida e fontes extras', () => {
  for (const path of ['../fora.rs', '/fora.rs', 'src/../fora.rs', 'src\\fora.rs']) {
    const value = fixture();
    value.manifest.sources[0].path = path;
    assert.throws(() => validarInventario(value), /caminho/);
  }
  const value = fixture();
  value.manifest.sourceRevision = 'main';
  assert.throws(() => validarInventario(value), /revisão/);
  const extra = fixture();
  extra.manifest.sources.push({ path: 'extra.rs', sha256 });
  assert.throws(() => validarInventario(extra), /cobertura/);
});

test('recusa seletor ausente ou teste sem referência na ficha', () => {
  const missing = fixture();
  missing.manifest.checks[0].selectors = ['nao_existe'];
  assert.throws(() => validarInventario(missing), /seletor/);
  const unlinked = fixture();
  unlinked.manifest.checks[0].path = 'src/fluxo.rs';
  assert.throws(() => validarInventario(unlinked), /teste/);
  const noChecks = fixture();
  noChecks.manifest.checks = [];
  assert.throws(() => validarInventario(noChecks), /teste/);
});

test('recusa promoção automática, data ou responsável ausente', () => {
  for (const changes of [{ state: 'verified' }, { verificationDate: '2026-10-28' },
    { owner: '' }, { product: 'erp' }]) {
    const value = fixture();
    Object.assign(value.policies[0], changes);
    assert.throws(() => validarInventario(value));
  }
});

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { compareContracts, validator } from './compatibility.mjs';
import { consume } from './consumer.mjs';
import { baselinePath, loadBaseline } from './baseline.mjs';

const directory = resolve('target/contracts');
const candidate = JSON.parse(await readFile(resolve(directory, 'apps-v1.json'), 'utf8'));
const base = process.env.BASE_REVISION;
const git = (...args) => execFileSync('git', args, { stdio: ['ignore', 'pipe', 'pipe'] }).toString().trim();
const baseline = loadBaseline(base);
await writeFile(resolve(directory, 'baseline.json'), JSON.stringify(baseline, null, 2));
const report = { ...compareContracts(baseline, candidate), baselineRevision: base,
  executionRevision: git('rev-parse', 'HEAD'),
  contractSha256: createHash('sha256').update(JSON.stringify(candidate)).digest('hex'),
  operationCount: candidate.operations.length, responsesChecked: 0, successfulOperations: [],
  policies: candidate.policies, tests: candidate.tests, routerWithPostgresVerified: false };
try {
  assert.equal(report.compatible, true, report.differences.join('\n'));
  const ajv = validator();
  const operations = new Map();
  for (const operation of candidate.operations) {
    assert.ok(!operations.has(operation.id), 'código duplicado');
    operations.set(operation.id, operation);
    if (operation.request !== null) ajv.compile(operation.request);
    for (const schema of Object.values(operation.parameters ?? {})) ajv.compile(schema);
    for (const schema of Object.values(operation.responses)) if (schema !== null) ajv.compile(schema);
  }
  const examples = JSON.parse(await readFile('docs/contracts/examples.json', 'utf8'));
  const aad = ajv.compile(candidate.aadValidation);
  assert.equal(aad(examples.aad), true, 'exemplo AAD incompatível');
  for (const [id, body] of Object.entries(examples.requests)) {
    assert.equal(ajv.compile(operations.get(id).request)(body), true, `exemplo de entrada: ${id}`);
  }
  const runtime = candidate.operations.find(op => op.id === 'getRuntime');
  assert.equal(ajv.compile(runtime.responses[200])({ appId: 'example-app', environment: 'production', version: 1, definition: examples.aad }), true);
  const successes = new Set();
  const files = await readdir(resolve(directory, 'responses'));
  assert.ok(files.length > 0, 'respostas reais da suíte ausentes');
  for (const file of files) {
    const record = JSON.parse(await readFile(resolve(directory, 'responses', file), 'utf8'));
    const operation = candidate.operations.find(op => op.method === record.method &&
      op.path.split('/').length === record.path.split('/').length &&
      op.path.split('/').every((part, index) => part.startsWith('{') || part === record.path.split('/')[index]));
    if (!operation) {
      assert.ok([404, 405].includes(record.status), 'resposta de operação desconhecida');
      continue;
    }
    assert.ok(Object.hasOwn(operation.responses, record.status), `${operation.id}: status não declarado ${record.status}`);
    const schema = operation.responses[record.status];
    if (schema === null) assert.equal(record.body, null);
    else {
      const validate = ajv.compile(schema);
      assert.equal(validate(record.body), true, `${operation.id}/${record.status}: ${JSON.stringify(validate.errors)}`);
    }
    for (const header of operation.responseHeaders[record.status]) assert.ok(record.headers[header], `${operation.id}: cabeçalho ${header}`);
    consume(operation.id, record.body, record.status);
    if (record.status >= 200 && record.status < 300) {
      successes.add(operation.id);
      for (const [index, part] of operation.path.split('/').entries()) {
        if (!part.startsWith('{')) continue;
        const name = part.slice(1, -1);
        const raw = record.path.split('/')[index];
        const value = name === 'number' ? Number(raw) : raw;
        assert.equal(ajv.compile(operation.parameters[name])(value), true, `${operation.id}: parâmetro ${name}`);
      }
    }
    report.responsesChecked++;
  }
  report.successfulOperations = [...successes].sort();
  for (const operation of candidate.operations) assert.ok(successes.has(operation.id), `operação sem resposta de sucesso exercitada: ${operation.id}`);
  // O baseline do próximo incremento deve representar exatamente o código exportado.
  // Na primeira execução, a ausência permite obter o artefato para versioná-lo.
  try {
    const stored = JSON.parse(await readFile(baselinePath, 'utf8'));
    assert.deepEqual(stored, candidate, 'baseline do candidato desatualizado');
  } catch (error) { if (error.code !== 'ENOENT') throw error; }
  report.routerWithPostgresVerified = true;
} catch (error) {
  report.compatible = false;
  report.differences.push(error.message);
  process.exitCode = 1;
} finally {
  await writeFile(resolve(directory, 'compatibility.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
}

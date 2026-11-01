import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { loadBaseline } from './baseline.mjs';
import { compareContracts, validator } from './compatibility.mjs';
import { assertCaptureIdentity } from './capture.mjs';
import { consumeOAuth } from './oauth-consumer.mjs';

const directory = resolve('target/contracts');
const path = 'docs/contracts/oauth-v1.baseline.json';
const contract = JSON.parse(await readFile(resolve(directory, 'oauth-v1.json'), 'utf8'));
const baseline = loadBaseline(process.env.BASE_REVISION, process.cwd(), path);
const revision = execFileSync('git', ['rev-parse', 'HEAD']).toString().trim();
const report = { ...compareContracts(baseline, contract), executionRevision: revision,
  baselineRevision: process.env.BASE_REVISION, runId: `${process.env.GITHUB_RUN_ID}/${process.env.GITHUB_RUN_ATTEMPT}`,
  contractSha256: createHash('sha256').update(JSON.stringify(contract)).digest('hex'),
  operationCount: contract.operations.length, responsesChecked: 0, successfulOperations: [], grants: {},
  valuesRedacted: true, routerWithPostgresVerified: false };
await writeFile(resolve(directory, 'oauth-baseline.json'), JSON.stringify(baseline, null, 2));
try {
  assert.equal(report.compatible, true, report.differences.join('\n'));
  const ajv = validator();
  const examples = JSON.parse(await readFile('docs/contracts/oauth-examples.json', 'utf8'));
  for (const operation of contract.operations) {
    if (operation.request !== null) ajv.compile(operation.request);
    for (const schema of Object.values(operation.responses)) if (schema !== null) ajv.compile(schema);
    if (examples.requests[operation.id]) assert.ok(ajv.compile(operation.request)(examples.requests[operation.id]), 'exemplo OAuth inválido');
  }
  const successes = new Set();
  for (const file of await readdir(resolve(directory, 'oauth-responses'))) {
    const record = JSON.parse(await readFile(resolve(directory, 'oauth-responses', file), 'utf8'));
    assertCaptureIdentity(record, revision, report.runId);
    assert.equal(record.contentRedacted, true);
    const operation = contract.operations.find(op => op.method === record.method && op.path === record.path);
    if (!operation) { assert.ok([404, 405].includes(record.status)); continue; }
    assert.ok(Object.hasOwn(operation.responses, record.status), `${operation.id}: status ${record.status} não declarado`);
    const schema = operation.responses[record.status];
    if (schema === null) assert.equal(record.body, null);
    else { const validate = ajv.compile(schema); assert.ok(validate(record.body), `${operation.id}: resposta fora do schema`); }
    for (const header of operation.responseHeaders[record.status]) assert.ok(record.headers[header], `${operation.id}: cabeçalho ${header}`);
    consumeOAuth(operation.id, record.body, record.status, record.headers);
    if (operation.successStatuses.includes(String(record.status))) successes.add(operation.id);
    if (operation.id.endsWith('Token') && record.grantType) {
      assert.ok(['authorization_code', 'refresh_token'].includes(record.grantType));
      report.grants[operation.id] ??= {};
      report.grants[operation.id][record.grantType] ??= [];
      if (!report.grants[operation.id][record.grantType].includes(record.status)) report.grants[operation.id][record.grantType].push(record.status);
    }
    report.responsesChecked++;
  }
  report.successfulOperations = [...successes].sort();
  for (const operation of contract.operations) assert.ok(successes.has(operation.id), `OAuth sem sucesso exercitado: ${operation.id}`);
  for (const id of ['mcpToken', 'runtimeToken']) for (const grant of ['authorization_code', 'refresh_token']) {
    assert.ok(report.grants[id]?.[grant]?.includes(200), `${id}/${grant}: sucesso ausente`);
    assert.ok(report.grants[id]?.[grant]?.includes(400), `${id}/${grant}: recusa ausente`);
  }
  try { assert.deepEqual(JSON.parse(await readFile(path, 'utf8')), contract, 'baseline OAuth desatualizado'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  report.routerWithPostgresVerified = true;
} catch (error) { report.compatible = false; report.differences.push(error.message); process.exitCode = 1; }
finally {
  await writeFile(resolve(directory, 'oauth-compatibility.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
}

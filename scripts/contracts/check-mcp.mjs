import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { loadBaseline } from './baseline.mjs';
import { assertCaptureIdentity } from './capture.mjs';
import { compareMcp, consumeMcpCatalog, consumeMcpResponse } from './mcp-consumer.mjs';
import { validator } from './compatibility.mjs';

const directory = 'target/contracts';
const contract = JSON.parse(await readFile(`${directory}/mcp-v1.json`, 'utf8'));
const baseline = loadBaseline(process.env.BASE_REVISION, process.cwd(), 'docs/contracts/mcp-v1.baseline.json');
const revision = execFileSync('git', ['rev-parse', 'HEAD']).toString().trim();
const report = { ...compareMcp(baseline, contract), executionRevision: revision,
  baselineRevision: process.env.BASE_REVISION, runId: `${process.env.GITHUB_RUN_ID}/${process.env.GITHUB_RUN_ATTEMPT}`,
  contractSha256: createHash('sha256').update(JSON.stringify(contract)).digest('hex'),
  responsesChecked: 0, successfulTools: [], refusedTools: [], transportMethods: [],
  payloadsVerified: false, moduleComplete: false };
await writeFile(`${directory}/mcp-baseline.json`, JSON.stringify(baseline, null, 2));
try {
  assert.equal(report.compatible, true, report.differences.join('\n'));
  const names = consumeMcpCatalog(contract);
  const ajv = validator();
  const successes = new Set(), refusals = new Set(), methods = new Set();
  for (const file of await readdir(`${directory}/mcp-responses`)) {
    const capture = JSON.parse(await readFile(`${directory}/mcp-responses/${file}`, 'utf8'));
    assertCaptureIdentity(capture, revision, report.runId);
    consumeMcpResponse(contract, capture.method, capture.response, capture.requestId);
    methods.add(capture.method);
    if (capture.method === 'tools/call' && names.includes(capture.params.name)) {
      (capture.response.result.isError ? refusals : successes).add(capture.params.name);
      if (!capture.response.result.isError) {
        const tool = contract.tools.find(tool => tool.descriptor.name === capture.params.name);
        assert.ok(ajv.compile(tool.descriptor.inputSchema)(capture.params.arguments), `${capture.params.name}: entrada real fora do schema anunciado`);
      }
    }
    report.responsesChecked++;
  }
  report.successfulTools = [...successes].sort();
  report.refusedTools = [...refusals].sort();
  report.transportMethods = [...methods].sort();
  for (const name of names) assert.ok(refusals.has(name), `Recusa não exercitada: ${name}`);
  for (const method of ['initialize', 'ping', 'tools/list', 'tools/call', 'metodo/inexistente']) assert.ok(methods.has(method), `Método não exercitado: ${method}`);
  // Este primeiro incremento prova catálogo, transporte e recusas. A entrega
  // do módulo exige também payloads e sucesso de todas as ferramentas.
} catch (error) { report.compatible = false; report.differences.push(error.message); process.exitCode = 1; }
finally {
  await writeFile(`${directory}/mcp-compatibility.json`, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
}

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repository = 'https://github.com/avilaops/cms.avilaops.com';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

function exigir(condition, message) {
  if (!condition) throw new Error(message);
}

function caminhoSeguro(path) {
  exigir(typeof path === 'string' && /^[a-zA-Z0-9_./-]+$/.test(path)
    && !path.startsWith('/') && path.split('/').every(part => part && part !== '.' && part !== '..'),
  'Manifesto contém caminho inválido.');
}

/** Verifica integridade e referências; não executa ou certifica regras do produto. */
export function validarInventario({ policies, manifest, readPinned, readCurrent }) {
  exigir(manifest.schemaVersion === 1 && manifest.module === 'cms', 'Manifesto inválido.');
  exigir(/^[a-f0-9]{40}$/.test(manifest.sourceRevision), 'Manifesto exige revisão Git completa.');
  exigir(Array.isArray(policies) && policies.length > 0 && policies.length <= 500, 'Catálogo inválido.');
  exigir(Array.isArray(manifest.sources) && Array.isArray(manifest.checks), 'Manifesto inválido.');
  const prefix = `${repository}/blob/${manifest.sourceRevision}/`;
  const sources = new Map();
  for (const source of manifest.sources) {
    caminhoSeguro(source.path);
    exigir(/^[a-f0-9]{64}$/.test(source.sha256), 'SHA-256 inválido no manifesto.');
    exigir(!sources.has(prefix + source.path), 'Fonte duplicada.');
    sources.set(prefix + source.path, source);
  }
  const ids = new Set();
  const used = new Set();
  const testReferences = new Map();
  for (const policy of policies) {
    const fields = ['id', 'product', 'title', 'category', 'state', 'rule', 'owner',
      'enforcement', 'verificationDate', 'sources'];
    exigir(Object.keys(policy).length === fields.length && fields.every(field => field in policy),
      'Campos divergentes do contrato PolicyDraft.');
    exigir(typeof policy.id === 'string' && /^CMS-[A-Z]+-\d{3}$/.test(policy.id), 'Código inválido.');
    exigir(!ids.has(policy.id), 'Política duplicada.');
    ids.add(policy.id);
    exigir(policy.product === 'cms' && policy.state === 'implemented' && policy.verificationDate === '',
      'Este incremento registra somente implementação inspecionada, sem promoção a verificada.');
    exigir(['access', 'commercial', 'data', 'execution', 'publication', 'integration'].includes(policy.category),
      'Categoria inválida.');
    for (const [field, maximum] of Object.entries({ title: 180, rule: 8000, owner: 160, enforcement: 4000 })) {
      exigir(typeof policy[field] === 'string' && policy[field].trim() && policy[field].length <= maximum,
        `Campo obrigatório ou limite do contrato: ${field}.`);
    }
    exigir(Array.isArray(policy.sources) && policy.sources.length <= 20
      && policy.sources.some(source => source.kind === 'code'),
      'Ficha exige referência de código.');
    const tests = new Set();
    for (const reference of policy.sources) {
      exigir(Object.keys(reference).length === 3
        && ['kind', 'location', 'note'].every(field => field in reference), 'Fonte fora do contrato.');
      exigir(['code', 'test'].includes(reference.kind), 'Tipo de fonte inválido neste incremento.');
      exigir(sources.has(reference.location), 'Referência sem cobertura no manifesto.');
      exigir(reference.location.length <= 2000 && typeof reference.note === 'string'
        && reference.note.trim() && reference.note.length <= 2000, 'Fonte fora dos limites do contrato.');
      used.add(reference.location);
      if (reference.kind === 'test') tests.add(reference.location);
    }
    testReferences.set(policy.id, tests);
  }
  exigir(used.size === sources.size, 'Manifesto contém fonte sem cobertura nas fichas.');
  const contents = new Map();
  for (const source of sources.values()) {
    const pinned = readPinned(manifest.sourceRevision, source.path);
    exigir(hash(pinned) === source.sha256, `SHA-256 divergente na revisão: ${source.path}`);
    exigir(hash(readCurrent(source.path)) === source.sha256, `SHA-256 divergente no checkout: ${source.path}`);
    contents.set(source.path, pinned.toString('utf8'));
  }
  const covered = new Set();
  const checkKeys = new Set();
  for (const check of manifest.checks) {
    caminhoSeguro(check.path);
    exigir(testReferences.get(check.policyId)?.has(prefix + check.path), 'Check sem referência de teste na ficha.');
    exigir(Array.isArray(check.selectors) && check.selectors.length > 0, 'Check exige seletor de teste.');
    for (const selector of check.selectors) {
      exigir(typeof selector === 'string' && /^[A-Za-z_][A-Za-z0-9_]*$/.test(selector), 'Seletor inválido.');
      const key = `${check.policyId}:${check.path}:${selector}`;
      exigir(!checkKeys.has(key), 'Check de teste duplicado.');
      checkKeys.add(key);
      exigir(new RegExp(`\\bfn\\s+${selector}\\s*\\(`).test(contents.get(check.path)),
        `Ausência do seletor ${selector} em ${check.path}.`);
    }
    covered.add(check.policyId);
  }
  exigir(covered.size === ids.size, 'Política sem vínculo de teste.');
  return { schemaVersion: 1, module: 'cms', sourceRevision: manifest.sourceRevision,
    inventorySha256: hash(JSON.stringify(policies)), policies: policies.length,
    sources: manifest.sources, checks: manifest.checks,
    sourceIntegrityVerified: true, productRuntimeVerified: false };
}

function executar() {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const git = (...args) => execFileSync('git', ['-C', root, ...args],
    { stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 8 * 1024 * 1024 });
  let report;
  try {
    const policies = JSON.parse(readFileSync(resolve(root, 'docs/mlops/cms-policies.json'), 'utf8'));
    const manifest = JSON.parse(readFileSync(resolve(root, 'docs/mlops/sources.json'), 'utf8'));
    report = { ...validarInventario({ policies, manifest,
      readPinned: (revision, path) => {
        // Após squash, a revisão da fonte pode existir só no histórico do PR.
        // Busca exclusivamente o SHA validado pelo contrato, sem checkout.
        if (!/^[a-f0-9]{40}$/.test(revision)) throw new Error('Revisão inválida.');
        try {
          git('cat-file', '-e', `${revision}^{commit}`);
        } catch {
          git('fetch', '--no-tags', 'origin', revision);
        }
        return git('show', `${revision}:${path}`);
      },
      readCurrent: path => readFileSync(resolve(root, path)),
    }), integrityOutcome: 'success' };
  } catch (error) {
    report = { schemaVersion: 1, module: 'cms', integrityOutcome: 'failure',
      sourceIntegrityVerified: false, productRuntimeVerified: false, error: error.message };
    process.exitCode = 1;
  }
  const outcomes = ['success', 'failure', 'cancelled', 'skipped'];
  report.rustTestsOutcome = outcomes.includes(process.env.MLOPS_RUST_TESTS_OUTCOME)
    ? process.env.MLOPS_RUST_TESTS_OUTCOME : 'not_observed';
  report.gateTestsOutcome = outcomes.includes(process.env.MLOPS_GATE_TESTS_OUTCOME)
    ? process.env.MLOPS_GATE_TESTS_OUTCOME : 'not_observed';
  report.contractsOutcome = outcomes.includes(process.env.MLOPS_CONTRACTS_OUTCOME)
    ? process.env.MLOPS_CONTRACTS_OUTCOME : 'not_observed';
  report.executionRevision = git('rev-parse', 'HEAD').toString().trim();
  report.runUrl = process.env.GITHUB_RUN_ID && process.env.GITHUB_REPOSITORY
    ? `https://github.com/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}` : null;
  report.recordedAt = new Date().toISOString();
  if (process.argv[2] === '--report') {
    const path = resolve(root, 'target/mlops/evidence.json');
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, JSON.stringify(report, null, 2) + '\n');
  }
  console.log(JSON.stringify(report));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) executar();

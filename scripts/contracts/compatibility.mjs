import Ajv2020 from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';

export function validator() {
  const ajv = new Ajv2020({ strict: false, allErrors: true, addUsedSchema: false });
  addFormats(ajv);
  ajv.addFormat('int32', { type: 'number', validate: n => Number.isInteger(n) && n >= -2147483648 && n <= 2147483647 });
  // JSON/JavaScript não preserva todos os inteiros i64; não promete precisão decimal.
  ajv.addFormat('int64', { type: 'number', validate: Number.isInteger });
  return ajv;
}

const canonical = value => JSON.stringify(value, (_, item) =>
  item && typeof item === 'object' && !Array.isArray(item)
    ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
const same = (a, b) => canonical(a) === canonical(b);
const annotations = new Set(['$schema', '$id', 'title', 'description', 'examples', 'default', 'deprecated', 'readOnly', 'writeOnly']);
const supported = new Set(['type', 'properties', 'required', 'additionalProperties', 'items', 'minItems', 'maxItems',
  'minimum', 'maximum', 'exclusiveMinimum', 'exclusiveMaximum', 'minLength', 'maxLength',
  'pattern', 'format', 'enum', 'const', 'uniqueItems', 'anyOf', 'oneOf']);

function expand(value, root = value, seen = new Set()) {
  if (typeof value === 'boolean') return value;
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Schema inválido.');
  let schema = { ...value };
  if (schema.$ref) {
    if (!schema.$ref.startsWith('#/') || seen.has(schema.$ref)) throw new Error('Referência não suportada.');
    const target = schema.$ref.slice(2).split('/').reduce((object, key) =>
      object?.[key.replaceAll('~1', '/').replaceAll('~0', '~')], root);
    if (target === undefined) throw new Error('Referência ausente.');
    const nextSeen = new Set([...seen, schema.$ref]);
    const { $ref, ...siblings } = schema;
    const expanded = expand(target, root, nextSeen);
    if (Object.keys(siblings).some(key => !annotations.has(key) && key !== '$defs')) {
      throw new Error('Interseção de referência exige revisão.');
    }
    return expanded;
  }
  delete schema.$defs;
  for (const key of annotations) delete schema[key];
  if (schema.properties) schema.properties = Object.fromEntries(Object.entries(schema.properties)
    .map(([key, child]) => [key, expand(child, root, seen)]));
  if (schema.items !== undefined) schema.items = expand(schema.items, root, seen);
  if (schema.additionalProperties && typeof schema.additionalProperties === 'object') {
    schema.additionalProperties = expand(schema.additionalProperties, root, seen);
  }
  for (const key of ['anyOf', 'oneOf']) {
    if (schema[key]) schema[key] = schema[key].map(child => expand(child, root, seen));
  }
  // Referências em construções ainda não suportadas não podem perder suas
  // definições e aparentar igualdade depois da normalização.
  const hasReference = node => node && typeof node === 'object'
    && (Object.hasOwn(node, '$ref') || Object.values(node).some(hasReference));
  if (hasReference(schema)) throw new Error('Referência em construção não suportada.');
  return schema;
}

const types = schema => new Set(schema.type ? [].concat(schema.type) :
  ['null', 'boolean', 'object', 'array', 'number', 'integer', 'string']);
function disjoint(a, b) {
  const left = types(a), right = types(b);
  const overlap = [...left].some(type => right.has(type) || type === 'integer' && right.has('number') || type === 'number' && right.has('integer'));
  if (!overlap) return true;
  for (const key of a.required ?? []) {
    if (!(b.required ?? []).includes(key)) continue;
    const av = a.properties?.[key], bv = b.properties?.[key];
    if (av && bv && Object.hasOwn(av, 'const') && Object.hasOwn(bv, 'const') && !same(av.const, bv.const)) return true;
  }
  return false;
}

function branches(schema) {
  const key = schema.oneOf ? 'oneOf' : schema.anyOf ? 'anyOf' : null;
  if (!key) return [schema];
  const { [key]: entries, ...rest } = schema;
  if (key === 'oneOf' && entries.some((entry, index) => entries.slice(index + 1).some(other => !disjoint(entry, other)))) {
    throw new Error('oneOf sem discriminação comprovada.');
  }
  return entries.map(entry => {
    if (typeof entry === 'boolean') {
      if (!entry) return false;
      return rest;
    }
    if (Object.keys(rest).some(name => Object.hasOwn(entry, name) && !same(entry[name], rest[name]))) {
      throw new Error('Interseção de união exige revisão.');
    }
    return { ...rest, ...entry };
  });
}

function subset(a, b) {
  if (a === false || b === true || same(a, b)) return true;
  if (b === false) return false;
  if (a === true) return Object.keys(b).length === 0;
  const av = Object.hasOwn(a, 'const') ? [a.const] : a.enum;
  if (av) {
    const validate = validator().compile(b);
    return av.every(value => validate(value));
  }
  if (Object.hasOwn(b, 'const') || b.enum) return false;
  if (a.oneOf || a.anyOf || b.oneOf || b.anyOf) {
    return branches(a).every(left => branches(b).some(right => subset(left, right)));
  }
  for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
    if (!supported.has(key) && !same(a[key], b[key])) return false;
  }
  const at = types(a), bt = types(b);
  if ([...at].some(type => !bt.has(type) && !(type === 'integer' && bt.has('number')))) return false;
  for (const key of ['minLength', 'minItems', 'minimum', 'exclusiveMinimum']) {
    if (b[key] !== undefined && (a[key] === undefined || a[key] < b[key])) return false;
  }
  for (const key of ['maxLength', 'maxItems', 'maximum', 'exclusiveMaximum']) {
    if (b[key] !== undefined && (a[key] === undefined || a[key] > b[key])) return false;
  }
  for (const key of ['pattern', 'format']) if (b[key] !== undefined && a[key] !== b[key]) return false;
  if (b.uniqueItems && !a.uniqueItems) return false;
  if (bt.has('object') && at.has('object')) {
    if ((b.required ?? []).some(key => !(a.required ?? []).includes(key))) return false;
    const ap = a.properties ?? {}, bp = b.properties ?? {};
    const extraA = a.additionalProperties ?? true, extraB = b.additionalProperties ?? true;
    for (const name of new Set([...Object.keys(ap), ...Object.keys(bp)])) {
      if (!subset(ap[name] ?? extraA, bp[name] ?? extraB)) return false;
    }
    if (!subset(extraA, extraB)) return false;
  }
  if (bt.has('array') && at.has('array') && !subset(a.items ?? true, b.items ?? true)) return false;
  return true;
}

/** Prova conservadora de inclusão no subconjunto suportado; incerteza reprova. */
export function isSubset(sub, sup) {
  try { return subset(expand(sub), expand(sup)); } catch { return false; }
}

export function compareContracts(baseline, candidate) {
  const differences = [];
  const reject = message => differences.push(message);
  const operations = new Map();
  if (candidate.schemaVersion !== 1 || candidate.module !== 'cms-apps-v1' || !Array.isArray(candidate.operations)) {
    return { compatible: false, initialContract: baseline === null, differences: ['Contrato inválido.'] };
  }
  for (const operation of candidate.operations) {
    if (operations.has(operation.id)) reject(`Operação duplicada: ${operation.id}`);
    operations.set(operation.id, operation);
  }
  if (baseline) {
    if (baseline.schemaVersion !== candidate.schemaVersion || baseline.module !== candidate.module) reject('Versão ou módulo divergente.');
    if (candidate.limits.definitionBytes < baseline.limits.definitionBytes) reject('Limite de entrada reduzido.');
    if (!isSubset(baseline.aadValidation, candidate.aadValidation)) reject('Validação AAD restringiu entradas anteriores.');
    for (const old of baseline.operations) {
      const next = operations.get(old.id);
      if (!next) { reject(`Operação removida: ${old.id}`); continue; }
      for (const key of ['method', 'path', 'authentication']) if (old[key] !== next[key]) reject(`${old.id}: ${key} alterado.`);
      if ((next.requiredHeaders ?? []).some(header => !(old.requiredHeaders ?? []).includes(header))) reject(`${old.id}: novo cabeçalho obrigatório.`);
      if (old.request === null && next.request !== null) reject(`${old.id}: novo corpo obrigatório exige revisão.`);
      if (old.request !== null && (next.request === null || !isSubset(old.request, next.request))) reject(`${old.id}: entrada incompatível ou não comprovada.`);
      for (const [status, schema] of Object.entries(old.responses)) {
        if (!Object.hasOwn(next.responses, status)) { reject(`${old.id}: status ${status} removido.`); continue; }
        const body = next.responses[status];
        if (schema === null ? body !== null : body === null || !isSubset(body, schema)) reject(`${old.id}: saída ${status} incompatível ou não comprovada.`);
        if ((old.responseHeaders?.[status] ?? []).some(header => !(next.responseHeaders?.[status] ?? []).includes(header))) reject(`${old.id}: cabeçalho da resposta ${status} removido.`);
      }
      for (const status of Object.keys(next.responses)) if (!Object.hasOwn(old.responses, status)) reject(`${old.id}: novo status ${status} exige revisão do consumidor.`);
    }
  }
  return { compatible: differences.length === 0, initialContract: baseline === null, differences };
}

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

export const baselinePath = 'docs/contracts/apps-v1.baseline.json';

export function loadBaseline(revision, cwd = process.cwd(), path = baselinePath) {
  assert.ok([baselinePath, 'docs/contracts/oauth-v1.baseline.json'].includes(path), 'baseline não autorizado');
  assert.match(revision ?? '', /^[a-f0-9]{40}$/, 'baseline exige SHA completo');
  const git = (...args) => execFileSync('git', args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] }).toString().trim();
  git('cat-file', '-e', `${revision}^{commit}`);
  const exists = git('ls-tree', '--name-only', revision, '--', path) === path;
  return exists ? JSON.parse(git('show', `${revision}:${path}`)) : null;
}

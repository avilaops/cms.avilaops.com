import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

export const baselinePath = 'docs/contracts/apps-v1.baseline.json';

export function loadBaseline(revision, cwd = process.cwd()) {
  assert.match(revision ?? '', /^[a-f0-9]{40}$/, 'baseline exige SHA completo');
  const git = (...args) => execFileSync('git', args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] }).toString().trim();
  git('cat-file', '-e', `${revision}^{commit}`);
  const exists = git('ls-tree', '--name-only', revision, '--', baselinePath) === baselinePath;
  return exists ? JSON.parse(git('show', `${revision}:${baselinePath}`)) : null;
}

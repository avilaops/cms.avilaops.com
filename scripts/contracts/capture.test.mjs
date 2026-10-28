import assert from 'node:assert/strict';
import test from 'node:test';
import { assertCaptureIdentity } from './capture.mjs';

test('capturas de cache, commit anterior ou outra tentativa não servem como evidência atual', () => {
  const revision = 'a'.repeat(40);
  assertCaptureIdentity({ executionRevision: revision, runId: '10/2' }, revision, '10/2');
  for (const record of [{}, { executionRevision: 'b'.repeat(40), runId: '10/2' },
    { executionRevision: revision, runId: '10/1' }, { executionRevision: revision, runId: '9/2' }]) {
    assert.throws(() => assertCaptureIdentity(record, revision, '10/2'));
  }
});

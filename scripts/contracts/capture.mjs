import assert from 'node:assert/strict';

export function assertCaptureIdentity(record, executionRevision, runId) {
  assert.match(executionRevision ?? '', /^[a-f0-9]{40}$/);
  assert.match(runId ?? '', /^\d+\/\d+$/);
  assert.equal(record.executionRevision, executionRevision, 'captura pertence a outro commit');
  assert.equal(record.runId, runId, 'captura pertence a outra execução/tentativa');
}

"""Offline acquisition guards and metadata retention."""
import asyncio
import json
from pathlib import Path

import pytest

from tbench.public_replays import acquire, digest, trial_record

ID = "00000000-0000-0000-0000-000000000001"


def test_trial_record_never_copies_provider_configuration():
    record = trial_record({"id":ID,"task_name":"terminal-bench/task","trial_name":"task__1","job_id":"job","config":{"secret":"not retained"},"agent_info":{"version":"2.1"},"agent_execution":{"started_at":"start","finished_at":"end"},"verifier_result":{"rewards":{"reward":1}}}, {"id":"row","rank":2,"metadata":{"reasoning_effort":"max"}})
    assert record["task"] == "task"
    assert record["reward"] == 1
    assert record["started_at"] == "start"
    assert "secret" not in json.dumps(record)
    assert "config" not in record


def test_resume_uses_verified_bytes_and_counts_missing_public_transcripts(tmp_path):
    path = tmp_path/f"{ID}.json"
    path.write_text('{"steps":[]}')
    manifest = {"trials":[{"id":ID,"file":path.name,"sha256":digest(path),"trajectory_path":"remote","task":"task"}, {"id":"00000000-0000-0000-0000-000000000002","file":"00000000-0000-0000-0000-000000000002.json","task":"task","trajectory_path":None}]}
    asyncio.run(acquire(manifest,tmp_path,1))
    assert manifest["coverage"]["available"] == 1
    assert manifest["coverage"]["published"] == 1
    assert manifest["coverage"]["attempts"] == 2
    assert manifest["trials"][1]["error"] == "No published trajectory"


def test_paths_cannot_escape_the_cache(tmp_path):
    manifest = {"trials":[{"id":ID,"file":"../outside.json"}]}
    with pytest.raises(ValueError,match="identity"):
        asyncio.run(acquire(manifest,tmp_path,1))

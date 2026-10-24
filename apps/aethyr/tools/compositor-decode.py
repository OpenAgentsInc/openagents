#!/usr/bin/env python3
"""Decode bounded Chrome trace evidence with the official Perfetto processor."""
import argparse
import csv
import hashlib
import io
import json
import pathlib
import subprocess

MAX_TRACE_BYTES = 128 * 1024 * 1024
MAX_ROWS = 200000
MAX_CSV_FIELD_BYTES = 16 * 1024 * 1024
SQL = r"""
WITH selected AS (
 SELECT s.*, t.name AS track_name, t.source_arg_set_id,
        COALESCE(p.pid, pp.pid) AS pid, th.tid
 FROM slice s JOIN track t ON t.id=s.track_id
 LEFT JOIN thread_track tt ON tt.id=s.track_id
 LEFT JOIN thread th ON th.utid=tt.utid LEFT JOIN process p ON p.upid=th.upid
 LEFT JOIN process_track pt ON pt.id=s.track_id LEFT JOIN process pp ON pp.upid=pt.upid
 WHERE s.name IN ('Graphics.Pipeline','Graphics.Pipeline.DrawAndSwap','PipelineReporter',
   'Display::FrameDisplayed','LayerTreeHostImpl:snapshot','OffscreenCanvas::PushFrame',
   'CanvasResourceDispatcher::DispatchFrame','TracingStartedInBrowser')
 OR s.name GLOB 'af.compositor.*'
 LIMIT 200001
)
SELECT 'slice' AS kind, HEX(JSON_OBJECT(
 'id',CAST(s.id AS TEXT),'tsNs',CAST(s.ts AS TEXT),'durNs',CAST(s.dur AS TEXT),
 'name',s.name,'category',s.category,'trackId',CAST(s.track_id AS TEXT),
 'trackName',s.track_name,'pid',CAST(s.pid AS TEXT),'tid',CAST(s.tid AS TEXT),
 'args',JSON(COALESCE((SELECT JSON_GROUP_OBJECT(a.key,
  COALESCE(a.string_value,CAST(a.int_value AS TEXT),CAST(a.real_value AS TEXT)))
  FROM args a WHERE a.arg_set_id=s.arg_set_id),'{}')),
 'trackArgs',JSON(COALESCE((SELECT JSON_GROUP_OBJECT(a.key,
  COALESCE(a.string_value,CAST(a.int_value AS TEXT),CAST(a.real_value AS TEXT)))
  FROM args a WHERE a.arg_set_id=s.source_arg_set_id),'{}'))
 )) AS payload FROM selected s
UNION ALL SELECT 'stat', HEX(JSON_OBJECT('name',name,'severity',severity,
 'value',CAST(value AS TEXT),'source',source)) FROM stats WHERE value!=0
UNION ALL SELECT 'bounds',HEX(JSON_OBJECT('startNs',CAST(start_ts AS TEXT),
 'endNs',CAST(end_ts AS TEXT))) FROM trace_bounds;
"""


def decode(processor, trace, output):
    trace = pathlib.Path(trace).resolve()
    processor = pathlib.Path(processor).resolve()
    if not 0 < trace.stat().st_size <= MAX_TRACE_BYTES:
        raise ValueError("Trace size is outside the bounded prototype limit.")
    version = subprocess.run([str(processor), "--version"], capture_output=True,
                             text=True, check=True, timeout=10)
    result = subprocess.run([str(processor), "query", str(trace), SQL],
                            capture_output=True, check=True, timeout=90)
    if len(result.stdout) > 192 * 1024 * 1024:
        raise ValueError("Decoded trace output exceeds the bounded limit.")
    csv.field_size_limit(MAX_CSV_FIELD_BYTES)
    rows = csv.DictReader(io.StringIO(result.stdout.decode("utf8")))
    decoded = {"schema": "aethyrforge.compositor-decoded.v1", "slices": [],
               "stats": [], "bounds": None, "processor": {
                   "path": str(processor),
                   "sha256": hashlib.sha256(processor.read_bytes()).hexdigest(),
                   "version": (version.stdout + version.stderr).strip()},
               "trace": {"path": str(trace), "bytes": trace.stat().st_size,
                         "sha256": hashlib.sha256(trace.read_bytes()).hexdigest()},
               "decoderSha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()}
    for row in rows:
        value = json.loads(bytes.fromhex(row["payload"]).decode("utf8"))
        if row["kind"] == "slice":
            decoded["slices"].append(value)
            if len(decoded["slices"]) > MAX_ROWS:
                raise ValueError("Trace slice limit reached; attribution is incomplete.")
        elif row["kind"] == "stat":
            decoded["stats"].append(value)
        elif row["kind"] == "bounds":
            decoded["bounds"] = value
    output = pathlib.Path(output).resolve()
    output.write_text(json.dumps(decoded, separators=(",", ":")) + "\n")
    return {"path": str(output), "bytes": output.stat().st_size,
            "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            "slices": len(decoded["slices"]), "processor": decoded["processor"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--processor", required=True)
    parser.add_argument("--trace", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    print(json.dumps(decode(args.processor, args.trace, args.out)))

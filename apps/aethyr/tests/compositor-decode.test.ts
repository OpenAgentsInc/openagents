import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

test("trace decoder accepts bounded large layer fields and rejects field/row overflows", () => {
  const result = execFileSync(
    "python3",
    [
      "-c",
      String.raw`
import csv, importlib.util, io, json, pathlib, subprocess, sys, tempfile
from types import SimpleNamespace
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('decoder',sys.argv[1])
d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
def data(rows):
 out=io.StringIO();writer=csv.writer(out);writer.writerow(['kind','payload'])
 for kind,value in rows:writer.writerow([kind,json.dumps(value).encode().hex()])
 return out.getvalue().encode()
def run(rows,root):
 payload=data(rows)
 def mocked(args,**options):
  return SimpleNamespace(stdout='mock test processor',stderr='') if args[1]=='--version' else SimpleNamespace(stdout=payload,stderr=b'')
 with patch.object(subprocess,'run',side_effect=mocked):
  return d.decode(root/'processor',root/'trace',root/'decoded')
with tempfile.TemporaryDirectory(prefix='af-compositor-decode-') as folder:
 root=pathlib.Path(folder);(root/'processor').write_bytes(b'test');(root/'trace').write_bytes(b'trace')
 layer={'id':'1','name':'LayerTreeHostImpl:snapshot','args':{'snapshot':'x'*100000}}
 result=run([('slice',layer)],root)
 assert result['slices']==1
 assert json.loads((root/'decoded').read_text())['slices'][0]==layer
 original_limit=d.MAX_CSV_FIELD_BYTES;d.MAX_CSV_FIELD_BYTES=1000
 try:run([('slice',layer)],root)
 except csv.Error:pass
 else:raise AssertionError('Oversized field accepted')
 d.MAX_CSV_FIELD_BYTES=original_limit;d.MAX_ROWS=1
 try:run([('slice',{'id':'1'}),('slice',{'id':'2'})],root)
 except ValueError as error:assert 'slice limit' in str(error)
 else:raise AssertionError('Excess rows accepted')
 print('bounded decoder checks passed')
`,
      fileURLToPath(new URL("../tools/compositor-decode.py", import.meta.url)),
    ],
    {
      encoding: "utf8",
      timeout: 5000,
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
    },
  );
  assert.match(result, /bounded decoder checks passed/);
});

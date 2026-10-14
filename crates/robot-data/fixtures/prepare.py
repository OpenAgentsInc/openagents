"""Prepare small licensed LeRobot fixtures with pyarrow==21.0.0.

Pass an explicit scratch directory. This is offline acceptance tooling.
"""

import hashlib
import json
import pathlib
import urllib.request
import copy
import shutil

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

import sys

scratch = pathlib.Path(sys.argv[1]).resolve()
scratch.mkdir(parents=True, exist_ok=True)
raw = scratch / 'robot-dataset-source'
raw.mkdir(exist_ok=True)
revision = '73c13ed578e293bd08ec0b368216ae66caa272e6'
repo = 'avishekjana/so101-dataset-v2'
files = ['README.md', 'meta/info.json', 'meta/stats.json', 'meta/tasks.parquet', 'meta/episodes/chunk-000/file-000.parquet', 'data/chunk-000/file-000.parquet']
pins = {}
for relative in files:
    path = raw / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        with urllib.request.urlopen(f'https://huggingface.co/datasets/{repo}/resolve/{revision}/{relative}', timeout=40) as response:
            data = response.read((64 << 20) + 1)
            assert len(data) <= 64 << 20
            path.write_bytes(data)
    data = path.read_bytes()
    pins[relative] = {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
table = pq.read_table(raw / 'data/chunk-000/file-000.parquet')
meta = pq.read_table(raw / 'meta/episodes/chunk-000/file-000.parquet')
tasks = pq.read_table(raw / 'meta/tasks.parquet')



(scratch / 'robot-source-pins.json').write_text(json.dumps(pins, indent=2))
frames = table.slice(0, 4).to_pylist()
(scratch / 'robot-source-frames.json').write_text(json.dumps(frames, indent=2))

repo_root = pathlib.Path(__file__).resolve().parents[3]
fixture_root = repo_root / 'crates/robot-data/tests/fixtures'
rows = table.to_pylist()
selected = [row for row in rows if row['episode_index'] in (0,1) and row['frame_index'] < 4]
assert len(selected) == 8
for i,row in enumerate(selected): row['index'] = i
selected_table = pa.Table.from_pylist(selected, schema=table.schema)
episode_rows = meta.to_pylist()[:2]
original_info = json.loads((raw / 'meta/info.json').read_text())
statistics = {}
for feature in ['action','observation.state','timestamp','frame_index','episode_index','index','task_index']:
    values = np.asarray([row[feature] for row in selected], dtype=np.float64)
    if values.ndim == 1: values = values[:,None]
    statistics[feature] = {name: value.tolist() for name,value in [('min',values.min(0)),('max',values.max(0)),('mean',values.mean(0)),('std',values.std(0))]}
    statistics[feature]['count'] = [len(selected)]
for version in ['v2.1','v3.0']:
    target = fixture_root / version
    (target / 'meta').mkdir(parents=True, exist_ok=True)
    info = copy.deepcopy(original_info)
    info.update(codebase_version=version,total_episodes=2,total_frames=8)
    shutil.copyfile(raw / 'README.md',target / 'SOURCE.md')
    shutil.copyfile(repo_root / 'LICENSE',target / 'LICENSE')
    (target / 'NOTICE').write_text(f'Frames from {repo} at {revision}. Dataset creator: avishekjana. Apache-2.0, as declared in SOURCE.md. Joint values, actions, timestamps, and frame indexes are unchanged. Cropped to the first four frames of episodes 0 and 1; file layout, global indexes, counts, and statistics are updated. Converted to v2.1 only for the v2.1 fixture. Camera files are optional and omitted.\n')
    (target / 'meta/stats.json').write_text(json.dumps(statistics,indent=2)+'\n')
    if version == 'v3.0':
        path = target / 'data/chunk-000/file-000.parquet'; path.parent.mkdir(parents=True,exist_ok=True)
        pq.write_table(selected_table,path,compression='snappy')
        converted_meta=[]
        for i,row in enumerate(episode_rows):
            row={k:v for k,v in row.items() if not k.startswith('stats/')}
            row.update(length=4,dataset_from_index=i*4,dataset_to_index=i*4+4)
            for key in list(row):
                if key.startswith('videos/') and key.endswith('/to_timestamp'):
                    row[key] = row[key.replace('/to_timestamp','/from_timestamp')] + 4/info['fps']
            converted_meta.append(row)
        path = target / 'meta/episodes/chunk-000/file-000.parquet'; path.parent.mkdir(parents=True,exist_ok=True)
        pq.write_table(pa.Table.from_pylist(converted_meta),path,compression='snappy')
        pq.write_table(tasks,target / 'meta/tasks.parquet',compression='snappy')
    else:
        info['data_path']='data/chunk-{episode_chunk:03d}/episode_{episode_index:06d}.parquet'
        info['video_path']='videos/chunk-{episode_chunk:03d}/{video_key}/episode_{episode_index:06d}.mp4'
        for index in [0,1]:
            path = target / f'data/chunk-000/episode_{index:06d}.parquet'; path.parent.mkdir(parents=True,exist_ok=True)
            pq.write_table(selected_table.slice(index*4,4),path,compression='snappy')
        (target / 'meta/episodes.jsonl').write_text(''.join(json.dumps({'episode_index':i,'tasks':['pick_and_place'],'length':4})+'\n' for i in [0,1]))
        (target / 'meta/tasks.jsonl').write_text(json.dumps({'task_index':0,'task':'pick_and_place'})+'\n')
    (target / 'meta/info.json').write_text(json.dumps(info,indent=2)+'\n')
    current = {str(path.relative_to(target)): {'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'bytes':path.stat().st_size} for path in target.rglob('*') if path.is_file()}
    pin = {'repository':f'https://huggingface.co/datasets/{repo}','revision':revision,'license':'Apache-2.0','notice':'NOTICE','files':current}
    (target / 'source.json').write_text(json.dumps(pin,indent=2)+'\n')
(fixture_root / 'expected.json').write_text(json.dumps({'source_revision':revision,'source_files':pins,'frames':selected},indent=2)+'\n')
print(f'Wrote fixtures under {fixture_root}',flush=True)

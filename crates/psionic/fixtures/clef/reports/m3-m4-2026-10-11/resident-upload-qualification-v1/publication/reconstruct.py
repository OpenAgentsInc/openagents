"""Verify a publication and all losslessly split receipts without model files."""
import argparse,hashlib,json
from pathlib import Path

def sha(raw):return hashlib.sha256(raw).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('--private-map',type=Path);args=parser.parse_args()
    manifest=json.loads((args.root/'manifest.json').read_text());mapping=json.loads(args.private_map.read_text()) if args.private_map else None
    for row in manifest['files']:
        raw=(args.root/row['path']).read_bytes()
        assert len(raw)==row['published_bytes'] and sha(raw)==row['published_sha256']
        if mapping and 'original_sha256' in row:
            original=raw.decode()
            for token,value in reversed(list(mapping.items())):original=original.replace(token,value)
            assert len(original.encode())==row['original_bytes'] and sha(original.encode())==row['original_sha256']
    for row in manifest['split_files']:
        index=(args.root/row['index']).read_bytes();assert sha(index)==row['index_sha256'];info=json.loads(index)
        parts=[]
        for part in info['parts']:
            raw=(args.root/part['path']).read_bytes();assert len(raw)==part['bytes'] and sha(raw)==part['sha256'];parts.append(raw)
        clean=b''.join(parts);assert sha(clean)==info['published_sha256'] and len(clean)==info['published_bytes']
        if mapping:
            original=clean.decode()
            for token,value in reversed(list(mapping.items())):original=original.replace(token,value)
            assert sha(original.encode())==info['original_sha256'] and len(original.encode())==info['original_bytes']
    print(json.dumps({'status':'complete','files':len(manifest['files']),'split_files':len(manifest['split_files']),'original_reconstruction':mapping is not None}))
if __name__=='__main__':main()

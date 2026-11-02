from pathlib import Path
import hashlib,json
root=Path('/tmp/aethyr-ch11-owned-sans-frame-inputs');root.mkdir(exist_ok=True)
for slug,source in [('the-grid',Path('/tmp/aethyr-ch11-native-grid-sans/stage/frame.config')),('galaxy',Path('/tmp/aethyr-ch11-galaxy-half-160/frame.config'))]:
 out=root/slug;out.mkdir(exist_ok=True);package=Path('/tmp/aethyr-ch11-packages-owned-sans')/slug/'render.json';p=json.loads(package.read_text());original=source.read_text().split();t=original.copy();files={};changed=[];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
 def original_path(token):return Path(token)if Path(token).is_absolute()else source.parent/token
 def keep_path(index):
  if t[index]!='-':
   file=original_path(t[index]);files[str(file)]=sha(file);t[index]=str(file.resolve())
 for i in range(13,17):keep_path(i)
 t[17]=str(package);at=18;n=int(t[at]);at+=1;entries={e['entry_index']:e for e in p['entries']};assert set(entries)=={int(t[at+i*2])for i in range(n)}
 for i in range(n):
  index=int(t[at]);entry=entries[index];tree=package.parent/entry['tree_file'];spv=package.parent/entry['spirv_file'];assert sha(tree)==entry['tree_sha256'];assert sha(spv)==entry['spirv_sha256'];changed.append({'entry':index,'old_tree':str(original_path(t[at+1])),'new_tree':str(tree),'new_tree_sha256':sha(tree),'spirv_sha256':sha(spv)});t[at+1]=str(tree);at+=2
 n=int(t[at]);at+=1
 for i in range(n):keep_path(at+1);at+=8
 if at<len(t)and t[at]=='textures':
  n=int(t[at+1]);at+=2
  for i in range(n):levels=int(t[at+3]);keep_path(at+4);at+=5+5*levels
 if at<len(t)and t[at]=='media_text':keep_path(at+1);at+=2
 if at<len(t)and t[at]=='media_images':at+=2+5*int(t[at+1])
 assert at==len(t);assert t[:13]==original[:13];assert int(t[0])==p['profile_index'];(out/'frame.config').write_text('\n'.join(t)+'\n')
 record={'scope':'Explicit shader-product substitution: original actual producer counts, buffers, texture bytes and all raw observations retained. No producer time/camera reconstruction. Only package/tree paths change; physical-file paths are made absolute.','original_config':str(source),'original_config_sha256':sha(source),'derived_config_sha256':sha(out/'frame.config'),'package':str(package),'package_sha256':sha(package),'unchanged_producer_resource_files':files,'changed_shader_products':changed};(out/'derivation.json').write_text(json.dumps(record,indent=2)+'\n');print(slug,out/'frame.config')

"""Read Git objects and derive changed-input book observations; no repository imports/gates."""
from pathlib import Path
from urllib.parse import urlsplit,unquote
import collections,hashlib,json,posixpath,re,subprocess,unicodedata
R=Path('/Users/christopherdavid/.codex/worktrees/coax-audit-issues/openagents')
D=Path(__file__).resolve().parent
BASE='8fd5f7fdbc9fe9503be97841e24620cb307be106'
INCOMING='5426f4af89bcbe17b6e3c39132fc4de0d06efb9e'
REV='6784112fb02d3d418747e63923e22c6831c68c86'
ROOT_REV='44b25a4b3e83797ed8d6d3ffd9ca885aab8c1449'
P=Path('/tmp/coax-return-borrows-A1/current-books-latest-main-v12')
H=lambda b:hashlib.sha256(b).hexdigest()
def git(*args): return subprocess.check_output(['git',*args],cwd=R,timeout=30)
head_before=git('rev-parse','HEAD').decode().strip()
prior=json.loads((P/'review.json').read_text())
assert H((P/'review.json').read_bytes())=='f8a5e63e186947534a37a2369a32d8e3c5a962a3ea323d6455de8d036a4c0dff'
assert prior['reviewed_revision']==BASE
cache={}; inputs={}; trees={}
def tree(rev):
 if rev not in trees:
  result={}
  for row in git('ls-tree','-r','-z',rev).split(b'\0'):
   if row:
    meta,name=row.split(b'\t',1); mode,kind,oid=meta.decode().split(); result[name.decode()]={'mode':mode,'kind':kind,'oid':oid}
  trees[rev]=result
 return trees[rev]
def data(path,rev=REV):
 key=(rev,path)
 if key not in cache:
  entry=tree(rev)[path]; assert entry['kind']=='blob' and entry['mode']!='120000',key
  cache[key]=git('cat-file','blob',entry['oid'])
 inputs[rev+':'+path]={'git_blob':tree(rev)[path]['oid'],'sha256':H(cache[key])}
 return cache[key]
def read(path,rev=REV): return data(path,rev).decode()
def exists(path): return path in tree(REV) or any(p.startswith(path.rstrip('/')+'/') for p in tree(REV))
def plain(text):
 out=[]; active=None
 for line in text.splitlines():
  m=re.match(r'^\s*(`{3,}|~{3,})',line)
  if m:
   if active is None: active=m[1][0]
   elif m[1][0]==active:active=None
   out.append('')
  else:out.append(line if active is None else '')
 return '\n'.join(out)
def anchors(path):
 text=plain(read(path)); out=set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)',text)); seen=collections.Counter()
 for line in text.splitlines():
  m=re.match(r'^#{1,6}\s+(.+?)\s*#*$',line)
  if not m:continue
  s=re.sub(r'<[^>]*>','',m[1]).lower(); s=''.join(c for c in s if c in '-_ ' or unicodedata.category(c)[0] in 'LNM').replace(' ','-')
  out.add(s+('-'+str(seen[s]) if seen[s] else '')); seen[s]+=1
 return out
findings=[]
def check(ok,finding_kind,**facts):
 if not ok:findings.append({'kind':finding_kind,'facts':facts})
def resolve(doc,href):
 u=urlsplit(href)
 if not u.scheme and not u.netloc:return posixpath.normpath(posixpath.join(posixpath.dirname(doc),unquote(u.path))) if u.path else doc,'local'
 m=re.fullmatch(r'/OpenAgentsInc/openagents/(?:blob|tree)/main/(.+)',u.path)
 if u.netloc=='github.com' and m:return unquote(m[1]),'repository-main'
 return None,'external-unchecked'
def links(doc,text):
 out=[]; t=plain(text)
 for m in re.finditer(r'\]\(([^)]+)\)',t):
  dest,kind=resolve(doc,m[1]);out.append({'document':doc,'line':t[:m.start()].count('\n')+1,'href':m[1],'resolved':dest,'kind':kind})
 return out
changed=set(git('diff','--name-only',BASE,INCOMING).decode().splitlines())
(D/'incoming-name-status.txt').write_bytes(git('diff','--name-status',BASE,INCOMING))
(D/'book-changes.diff').write_bytes(git('diff','--unified=4',BASE,INCOMING,'--','coax/book','psionic/book','INVARIANTS.md'))
books=['coax','psionic']; docs={b:sorted(p for p in tree(REV) if p.startswith(b+'/book/') and p.count('/')==2 and p.endswith('.md')) for b in books}
changed_docs=sorted(p for b in books for p in docs[b] if p in changed)
all_links=[]; listings=[]; counts={}
for b in books:
 text='\n'.join(read(p) for p in docs[b]); counts[b]={'markdown_pages':len(docs[b]),'fenced_blocks_inventory_only':len(re.findall(r'^```[^`\n]*$',text,re.M))//2,'source_tag_blocks':len(re.findall(r'<!-- source:',text)),'checked_source_blocks':len(re.findall(r'<!-- checked-source:',text))}
 for doc in docs[b]:
  t=read(doc);all_links+=links(doc,t)
  for m in re.finditer(r'<!-- (checked-source|source): ([^\n]+) -->\n```[^\n]*\n(.*?)\n```',t,re.S):
   kind,args,listing=m.groups()
   if kind=='source': src,tag=args.split(' '); expected=read(src) if doc in changed_docs else None
   else: src=posixpath.normpath(posixpath.join(posixpath.dirname(doc),args));tag=None;expected=read(src) if doc in changed_docs else None
   ok=None
   if doc in changed_docs:
    if tag:
     start,end='// book:begin '+tag+'\n','// book:end '+tag
     ok=expected.count(start)==expected.count(end)==1 and listing==expected.split(start)[1].split(end)[0].rstrip('\n')
    else:ok=listing in expected
    check(ok,'changed-source-listing',document=doc,source=src,tag=tag)
   listings.append({'document':doc,'kind':kind,'source':src,'tag':tag,'newly_checked':doc in changed_docs,'matched':ok})
checked_links=[]
for row in all_links:
 dest=row['resolved']
 if dest is None or (row['document'] not in changed_docs and dest not in changed):continue
 checked_links.append(row);check(not dest.startswith('../') and exists(dest),'file-link',**row)
 frag=unquote(urlsplit(row['href']).fragment)
 if frag and dest.endswith('.md') and dest in tree(REV):check(frag in anchors(dest) or bool(re.fullmatch(r'L\d+(?:-L\d+)?',frag)),'fragment-link',**row)
new_fixture_docs=sorted({x['resolved'] for x in checked_links if x['resolved'] in changed and x['resolved'].endswith('.md') and not x['resolved'].startswith(('coax/book/','psionic/book/'))})
for doc in new_fixture_docs:
 for row in links(doc,read(doc)):
  dest=row['resolved']
  if dest is None:continue
  check(exists(dest),'new-fixture-link',**row);frag=unquote(urlsplit(row['href']).fragment)
  if frag and dest.endswith('.md') and dest in tree(REV):check(frag in anchors(dest),'new-fixture-fragment',**row)
registries={b:json.loads(read(b+'/book/chapters.json'))['chapters'] for b in ['coax','psionic','aethyr','nostr']}
registry_changes=[]
for b in books:
 rows=registries[b];old={c['number']:c for c in json.loads(read(b+'/book/chapters.json',BASE))['chapters']};summary=read(b+'/book/SUMMARY.md');readme=read(b+'/book/README.md')
 counts[b].update(chapters=len(rows),chapter_statuses=dict(collections.Counter(c['status'] for c in rows)))
 if b+'/book/chapters.json' in changed or b+'/book/SUMMARY.md' in changed:
  nav=[unquote(urlsplit(h).path) for h in re.findall(r'\]\(([^)]+)\)',plain(summary)) if urlsplit(h).path.endswith('.md')]
  check([x for x in nav if x.startswith('ch')]==[c['file'] for c in rows if c['file']],'chapter-navigation',book=b)
  check({p.rsplit('/',1)[1] for p in docs[b] if p.rsplit('/',1)[1].startswith('ch')}=={c['file'] for c in rows if c['file']},'chapter-membership',book=b)
  targets={unquote(urlsplit(h).path) for h in re.findall(r'\]\(([^)]+)\)',summary)}
  check({p.rsplit('/',1)[1] for p in docs[b] if not p.endswith('/SUMMARY.md')}<=targets,'page-reachability',book=b)
  check([c['number'] for c in rows]==list(range(1,len(rows)+1)),'numbering',book=b)
 for c in rows:
  if c==old.get(c['number']):continue
  doc=b+'/book/'+c['file'];t=read(doc);check(t.splitlines()[0]=='# '+str(c['number'])+'. '+c['title'],'chapter-heading',document=doc)
  expected={'implemented':'Runnable: '+(c['profile'] or ''),'partly implemented':'Partly implemented: '+(c['profile'] or ''),'design draft':'Unimplemented: design draft','planned':'Planned'}[c['status']]
  cells=re.findall(r'^\| \[\d+\. [^\]]+\]\('+re.escape(c['file'])+r'\) \| ([^|]*) \|',readme,re.M)
  check(len(cells)==1 and (cells[0].strip()==expected or cells[0].strip().startswith(expected+' (')),'readme-profile',document=doc)
  for path in c['source']+c['adopted_by']:check(path in tree(REV),'registered-source',path=path);data(path)
  for dep in c['depends_on']:
   valid=(dep!=c['number'] and 1<=dep<=len(rows)) if type(dep)is int else (lambda pair:pair[0]!=b and 1<=int(pair[1])<=len(registries[pair[0]]))(dep.split(':'))
   check(valid,'chapter-dependency',book=b,number=c['number'],value=dep)
  registry_changes.append({'book':b,'number':c['number'],'status':c['status'],'profile':c['profile'],'sources_added':sorted(set(c['source'])-set(old.get(c['number'],{}).get('source',[])))})
manifest=json.loads(read('psionic/book/examples.json'))['examples'];old_manifest=json.loads(read('psionic/book/examples.json',BASE))['examples'];declared=[x['source']for x in manifest];actual={p for p in tree(REV)if p.startswith('psionic/examples/')and p.count('/')==2 and p.endswith('.coax')}
check(len(declared)==len(set(declared)) and set(declared)==actual,'closed-program-inventory')
new_examples=[e for e in manifest if e not in old_manifest]
for e in new_examples:
 check(set(e)=={'source','chapter','stdout','exit_status','modes'}and type(e['exit_status'])is int and set(e['modes'])=={'debug','release'},'example-shape',source=e['source'])
 for key in ['source','chapter','stdout']:check(e[key]in tree(REV),'example-path',path=e[key]);data(e[key])
ledger=json.loads(read('coax/book/capability-claims.json'));old_ledger={c['id']:c for c in json.loads(read('coax/book/capability-claims.json',BASE))['claims']};features={f['id']:f for f in json.loads(read(ledger['catalogue']))['features']};claim_rows=[]
check(len(ledger['claims'])==9 and len({c['id']for c in ledger['claims']})==9,'claim-count')
for c in ledger['claims']:
 blocks=re.findall(r'<!-- capability-claim: '+re.escape(c['id'])+r' -->\n(.*?)\n<!-- /capability-claim -->',read(c['document']),re.S);check(len(blocks)==1 and H(blocks[0].encode())==c['text_sha256'],'claim-text',claim=c['id'])
 for f in c['features']:check(H(json.dumps(features[f['id']],sort_keys=True,ensure_ascii=False,separators=(',',':')).encode())==f['sha256'],'claim-feature',claim=c['id'],feature=f['id'])
 for e in c['evidence']:check(H(data(e['path']))==e['sha256'],'claim-evidence',claim=c['id'],path=e['path'])
 claim_rows.append({'id':c['id'],'ledger_object_changed':c!=old_ledger[c['id']],'changed_evidence':[e['path']for e in c['evidence']if e not in old_ledger[c['id']]['evidence']]})
# A1 prose is not on admitted main yet: exact unchanged base paths retain the prior Root review.
prior_rows=json.loads((P/'inventory.json').read_text())['inputs'];prior_inputs={str(R/k.split(':',1)[1]):v['sha256']for k,v in prior_rows.items()if k.startswith(BASE+':')};reused=0;root_only=[]
for abs_path,expected in prior_inputs.items():
 path=str(Path(abs_path).relative_to(R))
 if path not in tree(REV):continue
 if H(data(path))==expected:reused+=1
 elif path not in changed:root_only.append(path)
head_after=git('rev-parse','HEAD').decode().strip();assert head_after==head_before
inventory={'schema':'coax.books.git-object-inputs.v1','reviewed_revision':REV,'base':BASE,'root_source_revision':ROOT_REV,'inputs':inputs,'root_head_before':head_before,'root_head_after':head_after}
f=D/'inventory.json';f.write_text(json.dumps(inventory,indent=2)+'\n')
result={'schema':'coax.books.latest-main-v12.static-review.v1','outcome':'passed-static-review'if not findings else 'static-findings','reviewed_revision':REV,'reviewed_tree':git('rev-parse',REV+'^{tree}').decode().strip(),'base':BASE,'root_source_revision':ROOT_REV,'prior_review':{'path':str(P/'review.json'),'sha256':H((P/'review.json').read_bytes())},'scope':'Read-only Git-object changed-input navigation, registration, file/fragment, exact source-listing and reviewed-claim binding observations. No existing repository gate/import, compiler, native/model/book execution, network or Root write. Incoming main objects reviewed before rebase; preserve unpublished A1/cache prose.','counts':counts,'changed_pages':changed_docs,'incoming_paths':len(changed),'fresh_links':len(checked_links),'new_fixture_docs':new_fixture_docs,'source_listings':len([x for x in listings if x['kind']=='source']),'fresh_source_listings':[x for x in listings if x['newly_checked']],'program_inventory':{'declared':len(declared),'actual_top_level':len(actual),'closed':set(declared)==actual,'added':sorted(set(declared)-{x['source']for x in old_manifest}),'removed':sorted({x['source']for x in old_manifest}-set(declared)),'new_examples':new_examples},'registry_changes':registry_changes,'claims':claim_rows,'claim_drift':[x for x in findings if x['kind'].startswith('claim')],'prior_input_identity_reused':reused,'unpublished_root_only_differences_to_preserve_not_overwrite':root_only,'findings':findings,'input_manifest':{'path':str(f),'sha256':H(f.read_bytes())},'review_script_sha256':H(Path(__file__).read_bytes()),'root_head_unchanged':head_before==head_after,'newly_executed_repository_gates':0,'manual_scope_review_pending':True}
f=D/'review.json';f.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'path':str(f),'sha256':H(f.read_bytes()),'counts':counts,'program_inventory':result['program_inventory'],'source_listings':result['source_listings'],'fresh_listings':len(result['fresh_source_listings']),'fresh_links':len(checked_links),'claims':len(claim_rows),'root_only_differences':root_only,'findings':findings},indent=2))

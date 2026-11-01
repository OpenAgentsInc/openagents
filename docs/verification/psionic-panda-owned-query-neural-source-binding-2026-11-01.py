from pathlib import Path
import json,hashlib,re,subprocess
root=Path.cwd();out=Path('/tmp/panda-neural-source-token-binding-first');out.mkdir(exist_ok=False)
h=lambda b:hashlib.sha256(b).hexdigest();oldref='710bb6ba0f'
def old(p):return subprocess.check_output(['git','show',oldref+':'+p])
def declarations(data):
 s=data.decode();result={}
 for m in re.finditer(r'^([A-Za-z_][A-Za-z_0-9]*)\s*::',s,re.M):
  name=m.group(1);at=s.find('{',m.end());assert at>=0,name;i=at;depth=0;quote=False;escape=False;comment=False
  while i<len(s):
   c=s[i]
   if comment:
    if c=='\n':comment=False
   elif quote:
    if escape:escape=False
    elif c=='\\':escape=True
    elif c=='"':quote=False
   elif c=='"':quote=True
   elif c=='/'and i+1<len(s)and s[i+1]=='/':comment=True;i+=1
   elif c=='{':depth+=1
   elif c=='}':
    depth-=1
    if depth==0:break
   i+=1
  assert depth==0,name;assert name not in result,name;result[name]=s[m.start():i+1].encode()
 return result
fp='coax/lib/proto/fingerprint.coax';stream='coax/lib/crypto/sha256.coax';kernel='coax/lib/crypto/sha256-kernel.coax';od=declarations(old(fp));nf=declarations(Path(fp).read_bytes());nk=declarations(Path(kernel).read_bytes());combined={**nf,**nk};assert not set(nf)&set(nk);assert set(combined)==set(od),(set(od)-set(combined),set(combined)-set(od));different=[n for n in od if od[n]!=combined[n]]
def tokens(data):
 text=data.decode();pattern=re.compile(r'(?P<trivia>\s+|//[^\n]*|/\*[\s\S]*?\*/)|(?P<quoted>"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\')|(?P<word>[A-Za-z_][A-Za-z_0-9]*)|(?P<number>0[xX][0-9a-fA-F_]+|[0-9][A-Za-z_0-9]*(?:\.[0-9][A-Za-z_0-9]*)?(?:[eE][+-]?[0-9]+)?)|(?P<operator>\.\.<|<<=|>>=|::|->|==|!=|<=|>=|&&|\|\||\+=|-=|\*=|/=|%=|&=|\|=|\^=|<<|>>|\.\.|.)');return [m.group()for m in pattern.finditer(text)if m.lastgroup!='trivia']
token_differences=[n for n in od if tokens(od[n])!=tokens(combined[n])]

a=old(stream);b=Path(stream).read_bytes();taila=a[a.index(b'#authority;'):];tailb=b[b.index(b'#authority;'):];e=json.load(open('psionic/fixtures/robotics/panda-owned-query-neural/experiment.json'));bad=[p for p,d in e['source125_sha256'].items()if h(Path(p).read_bytes())!=d]
r={'schema':'psionic.panda-owned-query-neural.source-relocation-binding.v1','passed':not token_differences and taila==tailb,'scope':'Post-execution main migration only. Original executed binary/source125/context receipts remain unchanged; no current-source fullbyte identity claim. First raw declaration comparison failed on moved SHA formatting. New binding compares lexical token spellings/order while removing ONLY whitespace/comments; identifiers, numeric spellings, string/character literals and operators are retained. No semantic/numeric equivalence waiver. No executable/model/gate run.','first_byte_only_current_tree_check':{'passed':False,'mismatches':bad,'cause':'Main commit3580010f7a moved SHA kernel after the one-shot runtime completed.'},'executed_source_baseline':oldref,'main_tree_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'old_fingerprint_sha256':h(old(fp)),'current_fingerprint_sha256':h(Path(fp).read_bytes()),'new_kernel_sha256':h(Path(kernel).read_bytes()),'streaming_authority_and_all_remaining_bytes_exact':taila==tailb,'old_streaming_sha256':h(a),'current_streaming_sha256':h(b),'streaming_prefix_change':'Comments and #load ../proto/fingerprint.coax replaced by #load sha256-kernel.coax; authority section and complete remaining implementation/check bytes identical.','all_declaration_names_exact':True,'preserved_first_raw_binding':{'path':'/tmp/panda-neural-source-relocation-first/record.json','sha256':h(Path('/tmp/panda-neural-source-relocation-first/record.json').read_bytes())},'token_differences':token_differences,'tokenizer_script_sha256':h(Path(__file__).read_bytes()),'raw_declarations':[{'name':n,'old_sha256':h(od[n]),'current_sha256':h(combined[n]),'current_file':kernel if n in nk else fp,'raw_exact':od[n]==combined[n],'token_exact':tokens(od[n])==tokens(combined[n]),'token_count':len(tokens(od[n]))}for n in sorted(od)],'differing_declarations':different,'unchanged_full_files':125-len(bad),'limits':'This is a raw source relocation/function binding for the versioned already-executed profile; it is not a fresh compiler/bootstrap/native/NN replay or claim that all current125 files retain old hashes.'}
(out/'record.json').write_text(json.dumps(r,indent=2)+'\n');assert r['passed'],token_differences;assert bad==[stream,fp]or set(bad)=={stream,fp};print(json.dumps({'passed':True,'token_exact_declarations':len(od),'unchanged_whole_files':r['unchanged_full_files'],'new_kernel_declarations':len(nk)}))

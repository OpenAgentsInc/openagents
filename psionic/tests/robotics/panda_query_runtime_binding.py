#!/usr/bin/env python3
"""Exact old-body/global/type binding plus one verified private symbol rename."""
from pathlib import Path
import argparse,hashlib,json,re
from panda_query_runtime_wiring import tokens,TOKEN
ROOT=Path(__file__).resolve().parents[3]
OLD='coax_q3p_CoaxSpatialOp__o34a146b277r146'
NEW='coax_q3p_CoaxSpatialOp__o34a147b278r147'
ADDED={'pqn_file_chunk_bytes','pqn_file_request','pqn_file_plan','coax_p_pqn_hash_sized','pqn_file_hash','coax_show_t_PqhPlan'}
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def digest(value):return hashlib.sha256(json.dumps(value,separators=(',',':'),sort_keys=True).encode()).hexdigest()
def functions(text):
    # Mask directives/continuations without shifting source offsets. Macro
    # bodies remain in the separately compared non-function preamble.
    lines=[];continued=False
    for line in text.splitlines(keepends=True):
        directive=continued or line.lstrip().startswith('#');continued=directive and line.rstrip().endswith('\\');lines.append(re.sub(r'[^\n]',' ',line)if directive else line)
    masked=''.join(lines);matches=[m for m in TOKEN.finditer(masked)if not(m.group().isspace()or m.group().startswith('//')or m.group().startswith('/*'))];t=[m.group()for m in matches];found={};spans=[];i=0
    while i<len(t):
        if t[i]=='{':
            end=i+1;depth=1
            while depth:depth+=(t[end]=='{')-(t[end]=='}');end+=1
            if i and t[i-1]==')':
                k=i-2;n=1
                while n:n+=(t[k]==')')-(t[k]=='(');k-=1
                name=t[k];start=text.rfind('\n',0,matches[k].start())+1;stop=matches[end-1].end();key=name;ordinal=2
                while key in found:key=name+'#'+str(ordinal);ordinal+=1
                found[key]={'body':t[i:end],'signature':tokens(text[start:matches[i].start()])};spans.append((start,stop))
            i=end
        else:i+=1
    global_text=text
    for start,stop in reversed(spans):global_text=global_text[:start]+''.join('\n'if c=='\n'else' 'for c in global_text[start:stop])+global_text[stop:]
    return found,global_text
def remove_added(text):
    out=[];skip=0
    for line in text.splitlines(keepends=True):
        s=line.strip()
        if s=='#ifdef COAX_coax_t_PqhPlan_DEFINED':skip=1;continue
        if skip:
            if s.startswith(('#if ','#ifdef ','#ifndef ')):skip+=1
            if s=='#endif':skip-=1
            continue
        if '"psionic/tests/robotics/panda-query-runtime-hash.coax"'in line:continue
        if 'coax_t_PqhPlan'in line and (s.startswith(('typedef ','struct ','COAX_LAYOUT('))):continue
        if any(re.search(r'\b'+name+r'\s*\(',line)for name in ADDED)and s.endswith(';'):continue
        out.append(line)
    require(skip==0,'new type guard complete');return ''.join(out)
def renames(ts):return [NEW if x==OLD else x for x in ts]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--old',type=Path,required=True);p.add_argument('--hash-proof',type=Path,required=True);p.add_argument('--current',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();require(not a.output.exists(),'preserve binding');a.output.mkdir(parents=True);r={'schema':'psionic.panda-query-runtime.current-binding.v1','passed':False,'normalization':'One private identifier rename only; no numeric, diagnostic, string or expression normalization.','private_identifier':{'old':OLD,'new':NEW},'modes':[]}
    try:
        for mode in ['debug','release']:
            old=a.old/mode/'panda-session.c';current=a.current/mode/'panda-session.c';hashed=a.hash_proof/mode/'panda-session.c';before,old_global=functions(old.read_text());after,current_global=functions(current.read_text());hash_functions,_=functions(hashed.read_text());require(set(after)-{NEW}==set(before)-{OLD}|ADDED,'exact added function inventory')
            rows=[]
            for name,v in before.items():
                key=NEW if name==OLD else name;w=after[key];require(renames(v['signature'])==w['signature']and renames(v['body'])==w['body'],('old body/signature',mode,name));rows.append((key,digest(w)))
            for name in ADDED:require(after[name]==hash_functions[name],('new hash body/signature',mode,name))
            require(renames(tokens(old_global))==tokens(remove_added(current_global)),('all old non-function C globals/preamble',mode))
            oh=a.old/mode/'panda-session.h';nh=a.current/mode/'panda-session.h';require(tokens(oh.read_text())==tokens(remove_added(nh.read_text())),('all old header declarations/layouts',mode));r['modes'].append({'mode':mode,'old_C_sha256':sha(old),'current_C_sha256':sha(current),'old_header_sha256':sha(oh),'current_header_sha256':sha(nh),'old_functions':len(before),'current_functions':len(after),'exact_bound_function_inventory_sha256':digest(sorted(rows)),'new_functions':sorted(ADDED),'all_old_globals_and_preamble_exact':True,'all_old_header_declarations_and_layouts_exact':True,'private_helper_signature_old':before[OLD]['signature'],'private_helper_signature_new':after[NEW]['signature']})
        helper=ROOT/'psionic/tests/robotics/panda_query_runtime.h';current=helper.read_text();old=current.replace('if(pqn_file_request((int64_t)n,32,0)){fclose(f);return 0;}','').replace('return !pqn_file_hash(','return !ps_digest(');require(hashlib.sha256(old.encode()).hexdigest()=='a526d091766774644d70977e6f1c4b4ab16dea63958df7fb09c92ecf48a99c47','exact historical root mock helper');bf,_=functions(old);af,_=functions(current)
        for name in ['panda_query_current_v1','panda_query_control_v1']:require(bf[name]==af[name],('control helper exact',name))
        r['root_mock_carry']={'historical_helper_sha256':hashlib.sha256(old.encode()).hexdigest(),'current_helper_sha256':sha(helper),'exact_control_functions':['panda_query_current_v1','panda_query_control_v1'],'changed_helpers_only':['panda_query_read_v1: call admitted16MiB length before allocation','panda_query_hash_v1: call Coaxstreaming wrapper']};r['passed']=True
    except BaseException as e:r['failure']=repr(e);raise
    finally:(a.output/'record.json').write_text(json.dumps(r,indent=2)+'\n')
    print(json.dumps({'passed':True,'record':str(a.output/'record.json')}))
if __name__=='__main__':main()

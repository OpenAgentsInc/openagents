"""Mechanical historical eight-slot oracle transport into twenty-four-slot carriers.

The original literal oracle and its identity stay unchanged. New inactive slots
are explicit zeroes; this adapter never computes expected lifetime policy.
"""
import hashlib
import json
from pathlib import Path

ORACLE=Path(__file__).with_name('retain-policy-oracle.json')
ORIGINAL_SHA256='b5856aaa2839b0e94fa493302d12aa880d7aa751e161b3a00a6408adb6e73ab2'
API_SHA256='c7b16606c4768ae5c33f5d24a9418737f89f55a2dacccfc0673f0e7d716268d5'

def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def integer(value):
    if type(value)is not int or not -(1<<63)<=value<(1<<63):raise ValueError('literal integer outside int64')
    if value==-(1<<63):return 'INT64_MIN'
    return ('(-INT64_C('+str(-value)+'))') if value<0 else 'INT64_C('+str(value)+')'

def bit(value):
    if type(value)is not bool:raise ValueError('expected literal bool')
    return '1' if value else '0'

def exact(row,keys):
    if type(row)is not dict or set(row)!=set(keys):raise ValueError('literal carrier keys differ')

def origin(row):
    exact(row,('kind','root','role','type','path'));path=row['path'];exact(path,('count','steps'))
    if len(path['steps'])!=8:raise ValueError('all eight path slots must be explicit')
    steps=[]
    for step in path['steps']:
        exact(step,('kind','index'));steps.append('{.kind='+integer(step['kind'])+',.index='+integer(step['index'])+'}')
    steps += ['{.kind=0,.index=0}'] * 16
    return '{.kind='+integer(row['kind'])+',.root='+integer(row['root'])+',.role='+integer(row['role'])+',.type='+integer(row['type'])+',.path={.count='+integer(path['count'])+',.steps={.e={'+','.join(steps)+'}}}}'

def term(row):
    exact(row,('frame','origin'));return '{.frame='+integer(row['frame'])+',.origin='+origin(row['origin'])+'}'

def address(row):
    exact(row,('storage','value_type'));return '{.storage='+term(row['storage'])+',.value_type='+integer(row['value_type'])+'}'

def obligation(row):
    exact(row,('source','destination'));return '{.source='+term(row['source'])+',.destination='+address(row['destination'])+'}'

def array(kind,name,items,encode,count):
    if type(items)is not list:raise ValueError('literal backing must be a list')
    if type(count)is not int:raise ValueError('literal count override must be int')
    # Admitted-size positive views have actual backing; invalid extents are
    # transported with valid bounded backing and refused only by Coax.
    if 0<=count<=65 and count>len(items):raise ValueError('positive span exceeds supplied backing')
    extent=max(1,len(items))
    return 'static '+kind+' '+name+'['+str(extent)+']={'+','.join(encode(row)for row in items)+'};' if items else 'static '+kind+' '+name+'[1]={0};'

def generate(output):
    output=Path(output);before=digest(ORACLE);data=json.loads(ORACLE.read_text())
    if data['original_oracle']['sha256']!=ORIGINAL_SHA256 or data['api_sha256']!=API_SHA256:raise ValueError('unreviewed literal oracle identity')
    checks=data['check_cases'];bounds=data['bounds_cases']
    if len(checks)!=112 or len(bounds)!=16 or len({row['id']for row in checks})!=112:raise ValueError('literal vector inventory changed')
    lines=['/* Literal inputs and literal expected fields, no policy evaluation. */','static void independent_vectors(void){']
    for case in checks:
        inp=case['input'];expected=case['expected'];rows=case['expected_rows']
        required={'frame','value_type','sources','source_complete','destinations','destination_complete','output_count'}
        if not required<=set(inp) or set(inp)-required-{'source_count_override','destination_count_override'}:raise ValueError('unrecognized literal inputs')
        exact(expected,('status','facts_complete','relation_complete','forbidden','obligation_count','bad_source','bad_destination'))
        if len(rows)!=expected['obligation_count']:raise ValueError('literal expected row count mismatch')
        ns=inp.get('source_count_override',len(inp['sources']));nd=inp.get('destination_count_override',len(inp['destinations']))
        lines+=['{ /* '+case['id'].replace('*/','')+' */',array('Term','sources',inp['sources'],term,ns),array('Address','destinations',inp['destinations'],address,nd),array('Obligation','expected',rows,obligation,len(rows))]
        args=[integer(inp['frame']),integer(inp['value_type']),'sources',integer(ns),bit(inp['source_complete']),'destinations',integer(nd),bit(inp['destination_complete']),integer(inp['output_count']),integer(expected['status']),bit(expected['facts_complete']),bit(expected['relation_complete']),bit(expected['forbidden']),integer(expected['bad_source']),integer(expected['bad_destination']),'expected',integer(expected['obligation_count'])]
        lines+=['check('+','.join(args)+');','independent_check_cases++;','}']
    for row in bounds:
        exact(row,('counts','expected_status'))
        if len(row['counts'])!=3:raise ValueError('bounds literal arity')
        lines+=['assert(RP_bounds('+','.join(integer(x)for x in row['counts'])+')=='+integer(row['expected_status'])+');','cases++;calls++;independent_bounds_cases++;']
    lines+=['}']
    output.write_text('\n'.join(lines)+'\n')
    if digest(ORACLE)!=before:raise ValueError('oracle changed during encoding')
    return {'schema':'coax.retain-policy.literal-vector-transport.v2','oracle':str(ORACLE),'oracle_sha256':before,'original_oracle_sha256':ORIGINAL_SHA256,'adapter_sha256':digest(__file__),'generated_include':str(output),'generated_include_sha256':digest(output),'check_cases':len(checks),'bounds_cases':len(bounds),'case_ids':[row['id']for row in checks],'scope':'Mechanical literal input/expected carrier encoding only. No expected relations, statuses, obligations or witnesses are calculated. All path slots and named fields are compared; no C aggregate-padding contract.'}

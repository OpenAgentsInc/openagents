#!/usr/bin/env python3
"""Effects-only instrumentation of retained generated Coax stage counters.

The wrappers return the original Coax value unchanged. No shader, budget,
resource, pass, geometry or numeric decision is replaced by this profiler.
"""
import argparse,json,re,time,signal
from pathlib import Path
from check_pipeline import ROOT,HERE,sha,run_process,interrupted

def instrument(text,pixels=(),fragments=False,extent=False):
 wrappers=[]
 for kind,tag,name in [('vertex','RRVertices','rr_vertices'),('fragment','RRShaded','rr_shade')]:
  pattern=re.compile(r'^static coax_t_'+tag+r' (coax_p_'+name+r')\(([^\n]*)\) \{',re.M)
  matches=list(pattern.finditer(text))
  if len(matches)!=1:raise ValueError('expected one generated '+name+' definition')
  m=matches[0];args=m[2];names=[part.strip().split()[-1]for part in args.split(',')];original='rfc_original_'+name
  text=text[:m.start()]+m[0].replace(m[1]+'(',original+'(',1)+text[m.end():]
  slot=0 if kind=='vertex'else 1
  wrappers.append('static coax_t_'+tag+' '+m[1]+'('+args+') {coax_t_'+tag+' q='+original+'('+','.join(names)+');rfc_note(l_tree.data,'+str(slot)+',q.steps);return q;}')
 trailer='''
static const uint32_t *rfc_trees[14];
static int64_t rfc_indices[14],rfc_count,rfc_steps[14][2],rfc_calls[14][2];
void rfc_register(const uint32_t *tree,int64_t index){assert(rfc_count<14);rfc_trees[rfc_count]=tree;rfc_indices[rfc_count++]=index;}
static void rfc_note(const uint32_t *tree,int kind,int64_t steps){for(int64_t k=0;k<rfc_count;k++)if(tree==rfc_trees[k]){assert(steps>=0);rfc_steps[k][kind]+=steps;rfc_calls[k][kind]++;return;}abort();}
void rfc_report(void){for(int64_t k=0;k<rfc_count;k++)fprintf(stderr,"RF_COST {\\"entry\\":%lld,\\"vertex_steps\\":%lld,\\"fragment_steps\\":%lld,\\"vertex_groups\\":%lld,\\"fragment_groups\\":%lld}\\n",(long long)rfc_indices[k],(long long)rfc_steps[k][0],(long long)rfc_steps[k][1],(long long)rfc_calls[k][0],(long long)rfc_calls[k][1]);}
'''
 if pixels:
  pattern=re.compile(r'^static coax_t_RFResult (coax_p_rf_execute_draw)\(([^\n]*)\) \{',re.M);matches=list(pattern.finditer(text))
  if len(matches)!=1:raise ValueError('expected one generated frame draw definition')
  m=matches[0];args=m[2];names=[part.strip().split()[-1]for part in args.split(',')];original='rfc_original_rf_execute_draw'
  text=text[:m.start()]+m[0].replace(m[1]+'(',original+'(',1)+text[m.end():]
  points=','.join('{'+str(x)+','+str(y)+'}'for x,y in pixels)
  trailer+='static const int64_t rfc_points['+str(len(pixels))+'][2]={'+points+'};\n'
  trailer+='static void rfc_pixels(int64_t entry,coax_t_RFRequirements req,coax_t_RFWorking work){for(int target=1;target<=4;target++){coax_t_RFCanvasLayout c=req.images.e[target];if(!c.present)continue;for(unsigned k=0;k<sizeof(rfc_points)/sizeof(rfc_points[0]);k++){int64_t x=rfc_points[k][0],y=rfc_points[k][1];if(x>=c.descriptor.width||y>=c.descriptor.height)continue;coax_t_RQColor v=work.images.data[c.offset+y*c.descriptor.width+x];fprintf(stderr,"RF_PIXEL {\\"entry\\":%lld,\\"target\\":%d,\\"x\\":%lld,\\"y\\":%lld,\\"rgba\\":[%.9g,%.9g,%.9g,%.9g]}\\n",(long long)entry,target,(long long)x,(long long)y,(double)v.r,(double)v.g,(double)v.b,(double)v.a);}}}\n'
  wrappers.append('static coax_t_RFResult '+m[1]+'('+args+') {coax_t_RFResult q='+original+'('+','.join(names)+');rfc_pixels(l_index,l_req,l_work);return q;}')
 if fragments:
  if not pixels:raise ValueError('fragment diagnostics require selected pixels')
  trailer+=(HERE/'reference-fragment-probe.h').read_text()
  for k,wrapper in enumerate(wrappers):
   if 'coax_p_rr_vertices('in wrapper:wrapper=wrapper.replace('return q;', 'rfc_last_vertices=q;rfc_last_ids=l_ids;rfc_last_instance=l_instance;rfc_last_words=l_r.layout.count;return q;')
   if 'coax_p_rr_shade('in wrapper:wrapper=wrapper.replace('return q;', 'rfc_last_quad=l_quad;rfc_last_mask=q.publish;rfc_next_lane=0;return q;')
   if 'coax_p_rf_execute_draw('in wrapper:wrapper=wrapper.replace('{coax_t_RFResult q=', '{rfc_last_entry=l_index;coax_t_RFResult q=')
   wrappers[k]=wrapper
  pattern=re.compile(r'^static coax_t_RRStored (coax_p_rr_store)\(([^\n]*)\) \{',re.M);matches=list(pattern.finditer(text))
  if len(matches)!=1:raise ValueError('expected one generated store definition')
  m=matches[0];args=m[2];names=[part.strip().split()[-1]for part in args.split(',')];original='rfc_original_rr_store';text=text[:m.start()]+m[0].replace(m[1]+'(',original+'(',1)+text[m.end():]
  wrappers.append('static coax_t_RRStored '+m[1]+'('+args+') {coax_t_RRStored q='+original+'('+','.join(names)+');rfc_fragment(q,l_old,l_state,l_colour,l_depth);return q;}')
 if extent:
  pattern=re.compile(r'^static coax_t_RRTriangle (coax_p_rr_triangle_profile)\(([^\n]*)\) \{',re.M);matches=list(pattern.finditer(text))
  if len(matches)!=1:raise ValueError('expected one generated triangle definition')
  m=matches[0];args=m[2];names=[part.strip().split()[-1]for part in args.split(',')];original='rfc_original_rr_triangle_profile';text=text[:m.start()]+m[0].replace(m[1]+'(',original+'(',1)+text[m.end():]
  trailer+='static int rfc_extent_seen;static double rfc_minx,rfc_maxx,rfc_miny,rfc_maxy,rfc_excess;static long long rfc_setups;\n'
  trailer+='static void rfc_extent(coax_t_RRTriangle q){if(q.status!=COAX_RQStatus_ok)return;rfc_setups++;for(int k=0;k<3;k++){double x=q.vertices.e[k].x,y=q.vertices.e[k].y;if(!rfc_extent_seen){rfc_minx=rfc_maxx=x;rfc_miny=rfc_maxy=y;rfc_extent_seen=1;}if(x<rfc_minx)rfc_minx=x;if(x>rfc_maxx)rfc_maxx=x;if(y<rfc_miny)rfc_miny=y;if(y>rfc_maxy)rfc_maxy=y;double e[4]={-x,x-q.width,-y,y-q.height};for(int j=0;j<4;j++)if(e[j]>rfc_excess)rfc_excess=e[j];}}\n'
  wrappers.append('static coax_t_RRTriangle '+m[1]+'('+args+') {coax_t_RRTriangle q='+original+'('+','.join(names)+');rfc_extent(q);return q;}')
  report='fprintf(stderr,"RF_EXTENT {\\"min_x\\":%.17g,\\"max_x\\":%.17g,\\"min_y\\":%.17g,\\"max_y\\":%.17g,\\"max_excess\\":%.17g,\\"setups\\":%lld}\\n",rfc_minx,rfc_maxx,rfc_miny,rfc_maxy,rfc_excess,rfc_setups);'
  # Define the observer before report, then append its record to that effect.
  report_at=trailer.index('void rfc_report(void)');ext_at=trailer.index('static int rfc_extent_seen;');observer=trailer[ext_at:];trailer=trailer[:ext_at];report_end=trailer.index('\n',report_at)
  trailer=observer+trailer[:report_end-1]+report+trailer[report_end-1:]
 return text+'\n#include <assert.h>\n#include <stdio.h>\n'+trailer+'\n'+'\n'.join(wrappers)+'\n' 

def render_command(measurement,executable):
 runs=[cmd for cmd in measurement['commands']if Path(cmd[0]).resolve()==executable.resolve()]
 if len(runs)!=1:raise ValueError('receipt must contain exactly one renderer execution')
 return list(runs[0])

def main():
 signal.signal(signal.SIGTERM,interrupted)
 p=argparse.ArgumentParser();p.add_argument('--measurement',required=True,type=Path);p.add_argument('--work-dir',required=True,type=Path);p.add_argument('--pixels',nargs='*',default=[]);p.add_argument('--fragments',action='store_true');p.add_argument('--extent',action='store_true');a=p.parse_args();start=time.monotonic();a.work_dir.mkdir(parents=True,exist_ok=True)
 tool_before=sha(Path(__file__));measurement_before=sha(a.measurement);m=json.loads(a.measurement.read_text());assert m['status']in['rendered','refused']and not m['source_changes']and not m['artifact_changes']and not m['input_changes']
 artifacts={k:Path(m[k+'_path'])for k in ['generated_c','generated_header','executable']}
 for k,path in artifacts.items():assert sha(path)==m[k+'_sha256'],k
 driver=HERE/'reference-frame-native.c';assert sha(driver)==m['source_sha256']['aethyr/render/tests/reference-frame-native.c']
 for name,digest in m['input_sha256'].items():assert sha(Path(name))==digest,name
 source=a.work_dir/'profiled.c';pixels=[tuple(map(int,item.split(',')))for item in a.pixels];assert all(len(q)==2 and min(q)>=0 for q in pixels);source.write_text(instrument(artifacts['generated_c'].read_text(),pixels,a.fragments,a.extent))
 body=driver.read_text();body='#include <stdint.h>\nvoid rfc_register(const uint32_t *,int64_t);\nvoid rfc_report(void);\n'+body
 needle=' coax_t_RFRequirements req=';assert body.count(needle)==1;body=body.replace(needle,' for(int64_t k=0;k<programs_count;k++)rfc_register(programs[k].tree.data,programs[k].index);\n'+needle)
 needle=' assert(out[pixels].r==91';assert body.count(needle)==1;body=body.replace(needle,' rfc_report();\n'+needle);host=a.work_dir/'driver.c';host.write_text(body);exe=a.work_dir/'profiled';commands=[]
 def run(argv):
  commands.append(list(map(str,argv)));return run_process(argv,timeout=max(1,830-(time.monotonic()-start)))
 out,err=run(['clang','-std=c11','-O2','-ffp-contract=off','-I',artifacts['generated_header'].parent,'-I',HERE,source,host,'-lm','-o',exe]);(a.work_dir/'compile.log').write_bytes(out+err)
 fenced={str(p):sha(p)for p in [source,host,artifacts['generated_header'],exe,driver]};argv=render_command(m,artifacts['executable']);argv[0]=str(exe);argv[2]=str(a.work_dir/'screen.rgba32f');argv[3]=str(a.work_dir/'frame.json');out,err=run(argv);(a.work_dir/'execution.log').write_bytes(out+err)
 result=json.loads((a.work_dir/'frame.json').read_text());assert result==m['result'],'instrumentation changed a Coax receipt';assert sha(a.work_dir/'screen.rgba32f')==m['screen_sha256'],'instrumentation changed pixels'
 stages=[json.loads(line[len('RF_COST '):])for line in err.decode().splitlines()if line.startswith('RF_COST ')];observed=sum(x['vertex_steps']+x['fragment_steps']for x in stages);assert 0<observed<=result['shader_steps']
 for name,digest in m['input_sha256'].items():assert sha(Path(name))==digest,name
 assert tool_before==sha(Path(__file__))and measurement_before==sha(a.measurement),'profiler source/receipt changed'
 for name,digest in fenced.items():assert sha(Path(name))==digest,name
 for k,path in artifacts.items():assert sha(path)==m[k+'_sha256'],k
 pixel_records=[json.loads(line[len('RF_PIXEL '):])for line in err.decode().splitlines()if line.startswith('RF_PIXEL ')]
 fragment_records=[json.loads(line[len('RF_FRAGMENT '):])for line in err.decode().splitlines()if line.startswith('RF_FRAGMENT ')]
 extent_records=[json.loads(line[len('RF_EXTENT '):])for line in err.decode().splitlines()if line.startswith('RF_EXTENT ')]
 record=dict(extent_records=extent_records,fragment_records=fragment_records,pixel_records=pixel_records,tool_sha256=tool_before,measurement_sha256_before=measurement_before,profile_source_fences=fenced,schema='aethyr.render.generated-stage-profile.v1',status='pass',scope='Unchanged retained generated-Coax values with effects-only stage-counter logging; result and image byte identity checked. Not a new shader/profile admission or GPU numerical qualification.',measurement_sha256=sha(a.measurement),compiler_sha256=m['compiler_sha256'],generated_c_sha256=m['generated_c_sha256'],profiled_c_sha256=sha(source),driver_sha256=sha(host),stages=stages,result=result,unattributed_steps=result['shader_steps']-observed,commands=commands,elapsed_seconds=time.monotonic()-start)
 (a.work_dir/'profile.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record['stages']))
if __name__=='__main__':main()

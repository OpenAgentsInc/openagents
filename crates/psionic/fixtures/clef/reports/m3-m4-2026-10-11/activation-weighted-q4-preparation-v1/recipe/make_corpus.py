#!/usr/bin/env python3
"""Author distinct calibration and development judgments without other datasets."""
import argparse
import hashlib
import json
from pathlib import Path


def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()
def choice(instructions,criteria):return {'type':'choice','instructions':instructions,'criteria':criteria}
def noul(instructions):return {'type':'noul','instructions':instructions}
def score(instructions,criteria):return {'type':'score','instructions':instructions,'criteria':criteria}
def body(state,questions):return {'model':'clef','state':state,'questions':questions}


def document_topic(i):
    reports=[
        'The orchard team replaced a cracked irrigation hose and tested the repaired connection. No leak remained during the test.',
        'Volunteers counted the pears, separated bruised fruit, and packed the sound fruit into baskets for collection.',
        'The coordinator moved the autumn gathering to Saturday and asked speakers to confirm their arrival times.',
        'The shared tool cupboard now has labeled shelves. Returned items must be dry before storage.',
    ]
    state={'document':reports[i%4],'footer':f'Field bulletin number {100+i}. Written for the volunteer handover.',
           'additional_notes':['Use the latest signed bulletin.','Questions go to the listed coordinator.'][:1+i%2]}
    return body(state,{'topic':choice('What is the main topic of `document`?',{'water_system':'Repair or operation of irrigation equipment.','produce':'Sorting or handling harvested fruit.','event':'Arrangements for a gathering.','storage':'Where and how tools are stored.','other':'None of these topics.'}),
                       'completed_test':noul('Does `document` explicitly report a completed equipment test?')})


def stock_matching(i):
    colors=['amber','violet','white'];sizes=['small','large'];materials=['glass','steel']
    items=[{'id':f'item-{j+1}','color':colors[j%3],'size':sizes[j%2],'material':materials[(j//2)%2],'available':3+j} for j in range(6)]
    wanted=items[i%6].copy();wanted.pop('available');wanted.pop('id')
    if i>=6:wanted['material']='ceramic'
    return body({'inventory':items,'requested':wanted,'requested_quantity':1+i%8},
                {'matching_item':choice('Which inventory item matches every attribute in `requested`? Ignore quantity for this question.',{**{x['id']:x for x in items},'none':'No listed item matches every requested attribute.'}),
                 'enough':noul('Is there a single matching inventory item whose available quantity meets `requested_quantity`?')})


def code_behavior(i):
    n=i%5;limit=2+i%3
    code=f'fn mark(n: u32) -> &\'static str {{ if n == 0 {{ "empty" }} else if n < {limit} {{ "short" }} else {{ "ready" }} }}'
    state={'language':'Rust','code':code,'call':f'mark({n})','note':'This function does not mutate external state.'}
    questions={'returned':choice('Which string does the shown call return?',{'empty':'The string empty.','short':'The string short.','ready':'The string ready.','invalid':'The shown call cannot be evaluated as written.'})}
    if i%2:questions['mutation']=noul('Does executing the shown function change a caller-owned collection or write a file?')
    return body(state,questions)


def message_intent(i):
    messages=[
        'Could you explain which entrance the visiting choir should use? The map has two arrows.',
        'We will bring the music stands at noon as agreed. No changes are needed.',
        'We cannot attend the practice this week. Please remove our names from the list.',
        'Can the practice move to Thursday? Our usual room is unavailable on Tuesday.',
        'Thank you for lending the keyboard. It made the rehearsal possible.',
        'The north door rattles when the wind picks up. I noticed it during setup.',
    ]
    return body({'message':messages[i%6],'sender_role':'choir volunteer','previous_context':'The group is arranging a local rehearsal.'},
                {'intent':choice('What is the sender primarily doing in `message`?',{'clarify':'Asking for an explanation or missing information.','confirm':'Confirming an existing arrangement.','cancel':'Declining attendance or cancelling participation.','reschedule':'Requesting a different time.','thank':'Expressing thanks.','report':'Reporting an observation without making one of the other requests.'}),
                 'explicit_time':noul('Does the message name a day or clock time?')})


def schedule_conflict(i):
    start=9+i%4;other=10+(i*2)%4
    state={'rule':'One room can hold only one session at a time. An end time equal to the next start time is allowed.',
           'bookings':[{'name':'reading circle','room':'Cedar','start_hour':start,'end_hour':start+2},
                       {'name':'map workshop','room':'Cedar' if i%3 else 'Birch','start_hour':other,'end_hour':other+1}],
           'flexibility':'Either session may move to the next day.' if i%2 else 'Neither session may move.'}
    return body(state,{'collision':choice('Which statement describes a scheduling conflict under `rule`?',{'overlap':'The sessions use the same room during overlapping hours.','separate':'The sessions use different rooms or nonoverlapping hours.','unknown':'The supplied details are insufficient.'}),
                       'flexibility':score('How much permission to reschedule is explicitly stated?', ['Neither session may move.','Exactly one session may move.','Either session may move.'])})


def instruction_coverage(i):
    steps=['disconnect the power','wipe the outer case','check the cable for damage','record the inspection date']
    omitted=i%5;completed=[s for j,s in enumerate(steps) if j!=omitted]
    return body({'instructions':steps,'work_log':completed,'note':f'Bench inspection ticket {200+i}. The work log lists only completed actions.'},
                {'missing':choice('Which required action is absent from `work_log`?',{**{f'step-{j+1}':s for j,s in enumerate(steps)},'none':'All required actions are listed.'}),
                 'coverage':score('How much of the required checklist does the work log explicitly cover?', ['No required actions.','Some, but fewer than half of the required actions.','At least half, but not all required actions.','Every required action.'])})


def passage_relevance(i):
    passages=[
        'The seed packets are kept in a cool cabinet. Each envelope lists its variety and collection year.',
        'Visitors enter the garden through the west gate. Wheelchair access uses the paved route beside the shed.',
        'The greenhouse fans run during warm afternoons. A wall switch disables them during maintenance.',
    ]
    queries=['Where are seeds stored?','How does a visitor enter the garden?','How can staff stop the greenhouse fans?','When is the next garden concert?']
    return body({'query':queries[i%4],'passages':{f'p{j+1}':s for j,s in enumerate(passages)},'context':'Choose evidence from the supplied passages only.'},
                {'evidence':choice('Which passage most directly answers `query`?',{'p1':'The first passage.','p2':'The second passage.','p3':'The third passage.','none':'None of the passages supplies the requested information.'})})


def work_readiness(i):
    statuses=[('available','passed','not started'),('missing','passed','not started'),('available','pending','not started'),('available','passed','in progress'),('available','passed','finished'),('unknown','unknown','unknown')]
    material,inspection,progress=statuses[i%6]
    return body({'work':'print posters','paper_stock':material,'roller_inspection':inspection,'progress':progress,
                 'policy':'Printing may begin only when paper is available and the roller inspection has passed.',
                 'handover_notes':['Store unused paper flat.','Keep the finished posters separate from draft sheets.','Record the number of prints.'][:1+i%3]},
                {'ready':noul('Do the stated material and inspection conditions permit printing under `policy`?'),
                 'phase':choice('What is the explicitly reported work phase?',{'pending':'Work has not started.','active':'Work is in progress.','done':'Work is finished.','unknown':'The phase is unknown.'})})


# Development judgments are new templates, not noun substitutions of calibration.
def dependency_order(i):
    done=['measure'] if i%3==0 else (['measure','cut'] if i%3==1 else [])
    return body({'tasks':[{'name':'measure','needs':[]},{'name':'cut','needs':['measure']},{'name':'assemble','needs':['cut']}],
                 'completed':done,'rule':'Choose an unfinished task only when every prerequisite is completed.'},
                {'next':choice('Which unfinished task is eligible next under the dependency rule?',{'measure':'Measure the parts.','cut':'Cut the measured parts.','assemble':'Assemble the cut parts.','none':'No unfinished task is eligible.'}),
                 'assembly_allowed':noul('Are all prerequisites of assemble listed in `completed`?')})


def route_endpoint(i):
    routes=[['north','east'],['south','west'],['east','west'],['north','north'],['south','east'],['west','west']]
    return body({'start':[0,0],'moves':routes[i%6],'rule':'Each north move adds1 to y; south subtracts1 from y; east adds1 to x; west subtracts1 from x.'},
                {'position':choice('Which description matches the final position relative to the start?',{'northeast':'Both x and y increased.','southwest':'Both x and y decreased.','north':'Only y increased.','west':'Only x decreased.','southeast':'x increased and y decreased.','start':'Both coordinates are unchanged.'})})


def statement_consistency(i):
    pairs=[('All display labels are printed.','The label for display C has not been printed.'),('The cabinet contains three maps.','One map is folded and two maps are flat.'),('The desk is open on Monday.','The desk is closed every Monday.'),('The curator checked one drawer.','The cabinet has four drawers.'),('No lamps are switched on.','A lamp beside the exit is switched on.'),('The guide arrives before noon.','The guide arrives at11:30.')]
    first,second=pairs[i%6]
    return body({'statement_a':first,'statement_b':second,'context':'Both statements describe the same place and time. Interpret ordinary quantities literally.'},
                {'relationship':choice('How do the two statements relate?',{'contradict':'They cannot both be true under the stated context.','compatible':'They can both be true.','unclear':'The wording is too ambiguous to decide.'})})


def audience_register(i):
    texts=[('Hey folks, bring your favorite word game and we will try it together.','friends'),('Participants must submit the completed registration form before entry.','public notice'),('An alveolar stop begins with tongue contact at the alveolar ridge.','linguistics students'),('Please bring a pencil. We will practice greeting each other in pairs.','beginners'),('Yo, that pronunciation trick was neat. Show me again?','friends'),('The assessment rubric specifies phonemic accuracy and temporal fluency.','instructors')]
    text,audience=texts[i%6]
    return body({'text':text,'intended_audience':audience},
                {'register':score('How formal is the wording of `text`?', ['Casual slang or conversational shorthand.','Plain everyday wording without slang.','Formal administrative or instructional wording.','Specialist technical wording that assumes domain knowledge.']),
                 'explained':noul('Does the text explicitly explain the meaning of a specialist term it uses?')})


CALIBRATION=[document_topic,stock_matching,code_behavior,message_intent,schedule_conflict,instruction_coverage,passage_relevance,work_readiness]
DEVELOPMENT=[dependency_order,route_endpoint,statement_consistency,audience_register]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
    a.output_dir.mkdir(parents=True,exist_ok=False);rows=[]
    for partition,families,count in [('calibration',CALIBRATION,12),('development',DEVELOPMENT,6)]:
        for factory in families:
            for i in range(count):
                request=factory(i)
                # Per-record provenance is metadata, not model input.
                rows.append({'id':f'{partition}-{factory.__name__}-{i:02d}','partition':partition,'family':factory.__name__,
                             'request':request,'request_sha256':hashlib.sha256(canonical(request)).hexdigest()})
    # Repeating a semantic variant changes no model input and would silently
    # over-weight a request. Make every record unique through relevant context.
    for row in rows:
        variant=int(row['id'].rsplit('-',1)[1])
        row['request']['state']['record_context']={'record_number':300+variant,'review_note':['Use only the supplied facts.','Treat missing information as unknown.','Do not infer that planned work has finished.'][variant%3]}
        row['request_sha256']=hashlib.sha256(canonical(row['request'])).hexdigest()
    assert len(rows)==120 and len({r['request_sha256'] for r in rows})==120
    value={'schema':'clef.independent-activation-corpus.v1','authoring':'New authored synthetic scenarios; no acceptance examples, errors, model answers, or external datasets read. Development uses new judgment templates.','records':rows}
    raw=canonical(value)+b'\n';(a.output_dir/'corpus.json').write_bytes(raw)
    manifest={'schema':'clef.independent-activation-corpus-manifest.v1','status':'authored_not_encoded_or_executed','corpus_sha256':hashlib.sha256(raw).hexdigest(),'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'calibration_records':96,'development_records':24,'calibration_families':[f.__name__ for f in CALIBRATION],'development_families':[f.__name__ for f in DEVELOPMENT],'max_tokens':1024,'seed':'fixed enumerated scenarios; no PRNG','model_outputs_used':False,'acceptance_examples_used':False,'acceptance_errors_used':False,'limitation':'Small synthetic corpus; no claim to cover the full production distribution.'}
    (a.output_dir/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps(manifest))


if __name__=='__main__':main()

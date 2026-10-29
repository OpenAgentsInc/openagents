#!/usr/bin/env python3
"""Owned HTTP/SDK oracles. Python is verification tooling, never runtime.
Run with a fixture binary + table, or --url for an already running GPU server.
An OpenAI SDK/jsonschema environment is required for the optional client checks.
"""
import argparse, concurrent.futures, http.client, json, os, secrets, socket, subprocess, time, urllib.error, urllib.request
from pathlib import Path

TOOL = {"type":"function","name":"weather","description":"Return the weather in a city.","parameters":{"type":"object","properties":{"city":{"type":"string","enum":["Paris","Chicago"]}},"required":["city"],"additionalProperties":False},"strict":False}

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--binary');ap.add_argument('--table');ap.add_argument('--url');ap.add_argument('--schema');ap.add_argument('--sdk',action='store_true');ap.add_argument('--tools',action='store_true');a=ap.parse_args()
    key=os.environ.get('PSIONIC_API_KEY') or secrets.token_hex(24);process=None
    if a.binary:
        with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
        url=f'http://127.0.0.1:{port}'
        process=subprocess.Popen([a.binary,a.table,str(port)],env={**os.environ,'PSIONIC_API_KEY':key},stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
    else:url=a.url.rstrip('/')
    def request(body=None,path='/v1/responses',auth=True,headers=None):
        h={'Content-Type':'application/json',**({'Authorization':'Bearer '+key} if auth else {}),**(headers or {})}
        req=urllib.request.Request(url+path,data=json.dumps(body).encode() if body is not None else None,headers=h)
        try:
            with urllib.request.urlopen(req,timeout=125) as r:return r.status,json.load(r)
        except urllib.error.HTTPError as e:return e.code,json.load(e)
    def body(**kw):return dict(model='gpt-oss-20b',input='Say hello in one short sentence.',max_output_tokens=128,**kw)
    def stream(b,headers=None):
        req=urllib.request.Request(url+'/v1/responses',data=json.dumps({**b,'stream':True}).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+key,**(headers or {})})
        events=[];start=time.monotonic();first=None
        with urllib.request.urlopen(req,timeout=125) as r:
            assert r.headers['Content-Type']=='text/event-stream'
            for line in r:
                if line.startswith(b'data: '):
                    if first is None:first=time.monotonic()-start
                    events.append(json.loads(line[6:]))
        assert [e['sequence_number'] for e in events]==list(range(len(events)))
        assert events[0]['type']=='response.created' and events[1]['type']=='response.in_progress'
        assert events[-1]['type'] in ('response.completed','response.incomplete','response.failed')
        result=events[-1]['response'];assert result['status'] in ('completed','incomplete'),result
        for i,item in enumerate(result['output']):
            added=[e for e in events if e['type']=='response.output_item.added' and e['output_index']==i];done=[e for e in events if e['type']=='response.output_item.done' and e['output_index']==i]
            assert len(added)==len(done)==1 and added[0]['item']['id']==done[0]['item']['id']==item['id']
            deltas=[e['delta'] for e in events if e['type'].endswith('.delta') and e.get('output_index')==i]
            expected=item['arguments'] if item['type']=='function_call' else item['content'][0]['text']
            assert ''.join(deltas)==expected
        return result,events,first,time.monotonic()-start
    try:
        for _ in range(300):
            if process and process.poll() is not None:raise AssertionError(process.stderr.read().decode())
            try:
                if request(path='/readyz',auth=False)[0]==200:break
            except (OSError,urllib.error.URLError):pass
            time.sleep(.05)
        else:raise AssertionError('server did not become ready')
        assert request(path='/v1/models')[1]['data'][0]['id']=='gpt-oss-20b'
        assert request(body(),auth=False)[0]==401
        for opts in [dict(temperature=1e-8),dict(store=True),dict(previous_response_id='resp_missing'),dict(parallel_tool_calls=True),dict(text={'format':{'type':'json_object'}}),dict(tools=[{**TOOL,'strict':True}]),dict(input=[{'type':'function_call_output','call_id':'unknown','output':'x'}])]:
            b=body();b.update(opts);status,error=request(b);assert status==400,(opts,status,error)
        status,response=request(body());assert status==200,response;assert response['status']=='completed',response
        assert response['usage']['total_tokens']==response['usage']['input_tokens']+response['usage']['output_tokens']
        assert any(i['type']=='message' for i in response['output']),response
        streamed,events,first,total=stream(body());assert first<total
        if a.binary:assert any(e['type']=='response.reasoning_text.delta' for e in events)
        _,base_events,_,_=stream(body(),{'X-Psionic-Responses-Profile':'openresponses-2026-04-24'})
        assert not any(e['type']=='response.reasoning_text.delta' for e in base_events)
        short=body();short['max_output_tokens']=1;status,limited=request(short);assert status==200 and limited['status']=='incomplete',(status,limited)
        if a.binary:
            tool_body=body(tools=[TOOL]);tool_response,_,_,_=stream(tool_body);call=next(i for i in tool_response['output'] if i['type']=='function_call');assert json.loads(call['arguments'])=={'city':'Paris'}
            replay=[{'role':'user','content':tool_body['input']},*tool_response['output'],{'type':'function_call_output','call_id':call['call_id'],'output':'sunny'}]
            status,follow=request({**tool_body,'input':replay});assert status==200 and follow['output'][-1]['content'][0]['text']=='The weather is sunny.',follow
        # Stream disconnect must release the resident worker without poisoning it.
        parsed=urllib.request.urlparse(url);s=socket.create_connection((parsed.hostname,parsed.port));cancel_body={**body(),'stream':True}
        if not a.binary:cancel_body['input']='A longer owned cancellation prefix. '*120
        data=json.dumps(cancel_body).encode();s.sendall(b'POST /v1/responses HTTP/1.1\r\nAuthorization: Bearer '+key.encode()+b'\r\nContent-Type: application/json\r\nContent-Length: '+str(len(data)).encode()+b'\r\n\r\n'+data);s.recv(4096);time.sleep(.15);s.close()
        for _ in range(100):
            status,reused=request(body())
            if status==200:break
            assert status==503,reused
            time.sleep(.05)
        else:raise AssertionError("cancelled worker was not released")
        # Overlapping requests have one successful worker and one explicit refusal.
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            replies=list(pool.map(lambda _:request(body()),range(2)))
        assert sorted(s for s,_ in replies)==[200,503],replies
        # Duplicate framing and escaped duplicate JSON keys cannot enter generation.
        conn=http.client.HTTPConnection(parsed.hostname,parsed.port);conn.putrequest('POST','/v1/responses');conn.putheader('Authorization','Bearer '+key);conn.putheader('Content-Type','application/json');conn.putheader('Content-Length','2');conn.putheader('Content-Length','2');conn.endheaders(b'{}');assert conn.getresponse().status==400;conn.close()
        if a.sdk:
            from openai import OpenAI
            client=OpenAI(base_url=url+'/v1',api_key=key,max_retries=0,timeout=125)
            response=client.responses.create(**body());assert response.output_text
            with client.responses.create(**{**body(),'stream':True}) as sdk_stream:
                sdk_events=list(sdk_stream)
            assert sdk_events[-1].type in ('response.completed','response.incomplete') and sdk_events[-1].response.output_text
            if a.binary:
                tool=client.responses.create(**body(tools=[TOOL]));call=next(i for i in tool.output if i.type=='function_call')
                replay=[{'role':'user','content':'Weather?'},*[i.model_dump() for i in tool.output],{'type':'function_call_output','call_id':call.call_id,'output':'sunny'}]
                follow=client.responses.create(**{**body(tools=[TOOL]),'input':replay});assert follow.output_text=='The weather is sunny.'
            with client.responses.stream(**body()) as accumulated:
                final=accumulated.get_final_response()
            assert final.output_text
            client.close()
        if a.tools:
            from openai import OpenAI
            client=OpenAI(base_url=url+'/v1',api_key=key,max_retries=0,timeout=125)
            prompt='Use the weather function to look up the weather in Paris. Call that function now.'
            with client.responses.stream(model='gpt-oss-20b',input=prompt,tools=[TOOL],max_output_tokens=256) as tool_stream:
                tool=tool_stream.get_final_response()
            assert tool.status=='completed'
            call=next(i for i in tool.output if i.type=='function_call')
            assert call.name=='weather' and json.loads(call.arguments)=={'city':'Paris'}
            replay=[{'role':'user','content':prompt},*[i.model_dump() for i in tool.output],{'type':'function_call_output','call_id':call.call_id,'output':'{"city":"Paris","conditions":"sunny","temperature_c":22}'}]
            with client.responses.stream(model='gpt-oss-20b',input=replay,tools=[TOOL],max_output_tokens=256) as continuation:
                answer=continuation.get_final_response()
            assert answer.status=='completed' and answer.output_text
            print(('Owned fixture' if a.binary else 'Original GPU')+' function handoff and typed SDK replay: PASS; input/output IDs:',tool.usage.input_tokens,tool.usage.output_tokens,answer.usage.input_tokens,answer.usage.output_tokens)
            client.close()
        if a.schema:
            import jsonschema
            doc=json.loads(Path(a.schema).read_text());root={**doc,'$ref':'#/components/schemas/ResponseResource'}
            for result in (response.model_dump(exclude_unset=True) if a.sdk else response,streamed,limited):jsonschema.Draft202012Validator(root).validate(result)
            for event in base_events:
                name={v['properties']['type']['enum'][0]:k for k,v in doc['components']['schemas'].items() if isinstance(v,dict) and 'StreamingEvent' in k and v.get('properties',{}).get('type',{}).get('enum')}.get(event['type'])
                if name:jsonschema.Draft202012Validator({**doc,'$ref':'#/components/schemas/'+name}).validate(event)
        print('Responses HTTP, typed replay, SSE, limits, cancellation'+(', OpenAI SDK' if a.sdk else '')+(', Open Responses schemas' if a.schema else '')+': PASS')
        print(f'SSE first event {first:.3f}s, request {total:.3f}s; input={streamed["usage"]["input_tokens"]}, output={streamed["usage"]["output_tokens"]}')
    finally:
        if process:
            process.terminate()
            try:_,err=process.communicate(timeout=10)
            except subprocess.TimeoutExpired:process.kill();_,err=process.communicate();raise AssertionError('server failed to drain')
            assert process.returncode==0,err.decode()
            assert b'ERROR: AddressSanitizer' not in err and b'runtime error:' not in err,err.decode()
if __name__=='__main__':main()

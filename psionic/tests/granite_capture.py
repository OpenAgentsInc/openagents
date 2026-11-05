#!/usr/bin/env python3
# Firm reason: 3 (runs the reference implementations it records: Hugging Face tokenizers and transformers, and llama.cpp's tokenizer)
"""Opt-in oracle capture for Granite 4.2 8B: Hugging Face `tokenizers` and
`transformers` over the pinned files, and the packaged llama.cpp tokenizer.
Writes psionic/fixtures/granite/{tokenizer-vectors,chat-vectors,source}.json.
The normal gate reads those files with the standard library only.
  python granite_capture.py MODEL_DIR [LLAMA_TOKENIZE GGUF]
"""
import hashlib,json,subprocess,sys
from pathlib import Path
import tokenizers,transformers
from tokenizers import Tokenizer
OUT=Path(__file__).resolve().parents[1]/'fixtures/granite'
TEXTS=['','hello world','Hello, world!',' leading space','trailing space ','  two leading','tabs\tand\nnewlines\n\n\n','\r\n windows \r\n',
 "it's we're they've I'm you'll he'd don't IT'S","'quoted' \"double\"",'1234567890 3.14159 -42 1e10','x=1;y=22;z=333;w=4444',
 'The quick brown fox jumps over the lazy dog.','naïve café résumé','Ünïcödé ÀÉÎÕÜ','日本語のテキスト、中文文本。','한국어 텍스트','Привет, мир!','مرحبا بالعالم',
 'emoji 😀🎉👍🏽 and ❤️','math: ∑ x² ≤ ∞, α+β=γ','def f(x):\n    return x * 2  # double\n','{"key": [1, 2, {"a": null}]}','<div class="a">text</div>',
 'snake_case camelCase PascalCase kebab-case','https://example.com/path?q=1&r=2#frag','   \n   \n',' non-breaking em space　ideographic',
 'Move the oak tree two meters north.','Set the river flow speed to 0.4 and make the water clearer.','Add a lantern to every house on the south road.',
 '<|im_start|>user\nhi<|im_end|>\n<|im_start|>assistant\n<think></think>Hello!<|im_end|>','<think>\nreasoning\n</think>\nanswer','<tool_call>\n<function=f>\n</function>\n</tool_call>',
 '<|im_start|><|im_end|>','a<|im_start|>b','[INST] text [/INST]','<s>bos</s>','<|unused_20|>x','<|im_start','im_end|>',
 'ﬁ ligature ǅ titlecase ⅷ roman ½ fraction','zero​width','é combining','\x00\x01\x1f control','  ','\n',' ']
CHATS=[
 dict(messages=[dict(role='user',content='Hello')],gen=True,kw={}),
 dict(messages=[dict(role='user',content='Hello')],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='user',content='Move the oak tree north.')],gen=True,kw=dict(enable_thinking=False,low_effort=True)),
 dict(messages=[dict(role='system',content='You edit worlds.'),dict(role='user',content='Add a lantern.')],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='system',content='Be brief.'),dict(role='user',content='a'),dict(role='assistant',content='b'),dict(role='user',content='c')],gen=True,kw={}),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='<think>\nx\n</think>\nb'),dict(role='user',content='c')],gen=True,kw=dict(low_effort=True)),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='  b  ')],gen=False,kw={}),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='<think>\nkeep\n</think>\nb')],gen=False,kw={}),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='')],gen=False,kw={}),
 dict(messages=[dict(role='user',content='q1'),dict(role='assistant',content='<think>t</think>  '),dict(role='user',content='q2')],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='system',content='only system')],gen=False,kw={}),
]
# Tool chats (the template's tools, tool calls and tool results).
MOVE=dict(type='function',function=dict(name='move_object',description='Move a placed object in the world by meters.',parameters=dict(type='object',properties=dict(object=dict(type='string'),dx=dict(type='number'),dz=dict(type='number')),required=['object','dx','dz'])))
WEATHER=dict(type='function',function=dict(name='get_weather',description='Météo: the weather for a city, in °C.',parameters=dict(type='object',properties=dict(city=dict(type='string')),required=['city']),strict=True))
def call(name,**a):return dict(type='function',function=dict(name=name,arguments=a))
TOOL_CHATS=[
 dict(messages=[dict(role='user',content='Move the oak tree two meters west.')],tools=[MOVE],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='system',content='You edit worlds.'),dict(role='user',content='Weather in Paris?')],tools=[MOVE,WEATHER],gen=True,kw={}),
 dict(messages=[dict(role='user',content='Weather in Paris?'),dict(role='assistant',content='',tool_calls=[call('get_weather',city='Paris')]),dict(role='tool',content='{"sky": "sunny", "c": 22}')],tools=[WEATHER],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='user',content='Move it.'),dict(role='assistant',content='<think>\nwhich one\n</think>\nMoving both.',tool_calls=[call('move_object',object='oak',dx=-2,dz=0),call('move_object',object='well',dx=0.5,dz=1)]),dict(role='tool',content='ok'),dict(role='tool',content='ok'),dict(role='assistant',content='Done.'),dict(role='user',content='Thanks')],tools=[MOVE],gen=True,kw=dict(low_effort=True)),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='<think>\nplan\n</think>\nLet me check.',tool_calls=[call('get_weather',city='Rome')])],tools=[WEATHER],gen=False,kw={}),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content='  checking  ',tool_calls=[call('move_object',object='crate',dx=1,dz=2)]),dict(role='tool',content='moved'),dict(role='user',content='b')],tools=[MOVE],gen=True,kw=dict(enable_thinking=False)),
 dict(messages=[dict(role='user',content='a'),dict(role='assistant',content=None,tool_calls=[call('place',spec={'kind': 'lantern', 'at': [1, 2]},tags=['a','b'])])],tools=[MOVE],gen=False,kw={}),
]
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    model=Path(sys.argv[1]);tok=Tokenizer.from_file(str(model/'tokenizer.json'))
    vectors=[dict(text=t,ids=tok.encode(t,add_special_tokens=False).ids) for t in TEXTS]
    if len(sys.argv)>3:
        llama,gguf=sys.argv[2],sys.argv[3]
        for v in vectors:
            if not v['text']:v['llama']=[];continue
            r=subprocess.run([llama,'-m',gguf,'--stdin','--ids','--no-bos','--log-disable'],input=v['text'].encode(),capture_output=True)
            v['llama']=json.loads(r.stdout.decode().strip().splitlines()[-1]) if r.returncode==0 else None
    hf=transformers.AutoTokenizer.from_pretrained(str(model));chats=[]
    for c in CHATS:
        text=hf.apply_chat_template(c['messages'],tokenize=False,add_generation_prompt=c['gen'],**c['kw'])
        chats.append(dict(messages=c['messages'],generation=c['gen'],options=c['kw'],text=text,ids=tok.encode(text,add_special_tokens=False).ids))
    for c in TOOL_CHATS:
        text=hf.apply_chat_template(c['messages'],tools=c['tools'],tokenize=False,add_generation_prompt=c['gen'],**c['kw'])
        chats.append(dict(messages=c['messages'],tools=c['tools'],generation=c['gen'],options=c['kw'],text=text,ids=tok.encode(text,add_special_tokens=False).ids))
    OUT.mkdir(parents=True,exist_ok=True)
    (OUT/'tokenizer-vectors.json').write_text(json.dumps(vectors,ensure_ascii=False,indent=1)+'\n')
    (OUT/'chat-vectors.json').write_text(json.dumps(chats,ensure_ascii=False,indent=1)+'\n')
    src=dict(model='ibm-granite/granite-4.2-8b',revision='a7042144125ce5ae64f89610e8289647fc000b9a',gguf_repository='ibm-granite/granite-4.2-8b-GGUF',gguf_revision='93f3f6a8938ee922b784cf4e5b4203cd3428df8f',
      oracle=dict(tokenizers=tokenizers.__version__,transformers=transformers.__version__),
      files={f:sha(model/f) for f in ['tokenizer.json','tokenizer_config.json','chat_template.jinja','config.json']})
    if len(sys.argv)>3:src['llama_tokenize']=subprocess.run([sys.argv[2],'--version'],capture_output=True,text=True).stderr.strip().splitlines()[0]
    (OUT/'source.json').write_text(json.dumps(src,indent=1)+'\n')
    print(f'recorded {len(vectors)} tokenizer vectors and {len(chats)} chat renders')
if __name__=='__main__':main()

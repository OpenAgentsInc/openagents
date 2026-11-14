// Firm reason: 2 (independent WebAssembly engine oracle for the JavaScriptCore artifact).
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const root = process.argv[2];
(async () => {
  for (const name of ['app', 'extras']) {
    const read = p => fs.readFileSync(`${root}/${p}`);
    const decoder = new TextDecoder();
    const context = vm.createContext({cnUTF8: bytes => decoder.decode(Uint8Array.from(bytes))});
    for (const p of ['tree-commands.js', `build/${name}.js`, 'runtime.js']) vm.runInContext(read(p).toString(), context);
    const memory = new WebAssembly.Memory({initial:512, maximum:1024});
    const {instance} = await WebAssembly.instantiate(read(`build/${name}.wasm`), {env:{memory},wasi_snapshot_preview1:{fd_fdstat_get:()=>8,fd_seek:()=>8,fd_write:()=>8}});
    const x=instance.exports, state=Math.ceil(x.__heap_base.value/65536)*65536;
    x.coaxial_state_init(state);
    function compare() {
      const n=x.dom_frame(state,0,1);
      context.raw=Array.from(new Uint8Array(memory.buffer,x.dom_out(state),n));
      const native=JSON.stringify(vm.runInContext('cnFrame()',context));
      const reference=JSON.stringify(vm.runInContext('coaxTreeCommands(Uint8Array.from(raw).buffer,0,raw.length,b=>cnUTF8(Array.from(b)))',context));
      assert.equal(native,reference,`${name}: component frames differ`);
      const op=x.dom_act_kind(state);
      const body=x.app_body ? decoder.decode(new Uint8Array(memory.buffer,x.app_body(state),x.app_body_len(state))) : '{}';
      const action=JSON.parse(JSON.stringify(vm.runInContext('cnAction()',context)));
      assert.equal(action[0],op);
      if(op) assert.equal(action[1],body);
      x.dom_act_done(state);
      return JSON.parse(native);
    }
    function event(kind,slot,text='') {
      const bytes=Array.from(new TextEncoder().encode(text));
      new Uint8Array(memory.buffer,x.dom_inbox(state),bytes.length).set(bytes);
      x.dom_event(state,kind,slot,bytes.length);
      context.eventArgs=[kind,slot,bytes];vm.runInContext('cnEvent(...eventArgs)',context);
      return compare();
    }
    const slots=new Map();
    function remember(commands) {for(const c of commands) if(c[0]===1)slots.set(c[3],c[1]);}
    remember(compare());
    if(name==='app') {
      remember(event(101,0,'{"loggedIn":false}'));
      event(2,slots.get('login.email'),'ada@example.com');
      event(1,slots.get('login.send'));
      remember(event(102,0,'{"pending":"test-challenge"}'));
      event(2,slots.get('login.code'),'123 456');event(1,slots.get('login.verify'));
      event(203,0,'{"error":"Wrong code."}');event(1,slots.get('login.verify'));
      remember(event(103,0,'{"loggedIn":true}'));
      remember(event(104,200,'{"schema":"openagents.chat.history.v1","chats":[{"id":"1","title":"Saved chat","model":"m","folder":"","updated_ms":0,"busy":false}]}'));
      remember(event(1,slots.get('chat.new')));
      event(2,slots.get('chat.composer'),'Hello');event(1,slots.get('chat.composer.send'));
      const thread={schema:'openagents.chat.thread.v1',id:'m_12345678901234567890',revision:1,messages:[{role:'user',content:'Hello'}]};
      remember(event(104,200,JSON.stringify(thread)));
      const markdown='## Heading\n\n**bold** and *italic* and [docs](https://openagents.com).\n\n- [x] done\n\n```coax\nx := 1;\n```';
      remember(event(104,200,JSON.stringify({choices:[{message:{content:markdown},finish_reason:'stop'}]})));
      thread.revision=2;thread.messages.push({role:'assistant',content:markdown});
      remember(event(104,200,JSON.stringify(thread)));
      remember(event(1,slots.get('message.1.copy')));remember(event(109,0,'{\"copied\":true}'));
      const codeCopy=[...slots.keys()].find(id=>id.startsWith('message.1.body.')&&id.endsWith('.copy'));
      assert.ok(codeCopy);remember(event(1,slots.get(codeCopy)));remember(event(109,0,'{}'));
      remember(event(1,slots.get('chat.back')));
      remember(event(104,200,'{"schema":"openagents.chat.history.v1","chats":[]}'));
      remember(event(1,slots.get('environments.open')));
      const host='a'.repeat(64);
      remember(event(104,200,JSON.stringify({schema:'openagents.environment.catalog.v1alpha1',environments:[{host_pubkey:host,display_name:'Mac'}]})));
      remember(event(1,slots.get('environment.0')));
      const view={schema:'openagents.environment.runtime.v1alpha1',host_pubkey:host,display_name:'Mac',method:'ListSessions',sessions:[],items:[],error:'',complete:true,next_cursor:'',command_id:'',command_status:'',source_epoch:''};
      remember(event(104,200,JSON.stringify(view)));
      remember(event(1,slots.get('env.new')));
      event(2,slots.get('env.composer'),'Hello Claude');event(1,slots.get('env.composer.send'));
      remember(event(111,0,JSON.stringify({command_id:'1'.repeat(32),session_id:'2'.repeat(32),now_ms:1700000000000})));
      remember(event(204,0,'{}'));remember(event(1,slots.get('env.reconnect')));
      view.method='OpenSession';view.command_id='1'.repeat(32);view.command_status='accepted';
      view.sessions=[{session_id:'2'.repeat(32),workspace_id:'default',title:'Hello Claude',origin:'managed',control:'available',generation:1,revision:1,busy:true,interaction_id:'',updated_at_ms:1700000000000}];
      view.items=[{item_id:'0',role:'user',text:'Hello Claude',tool_name:'',complete:true}];
      remember(event(104,200,JSON.stringify(view)));remember(event(112,0,'{}'));
      view.method='ReadSession';view.sessions[0].busy=false;view.items.push({item_id:'1',role:'assistant',text:markdown.repeat(40),tool_name:'',complete:true});
      remember(event(104,200,JSON.stringify(view)));

    } else {
      remember(event(1,slots.get('extras.more')));event(1,slots.get('extras.bitcoin'));
      event(21,0,'{"schema":"openagents.wallet.balance.v1","status":1,"sats":12345}');
    }
    console.log(`${name}: native artifact matches WebAssembly frames and actions`);
  }
})().catch(e=>{console.error(e.message);process.exit(1)});

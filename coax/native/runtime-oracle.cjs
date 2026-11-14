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
      event(104,200,'{"schema":"openagents.chat.history.v1","chats":[{"id":"1","title":"Saved chat","model":"m","folder":"","updated_ms":0,"busy":false}]}');
      remember(event(1,slots.get('chats.refresh')));event(104,401,'{}');
    } else {
      remember(event(1,slots.get('extras.more')));event(1,slots.get('extras.bitcoin'));
      event(21,0,'{"schema":"openagents.wallet.balance.v1","status":1,"sats":12345}');
    }
    console.log(`${name}: native artifact matches WebAssembly frames and actions`);
  }
})().catch(e=>{console.error(e.message);process.exit(1)});

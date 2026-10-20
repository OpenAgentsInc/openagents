// Test-only BIP39 vector. All provider traffic is intercepted before leaving Chrome.
// Run: openagents browser run -- node scripts/smoke/browser-wallet-privacy.mjs BUILD_DIR
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pbkdf2Sync, createHmac, createECDH } from 'node:crypto';
const build=resolve(process.argv[2]||'');
assert(process.env.OPENAGENTS_CHROME_PORT,'Run under openagents browser run.');
const words='legal winner thank year wave sausage worth useful legal winner thank yellow';
const seed=pbkdf2Sync(words,'mnemonic',2048,64,'sha512');
const secrets=[Buffer.from(words),Buffer.alloc(16,0x7f),seed];
const order=BigInt('0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141');
const master=createHmac('sha512','Bitcoin seed').update(seed).digest();
function child(node,index){
  const key=node.subarray(0,32), chain=node.subarray(32);const number=Buffer.alloc(4);number.writeUInt32BE(index);
  const ecdh=createECDH('secp256k1');ecdh.setPrivateKey(key);
  const data=Buffer.concat([index>=0x80000000?Buffer.concat([Buffer.from([0]),key]):ecdh.getPublicKey(null,'compressed'),number]);
  const digest=createHmac('sha512',chain).update(data).digest();
  const scalar=(BigInt('0x'+digest.subarray(0,32).toString('hex'))+BigInt('0x'+key.toString('hex')))%order;
  const privateKey=Buffer.from(scalar.toString(16).padStart(64,'0'),'hex');secrets.push(privateKey);
  return Buffer.concat([privateKey,digest.subarray(32)]);
}
secrets.push(master.subarray(0,32));
// The pinned SDK's mainnet account master, identity, leaf, deposit, and SDK-derived roots.
let nip06=master;for(const index of [44+0x80000000,1237+0x80000000,0x80000000,0,0])nip06=child(nip06,index);
const base=child(child(master,8797555+0x80000000),1+0x80000000);
for(const index of [0,1,2,3]){const root=child(base,index+0x80000000);for(const i of [0,1])child(root,i);}
function clean(value){
  const bytes=Buffer.isBuffer(value)?value:Buffer.from(value);const text=bytes.toString();
  for(const secret of secrets){
    assert(!bytes.includes(secret),'Private fixture material appeared in outgoing bytes.');
    assert(!text.toLowerCase().includes(secret.toString('hex')),'Hex private fixture material appeared in a request.');
    for(const encoded of [JSON.stringify([...secret]),secret.toString('hex'),secret.toString('base64'),secret.toString('base64url'),encodeURIComponent(secret.toString())]){
      if(encoded.length>=24)assert(!text.includes(encoded),'Encoded private fixture material appeared in a request.');
    }
  }
  assert(!words.split(" ").every(word=>text.includes(word)),"Recovery words appeared as separated fields.");
  for(const encoded of text.match(/[A-Za-z0-9+/_-]{32,}={0,2}/g)||[]){
    const decoded=Buffer.from(encoded,'base64');for(const secret of secrets)assert(!decoded.includes(secret),'Private fixture material appeared inside a binary request.');
  }
}
assert.throws(()=>clean(JSON.stringify({mnemonic:words})));
assert.throws(()=>clean(secrets[3].toString('base64')));
const files=new Set(['spark_wallet_web.js','spark_wallet_web_bg.wasm','start.js','wallet.css']);
let localRequests=0;
const server=createServer(async(req,res)=>{
  try{
    clean(req.url);clean(JSON.stringify(req.headers));assert.equal(req.method,'GET');localRequests++;
    const path=new URL(req.url,'http://localhost').pathname;
    if(path==='/away'){res.writeHead(200,{'Content-Type':'text/html','Cache-Control':'no-store'});res.end('<!doctype html><title>Fixture</title><p>Away</p>');return;}
    const file=path==='/wallet/restore'?'index.html':path.replace('/wallet/restore-assets/','');
    if(file!=='index.html'&&!files.has(file)){res.writeHead(404);res.end();return;}
    const mime=file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html';
    res.writeHead(200,{'Content-Type':mime,'Cache-Control':'no-store','Referrer-Policy':'no-referrer','Content-Security-Policy':"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self' https: wss:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"});
    res.end(await readFile(resolve(build,file)));
  }catch(error){res.writeHead(500);res.end();throw error;}
});
await new Promise(ok=>server.listen(0,'127.0.0.1',ok));
const origin=`http://127.0.0.1:${server.address().port}`;
const targets=await(await fetch(`http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}/json/list`)).json();
const ws=new WebSocket(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
await new Promise((ok,fail)=>{ws.onopen=ok;ws.onerror=fail;});
let next=0;const pending=new Map(),listeners=new Map();
ws.onmessage=({data})=>{const message=JSON.parse(data);if(message.id){const task=pending.get(message.id);pending.delete(message.id);message.error?task?.reject(Error(message.error.message)):task?.resolve(message.result);}else for(const fn of listeners.get(message.method)||[])fn(message.params);};
function send(method,params={}){return new Promise((resolve,reject)=>{const id=++next;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
function on(method,fn){listeners.set(method,[...(listeners.get(method)||[]),fn]);}
async function evaluate(expression){const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});assert(!result.exceptionDetails,'Browser evaluation failed.');return result.result.value;}
async function until(expression){for(let i=0;i<300;i++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,100));}throw Error('Browser wallet did not reach: '+expression);}
let external=0,payloads=0,leak=null;const exceptions=[];
on('Runtime.exceptionThrown',event=>exceptions.push(event.exceptionDetails.text));
on('Fetch.requestPaused',async event=>{
  try{
    const request=event.request;clean(request.url);clean(JSON.stringify(request.headers));
    if(request.postData)clean(request.postData);
    for(const entry of request.postDataEntries||[])if(entry.bytes)clean(Buffer.from(entry.bytes,'base64'));
    if(request.url.startsWith(origin+'/'))await send('Fetch.continueRequest',{requestId:event.requestId});
    else{
      external++;if(request.method!=='OPTIONS' && (request.postData || request.postDataEntries?.length))payloads++;
      await send('Fetch.fulfillRequest',{requestId:event.requestId,responseCode:request.method==='OPTIONS'?204:503,responseHeaders:[{name:'Access-Control-Allow-Origin',value:origin},{name:'Access-Control-Allow-Methods',value:'GET, POST, OPTIONS'},{name:'Access-Control-Allow-Headers',value:Object.entries(request.headers).find(([key])=>key.toLowerCase()==='access-control-request-headers')?.[1]||'authorization, content-type, x-grpc-web, x-user-agent'},{name:'Access-Control-Allow-Credentials',value:'true'},{name:'Content-Type',value:'application/json'}],body:Buffer.from('{"error":"fixture offline"}').toString('base64')});
    }
  }catch(error){leak=error;await send('Fetch.failRequest',{requestId:event.requestId,errorReason:'BlockedByClient'}).catch(()=>{});}
});
on('Network.webSocketFrameSent',event=>{try{clean(event.response.payloadData);}catch(error){leak=error;}});
try{
  await send('Runtime.enable');await send('Network.enable');await send('Fetch.enable',{patterns:[{urlPattern:'*',requestStage:'Request'}]});
  await send('Page.navigate',{url:origin+'/wallet/restore'});
  await until('!!document.getElementById("words")');
  await evaluate(`document.getElementById("words").value=${JSON.stringify(words)};document.getElementById("restore").click()`);
  assert.equal(external,0,'No provider contact before opt-in.');
  await evaluate('document.getElementById("consent").checked=true;document.getElementById("words").value="not valid";document.getElementById("restore").click()');
  await until('document.getElementById("status").textContent.includes("12 recovery words")');
  assert.equal(external,0,'Invalid words must be refused locally.');
  await evaluate(`document.getElementById("words").value=${JSON.stringify(words)};document.getElementById("restore").click()`);
  await until('document.getElementById("words").value===""');
  for(let i=0;i<300&&payloads===0&&!leak;i++)await new Promise(r=>setTimeout(r,100));
  assert(payloads>0,'The real compiled SDK must send an intercepted provider payload, beyond preflight.');
  await new Promise(r=>setTimeout(r,1500));
  if(leak)throw leak;
  assert.equal(await evaluate('localStorage.length+sessionStorage.length'),0);
  assert.deepEqual(await evaluate('indexedDB.databases().then(d=>d.map(x=>x.name))'),[]);
  assert.equal(exceptions.length,0,'No Wasm or script exceptions.');
  await send('Page.navigate',{url:origin+'/away'});
  await until('location.pathname==="/away"');
  await evaluate('history.back()');
  await until('!!document.getElementById("words") && !document.getElementById("consent").checked');
  await evaluate('window.__walletPrivacyDocument=1;document.getElementById("forget").click()');
  await until('typeof window.__walletPrivacyDocument==="undefined" && !!document.getElementById("words") && document.getElementById("words").value==="" && !document.getElementById("consent").checked');
  await evaluate('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
  if(leak)throw leak;
  if(process.env.OPENAGENTS_WALLET_CAPTURE){const capture=await send('Page.captureScreenshot',{format:'png'});await writeFile(process.env.OPENAGENTS_WALLET_CAPTURE,Buffer.from(capture.data,'base64'));}
  console.log(`PASS: opt-in, local validation, ${external} intercepted SDK requests, no fixture words/entropy/seed/private keys, no browser persistence, forget reset (${localRequests} static requests).`);
}finally{ws.close();await new Promise(ok=>server.close(ok));}

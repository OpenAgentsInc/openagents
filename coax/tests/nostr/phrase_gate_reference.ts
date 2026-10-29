// Public draft-fixture oracle only. Not an app codec, enabled writer or
// approved crypto provider. Actual product implementation waits for review.
import{createCipheriv,createDecipheriv,createHash,createHmac,pbkdf2Sync,hkdfSync,timingSafeEqual}from'node:crypto';
import{readFileSync,writeFileSync}from'node:fs';import{resolve}from'node:path';import{fileURLToPath}from'node:url';
export const ROOT=resolve(fileURLToPath(new URL('../../../',import.meta.url)));
export const PATH=resolve(ROOT,'coax/tests/nostr/data/phrase-gate-draft-2026-10-28.json');
export const ROUNDS=210000;
const L=(s:string)=>Buffer.from('openagents.phrase-gate.v2/'+s+'\0','ascii');
export const LABELS={kdf:L('kdf'),keys:L('keys'),mac:L('mac'),context:L('context')};
export type Pointer={id:string;recipient:string;relay:string};
export const pointer:Pointer={id:'a'.repeat(64),recipient:'79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',relay:'wss://relay.example'};
export const phrase='coaxdemophrase2026';
export const record={domain:'openagents.example',environment:'fixture',module:'access-terminal',presentation:'format-test'};
export const digest=(b:Uint8Array|string)=>createHash('sha256').update(b).digest();
export const context=digest(Buffer.concat([LABELS.context,Buffer.from(JSON.stringify(record))]));
export function normalizeV2(raw:string){
 const bytes=Buffer.from(raw);if(bytes.length>128)throw Error('refused');let out='';
 for(let byte of bytes){if(byte>=65&&byte<=90)byte+=32;if((byte>=97&&byte<=122)||(byte>=48&&byte<=57))out+=String.fromCharCode(byte);else if(byte!==32&&byte!==45)throw Error('refused');}
 if(out.length<16||out.length>40)throw Error('refused');return out;
}
export function keys(password:string,salt:Buffer,expected:Buffer){
 if(!/^[a-z0-9]{16,40}$/.test(password)||salt.length!==16||expected.length!==32)throw Error('refused');
 const master=pbkdf2Sync(Buffer.from(password),Buffer.concat([LABELS.kdf,salt]),ROUNDS,32,'sha256');
 const prk=createHmac('sha256',salt).update(master).digest();
 const material=Buffer.from(hkdfSync('sha256',master,salt,Buffer.concat([LABELS.keys,expected]),64));
 return{master,prk,material,enc:material.subarray(0,32),mac:material.subarray(32)};
}
export function seal(plain:Buffer,password=phrase,salt=Buffer.from(Array.from({length:16},(_,i)=>i)),nonce=Buffer.from(Array.from({length:12},(_,i)=>16+i)),expected=context){
 const header=Buffer.alloc(74);header.write('PG');header[2]=2;header[4]=1;header[5]=1;header.writeUInt32BE(ROUNDS,6);header.writeUInt32BE(plain.length,10);expected.copy(header,14);salt.copy(header,46);nonce.copy(header,62);
 const k=keys(password,salt,expected),iv=Buffer.alloc(16);iv.writeUInt32LE(1);nonce.copy(iv,4);
 const cipher=createCipheriv('chacha20',k.enc,iv),ciphertext=Buffer.concat([cipher.update(plain),cipher.final()]);
 const tag=createHmac('sha256',k.mac).update(Buffer.concat([LABELS.mac,header,ciphertext])).digest();
 return{header,ciphertext,tag,wire:Buffer.concat([header,ciphertext,tag]),...k};
}
export function open(wire:Buffer,password:string,expected:Buffer,onDecrypt=()=>{}){
 if(expected.length!==32||wire.length<107||wire.length>4202||wire.subarray(0,2).toString()!=='PG'||wire[2]!==2||wire[3]!==0||wire[4]!==1||wire[5]!==1||wire.readUInt32BE(6)!==ROUNDS)throw Error('refused');
 const n=wire.readUInt32BE(10);if(n<1||n>4096||wire.length!==106+n||!timingSafeEqual(wire.subarray(14,46),expected))throw Error('refused');
 const k=keys(password,wire.subarray(46,62),expected),ciphertext=wire.subarray(74,74+n),tag=createHmac('sha256',k.mac).update(Buffer.concat([LABELS.mac,wire.subarray(0,74),ciphertext])).digest();
 if(!timingSafeEqual(tag,wire.subarray(74+n)))throw Error('refused');
 onDecrypt();const iv=Buffer.alloc(16);iv.writeUInt32LE(1);wire.copy(iv,4,62,74);const cipher=createDecipheriv('chacha20',k.enc,iv);
 const plain=Buffer.concat([cipher.update(ciphertext),cipher.final()]),text=new TextDecoder('utf-8',{fatal:true}).decode(plain),value=JSON.parse(text);
 if(!value||Array.isArray(value)||Object.keys(value).sort().join(',')!=='id,recipient,relay'||typeof value.id!=='string'||typeof value.recipient!=='string'||typeof value.relay!=='string'||!/^[0-9a-f]{64}$/.test(value.id)||!/^[0-9a-f]{64}$/.test(value.recipient)||value.relay.length>2048||!/^[\x21-\x7e]+$/.test(value.relay))throw Error('refused');
 const url=new URL(value.relay);if(url.protocol!=='wss:'||url.username||url.password||url.hash)throw Error('refused');
 if(text!==JSON.stringify({id:value.id,recipient:value.recipient,relay:value.relay}))throw Error('refused');return value as Pointer;
}
export function decodeBase64(text:string){
 if(text.length>5604||!text.length||text.length%4||!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(text))throw Error('refused');
 const bytes=Buffer.from(text,'base64');if(bytes.toString('base64')!==text)throw Error('refused');return bytes;
}
export function openEntry(entry:unknown,password:string,expected:Buffer,onDecrypt=()=>{}){
 if(!entry||typeof entry!=='object'||Array.isArray(entry)||Object.keys(entry).sort().join(',')!=='payload,v')throw Error('refused');
 const e=entry as{v:unknown;payload:unknown};if(e.v!=='openagents.phrase-gate.v2'||typeof e.payload!=='string')throw Error('refused');
 return open(decodeBase64(e.payload),password,expected,onDecrypt);
}
export function fixture(){
 const plain=Buffer.from(JSON.stringify(pointer)),v2=seal(plain),legacyPhrase='opensesame',salt=Buffer.from(Array.from({length:16},(_,i)=>128+i)),iv=Buffer.from(Array.from({length:12},(_,i)=>160+i));
 const legacyPointer={relay:pointer.relay,id:pointer.id,recipient:pointer.recipient},legacyKey=pbkdf2Sync(Buffer.from(legacyPhrase),salt,ROUNDS,32,'sha256'),aes=createCipheriv('aes-256-gcm',legacyKey,iv),legacyData=Buffer.concat([aes.update(JSON.stringify(legacyPointer),'utf8'),aes.final(),aes.getAuthTag()]);
 const bad=[]as{case:string;wireHex:string;beforeDecrypt:boolean}[];
 for(const[name,at]of[['version',2],['flags',3],['kdf',4],['cipher',5],['rounds',9],['length',13],['context',14],['salt',46],['nonce',62],['ciphertext',74],['mac',v2.wire.length-1]]as const){const wire=Buffer.from(v2.wire);wire[at]^=1;bad.push({case:name,wireHex:wire.toString('hex'),beforeDecrypt:true});}
 bad.push({case:'truncated',wireHex:v2.wire.subarray(0,-1).toString('hex'),beforeDecrypt:true},{case:'trailing',wireHex:Buffer.concat([v2.wire,Buffer.from([0])]).toString('hex'),beforeDecrypt:true});
 for(const[name,plain]of[['noncanonical',JSON.stringify({relay:pointer.relay,id:pointer.id,recipient:pointer.recipient})],['duplicate','{"id":"'+pointer.id+'",'+JSON.stringify(pointer).slice(1)],['unknown',JSON.stringify({...pointer,effect:'grant'})],['shape','{"id":0}']]as const)bad.push({case:name,wireHex:seal(Buffer.from(plain)).wire.toString('hex'),beforeDecrypt:false});
 return{schema:'openagents.phrase-gate.format-fixtures.draft.v1',scope:'Public test-only; no enabled consumer, production secret or independent security approval',legacy:{sourceSha256:digest(readFileSync(resolve(ROOT,'coax/web/unlock/gate.ts'))).toString('hex'),phrase:legacyPhrase,pointer:legacyPointer,keyHex:legacyKey.toString('hex'),entry:{salt:salt.toString('hex'),check:digest(Buffer.concat([salt,Buffer.from(legacyPhrase)])).subarray(0,8).toString('hex'),iv:iv.toString('base64'),data:legacyData.toString('base64')}},v2:{phrase,contextRecord:record,contextHex:context.toString('hex'),plaintextHex:plain.toString('hex'),labels:Object.fromEntries(Object.entries(LABELS).map(([name,bytes])=>[name,bytes.toString('hex')])),masterHex:v2.master.toString('hex'),prkHex:v2.prk.toString('hex'),keysHex:v2.material.toString('hex'),headerHex:v2.header.toString('hex'),ciphertextHex:v2.ciphertext.toString('hex'),tagHex:v2.tag.toString('hex'),wireHex:v2.wire.toString('hex'),entry:{v:'openagents.phrase-gate.v2',payload:v2.wire.toString('base64')}},invalid:bad};
}
if(import.meta.main){if(!process.argv.includes('--write'))throw Error('Use --write only for deliberate draft fixture generation');writeFileSync(PATH,JSON.stringify(fixture(),null,2)+'\n');console.log(PATH);}

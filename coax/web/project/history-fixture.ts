// Public test key, independent TS fold oracle. No secret or relay side effect.
import { finalizeEvent, getPublicKey } from 'nostr-effect/pure'
import { fold, type ProjectConfig } from './model'
import { tables } from './tables'
const sk=new Uint8Array(32);sk[31]=1
const pk=getPublicKey(sk)
const config:ProjectConfig={slug:'coax',name:'Fixture',description:'',relay:'wss://relay.example',path:'coax/',contributions:'open',license:'Apache-2.0',repo:{d:'test',name:'Fixture',description:'',web:'',clone:'',euc:'0'.repeat(40)}}
const sign=(kind:number,at:number,tags:string[][])=>finalizeEvent({kind,created_at:at,tags,content:''},sk)
const root=sign(1621,1793217800,[['a',`30621:${pk}:coax`],['subject','History']])
const first=sign(1111,1793217801,[['E',root.id],['commit','a'.repeat(40)],['by','worker']])
const second=sign(1111,1793217802,[['E',root.id],['commit','b'.repeat(40)],['by','worker']])
const events=[root,first,second]
const expected=tables(fold(pk,config,events)).log
console.log(JSON.stringify({pk,events,expected}))

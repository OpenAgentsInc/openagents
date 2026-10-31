// Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
// Independent test-only JCS/hash oracle for actual Coax-emitted content.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const hash=(s:string)=>createHash('sha256').update(s).digest('hex');
const canonical=(x:any):any=>Array.isArray(x)?x.map(canonical):x&&typeof x==='object'?Object.fromEntries(Object.keys(x).sort().map(k=>[k,canonical(x[k])])):x;
const lines=(await Bun.file(process.argv[2]).text()).trim().split('\n');assert.equal(lines.length,5);
const [policy,body,tags,maxPolicy,maxBody]=lines.map(JSON.parse);
for(const i of [0,1,3,4])assert.equal(JSON.stringify(canonical(JSON.parse(lines[i]))),lines[i]);
assert.equal(body.policy,hash(lines[0]));assert.equal(maxBody.policy,hash(lines[3]));
assert.deepEqual(tags,[['d','demo'],['t','oa:pylon-pool:v2'],['x',hash(lines[1])],['expiration','180']]);
for(const [k,c]of[['beacons','1'],['receipts','3'],['checks','4']])assert.deepEqual(body.inputs[k],{count:1,digest:hash(c.repeat(64))});
assert.equal(policy.profile,'psionic-free-mesh-v1');assert.equal(policy.exclude_failed,false);assert.deepEqual(policy.payment_issuers,[]);
assert.deepEqual(body.totals,{by_family:{cpu:0,gpu:0,'unified-memory':1},checks:{fail:0,inconclusive:0,pass:1},jobs:{accepted:1,failed:0,rejected:0,timeout:0},paid_msat:{bitcoin:0,regtest:0,signet:0,testnet:0},pylons_online:1,slots_free:2,slots_total:3,units:{jobs:1,seconds:0,tokens:0}});
assert.deepEqual(body.window,{from:0,to:120});assert.deepEqual(body.rate,[1]);assert.equal(body.v,'openagents.pylon-pool.v2');assert.deepEqual(body.requires,[]);
for(const [role,n]of[['providers',32],['buyers',64],['checkers',128]]as const){assert.equal(maxPolicy[role].length,n);assert.equal(new Set(maxPolicy[role]).size,n);assert.deepEqual(maxPolicy[role],[...maxPolicy[role]].sort());}
for(const k of ['beacons','receipts','checks'])assert.deepEqual(maxBody.inputs[k],{count:0,digest:hash('')});
console.log('PASS independent JCS/SHA256: every total, policy/tag commitment, maximum role sets and empty counted-input hashes');

// Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
// Independent ECMAScript NIP-01 preimage/hash oracle; no product dependency.
const fs=require('node:fs'),{createHash}=require('node:crypto');
const rows=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
process.stdout.write(JSON.stringify(rows.map(e=>{
  const preimage=Buffer.from(JSON.stringify([0,e.pubkey,e.created_at,e.kind,e.tags,e.content]),'utf8');
  return {preimage:preimage.toString('hex'),id:createHash('sha256').update(preimage).digest('hex')};
})));

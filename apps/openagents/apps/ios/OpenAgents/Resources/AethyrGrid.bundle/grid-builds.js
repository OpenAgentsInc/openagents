'use strict';
const GridBuilds = (() => {
  const ANNOUNCER = 'eaf77c570b5cdfbd7056a5ebcde063446b86c767c17e50e0c7992ed078ddffb4';
  const PHASES = ['build_started','build_done','build_failed','deploying','promoted','deploy_failed','smoke_running','smoke_passed','smoke_failed','traffic_shifted','rolled_back','live'];
  function read(event, test = false, trusted = ANNOUNCER, now = Math.floor(Date.now()/1000)) {
    if (!event || event.pubkey !== trusted || ![3205,30205].includes(event.kind) || typeof event.content !== 'string' || event.content.length > 1024 || !Number.isSafeInteger(event.created_at) || event.created_at > now + 300 || (!Array.isArray(event.tags) || event.tags.length>16 || !event.tags.every(t=>Array.isArray(t)&&t.length===2&&t.every(v=>typeof v==='string')))) return null;
    let s;
    try { if (!NostrTools.verifyEvent(event)) return null; s = JSON.parse(event.content); } catch (_) { return null; }
    if (s.v !== 'openagents.deploy-status.v1' || !['staging','production'].includes(s.env) || !['image','module','content'].includes(s.tier) || !PHASES.includes(s.phase) ||
        !/^[a-z0-9-]{1,32}$/.test(s.run) || !/^[a-z0-9-]{1,24}$/.test(s.by) || !Number.isSafeInteger(s.started) || s.started < 0 || s.started > event.created_at ||
        (s.commit !== undefined && !/^[0-9a-f]{7,40}$/.test(s.commit)) || (s.subject !== undefined && (typeof s.subject !== 'string' || s.subject.length > 72 || /[^\x20-\x7e]/.test(s.subject))) ||
        (s.build !== undefined && !/^[a-zA-Z0-9_.-]{1,64}$/.test(s.build)) || (s.test !== undefined && typeof s.test !== 'boolean') || !!s.test !== test) return null;
    const one = name => { const matches=event.tags.filter(t=>t[0]===name); return matches.length===1?matches[0][1]:null; };
    const lane = (s.test ? 'test/' : '') + s.env;
    if(one('t')!=='oa:deploy:v1'||one('env')!==s.env||one('phase')!==s.phase||
       (event.kind===30205 ? one('d')!==lane : event.tags.some(t=>t[0]==='d'))) return null;
    return {...s, at:event.created_at, id:event.id};
  }
  return { ANNOUNCER, read, PHASES };
})();
if(typeof module!=='undefined') module.exports=GridBuilds;

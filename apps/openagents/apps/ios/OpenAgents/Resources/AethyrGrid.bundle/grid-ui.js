// The Grid's surrounding experience: overview, map navigation, people,
// walk-up stations, public Pylon/Foreman status, and signed deploy reports.
'use strict';
const GridUI = (() => {
  const $ = id => document.getElementById(id);
  const params = new URLSearchParams(location.search);
  const offline = params.has('offline');
  let peopleRevision = '';
  let G, session, panel = '', selectedKey = '', noticeTimer, destination = null, path = [], figures = [], currentStation = null;
  let hudAt = 0, servicesAt = -Infinity, serviceBusy = false, pylons = [], foreman = null, activity = null, machines = [];
  const labels = new Map(), slots = new Map(), builds = new Map();
  const cv = $('canvas'), body = $('panelBody');
  let reduced = GridNet.read('aethyr.grid.reduced', matchMedia('(prefers-reduced-motion: reduce)').matches);
  let stations = [];
  function element(tag, text, cls) { const el=document.createElement(tag); if(text!==undefined) el.textContent=text; if(cls) el.className=cls; return el; }
  function paragraph(text) { body.append(element('p',text)); }
  function button(label, fn, parent=body) { const b=element('button',label); b.onclick=fn; parent.append(b); return b; }
  function link(label, url, parent=body) { const a=element('a',label,'service-link'); a.href=url; a.target='_blank'; a.rel='noopener noreferrer'; parent.append(a); return a; }
  function notice(text) { $('notice').textContent=text; $('notice').classList.add('on'); clearTimeout(noticeTimer); noticeTimer=setTimeout(()=>$('notice').classList.remove('on'),3500); }
  function release() { dispatchEvent(new Event('grid-release')); if(document.pointerLockElement) document.exitPointerLock(); }
  function closePanel() { panel=''; $('gridPanel').hidden=true; G?.pause(session?.pendingSpawn?1:0); cv.focus({preventScroll:true}); }
  function openPanel(id, title) {
    release(); panel=id; $('panelTitle').textContent=title; body.replaceChildren(); $('gridPanel').hidden=false; G?.pause(1);
    $('panelClose').focus({preventScroll:true});
  }
  function connection() {
    if(!session) return;
    $('connection').textContent=session.status;
    $('playBtn').textContent=session.joined?'Watch':'Play';
    $('peopleBtn').textContent='Players' + (session.peers.size?' · '+session.peers.size:'');
    if(panel==='people' && peopleRevision!==peopleStamp()) people();
  }
  function join() {
    if(!G || session.joined) return;
    const spawn=session.join(); if(spawn) G.place(spawn.x,spawn.z,spawn.yaw);
    G.watch(0); G.pause(session.pendingSpawn?1:0); cv.focus({preventScroll:true});
  }
  function watch() { release(); session.watch(); G.watch(1); destination=null; path=[]; closePanel(); }
  function ready(engine) {
    G=engine;
    stations=G.metadata.landmarks.filter(l=>l.interaction).map(l=>({id:l.id,name:l.label,x:l.position[0],floorY:l.position[1],z:l.position[2],y:l.position[1]+l.labelHeight,enter:l.interaction.enter,leave:l.interaction.leave,title:l.interactionLabel}));
    GridNav.configure(G.metadata.colliders);
    GridNet.configureLayout(G.metadata);
    session=new GridNet.Session({relay:params.get('relay')||'wss://relay.openagents.com',offline,name:params.get('name'),changed:connection});
    session.deployTest=params.get('deploys')==='test';
    session.onBuild=record=>{
      const key=record.env+'/'+record.run, prev=builds.get(key);
      if(!prev || record.at>prev.at || record.at===prev.at && GridBuilds.PHASES.indexOf(record.phase)>GridBuilds.PHASES.indexOf(prev.phase)) builds.set(key,record);
      if(builds.size>48) builds.delete([...builds].sort((a,b)=>a[1].at-b[1].at)[0][0]);
      if(panel==='builds') showBuilds();
    };
    G.look(GridNet.seed(session.key));
    G.reduceMotion(reduced?1:0);
    const at=params.get('at')?.split(',').map(Number);
    if(at && at.length>=2 && at.every(Number.isFinite) && at.every(n=>Math.abs(n)<260)) session.resume={p:[at[0],0,at[1]],yaw:at[2]||0};
    G.watch(1); session.connect(); connection();
    if(params.has('play') || params.has('at')) join();
    if(at && at.length>=2 && at.every(Number.isFinite) && at.every(n=>Math.abs(n)<260)) G.place(at[0],at[1],at[2]||0);
    window.gridState=()=>({joined:session.joined,status:session.status,key:session.key,name:session.name,network:session.metrics(),players:session.figures().map(({key,name,p})=>({key,name,p})),destination,panel,station:currentStation?.id,builds:[...builds.values()],pylons:pylons.length});
    // Shared inspection hooks used by OpenAgents' Grid smoke tools.
    window.verse_players=()=>JSON.stringify(window.gridState().players);
    window.verse_face=(x,z)=>G.face(x,z);
    window.verse_where=()=>JSON.stringify({place:'grid',zone:'grid',scheme:'walk',watching:!session.joined});
    window.verse_build_board=()=>JSON.stringify([...builds.values()]);
    addEventListener('pagehide',()=>session.close());
    addEventListener('pageshow',e=>{if(e.persisted)location.reload();});
    document.addEventListener('visibilitychange',()=>{
      if(document.hidden) { release(); session.watch(); G.watch(1); }
      // Returning stays in Watch until the visitor chooses to rejoin.
    });
    if(params.has('help')) guide();
  }
  function label(id, text, x, y, z, onClick, sub='') {
    let el=labels.get(id);
    if(!el) { el=element('button','','world-label '+(onClick?'player':'static')); $('worldLabels').append(el); labels.set(id,el); }
    if(el.dataset.label!==text+'|'+sub){el.replaceChildren(document.createTextNode(text));if(sub)el.append(element('small',sub));el.dataset.label=text+'|'+sub;}
    el.onclick=onClick||null;
    el.sceneWidth=Math.min(240,Math.max(text.length*6.5,sub.length*5)+12);el.sceneHeight=sub?30:18;
    const visible=G.project(x,y,z);
    el.hidden=!visible;
    if(visible) { el.sceneX=G.screenX()/cv.width*cv.clientWidth;el.sceneY=G.screenY()/cv.height*cv.clientHeight;el.style.left=el.sceneX+'px';el.style.top=el.sceneY+'px'; }
    return el;
  }
  function resolveLabels() {
    const placed=[];
    for(const el of labels.values()) {
      if(el.hidden)continue;
      let y=el.sceneY,box;
      for(let attempt=0;attempt<10;attempt++) {
        box=[el.sceneX-el.sceneWidth/2,y-el.sceneHeight,el.sceneX+el.sceneWidth/2,y];
        if(!placed.some(r=>box[0]<r[2]+4&&box[2]>r[0]-4&&box[1]<r[3]+4&&box[3]>r[1]-4))break;
        y-=el.sceneHeight+5;
      }
      if(y-el.sceneHeight<65){el.hidden=true;continue;}
      el.style.top=y+'px';placed.push(box);
    }
  }
  function peers(now) {
    figures=session.figures();
    const active=new Set(figures.map(p=>p.key));
    for(const [key,slot] of slots) if(!active.has(key)) { G.removePeer(slot); slots.delete(key); labels.get('peer:'+key)?.remove(); labels.delete('peer:'+key); }
    const nearest=[...figures].sort((a,b)=>Math.hypot(a.p[0]-G.x(),a.p[2]-G.z())-Math.hypot(b.p[0]-G.x(),b.p[2]-G.z()));
    const shown=new Set(nearest.slice(0,24).map(p=>p.key));
    for(const peer of figures) {
      if(!slots.has(peer.key)) { let slot=0; while([...slots.values()].includes(slot)) slot++; slots.set(peer.key,slot); }
      G.peer(slots.get(peer.key),peer.seed,...peer.p,peer.yaw,peer.speed);
      const distance=Math.hypot(peer.p[0]-G.x(),peer.p[2]-G.z());
      const el=label('peer:'+peer.key,peer.name,...[peer.p[0],peer.p[1]+2.1,peer.p[2]],()=>player(peer.key),session.muted.has(peer.key)?'Muted':peer.key.slice(0,8));
      if(!shown.has(peer.key) || session.joined && distance>60) el.hidden=true;
    }
    if(now-hudAt>500) { connection(); hudAt=now; }
  }
  function player(key) {
    const peer=session.peers.get(key); if(!peer) return;
    selectedKey=key; openPanel('player',peer.name);
    paragraph('Your choices apply only in this browser.'); body.append(element('code',key));
    button(session.muted.has(key)?'Unmute':'Mute',()=>{session.moderate(key,'muted',!session.muted.has(key));player(key);});
    button('Block',()=>{session.moderate(key,'blocked',true);people();});
  }
  function peopleStamp() { return JSON.stringify([session.name,session.joined,[...session.peers].map(([k,p])=>[k,p.name]),[...session.blocked],[...session.muted]]); }
  function people() {
    if(!G) return;
    if(panel!=='people') openPanel('people','Players'); else body.replaceChildren();
    peopleRevision=peopleStamp();
    const self=element('div',session.name+(session.joined?' · You':' · Watching'),'row');body.append(self);
    if(!session.peers.size) paragraph(offline?'You are exploring offline.':'Other players appear here as they join.');
    for(const peer of [...session.peers.values()].slice(0,128)) button(peer.name+(session.muted.has(peer.key)?' · Muted':''),()=>player(peer.key));
    if(session.blocked.size) { body.append(element('h3','Blocked players')); for(const key of session.blocked) button(key.slice(0,12)+' · Unblock',()=>{session.moderate(key,'blocked',false);people();}); }
    if(session.muted.size) { body.append(element('h3','Muted players')); for(const key of session.muted) button(key.slice(0,12)+' · Unmute',()=>{session.moderate(key,'muted',false);people();}); }
  }
  function settings() {
    if(!G) return; openPanel('settings','Settings');
    const form=element('form'), nameLabel=element('label','Your name'), input=element('input');input.value=session.name;input.maxLength=40;nameLabel.append(input);form.append(nameLabel);
    const save=element('button','Save name');save.type='submit';form.append(save);form.onsubmit=e=>{e.preventDefault();session.rename(input.value);notice('Name saved.');};body.append(form);
    const motion=element('label'), check=element('input');check.type='checkbox';check.checked=reduced;motion.append(check,document.createTextNode(' Reduce camera motion'));body.append(motion);
    check.onchange=()=>{reduced=check.checked;GridNet.write('aethyr.grid.reduced',reduced);G.reduceMotion(reduced?1:0);};
    paragraph(offline?'Offline: your avatar stays in this browser.':'Your position and display name are shared while you play. Watch listens without sharing your position.');
    body.append(element('small','Player key'));body.append(element('code',session.key));
    button('Copy invite',()=>{
      const url=new URL(['https:','http:'].includes(location.protocol)?location.href:'https://aethyr-staging-ezxz4mgdsq-uc.a.run.app/grid.html');url.searchParams.delete('name');url.searchParams.delete('at');url.searchParams.delete('play');url.searchParams.delete('boot');
      if(!navigator.clipboard?.writeText){notice('Copy the address from your browser to invite a friend.');return;}
      navigator.clipboard.writeText(url.href).then(()=>notice('Invite copied.'),()=>notice('Copy the address from your browser to invite a friend.'));
    });
  }
  const helpRoutes=Semantic.selector([
    {id:'move',examples:['how do I move walk run jump controls camera','keyboard mouse touch zoom first person'],answer:'WASD or arrows move. Shift runs, Space jumps. Drag to orbit; click the world to capture the mouse, Esc releases it. Scroll to zoom into first person and back. On touch, use the two sticks and Run/Jump buttons.'},
    {id:'people',examples:['other players multiplayer name block mute friends invite online'],answer:'Play joins the shared Grid. Watch lets you look around without sharing your position. Click a player’s name to mute or block them; Players shows your lists. Settings lets you change your name or copy an invite.'},
    {id:'map',examples:['map where gym terminal help desk foreman game lab pylon bay walk destination lost'],answer:'Press M to open the map. Click a place to walk there, or press a movement key to stop. Walk up to a station, then press E or tap its prompt.'},
    {id:'build',examples:['build status staging production deploy billboard'],answer:'The billboard shows signed OpenAgents build reports from the Grid relay. Walk up to it and open Build status for staging and production history.'},
    {id:'world',examples:['what is the grid aethyr verse robots portals'],answer:'This is Aethyr’s port of the Grid: the original Gym, boards and robot patrols, with multiplayer and walk-up stations. Zone portals are omitted in this version. The Verse link opens the haunted house and your planets.'},
  ],{threshold:.22});
  function guide() {
    if(!G) return; openPanel('guide','Grid guide');
    paragraph('Explore the Grid, meet other players, and walk up to a station.');
    const transcript=element('div');body.append(transcript);
    function answer(text) { const route=helpRoutes.select(text);const line=element('div',route?.answer||'Try asking about movement, players, the map, or build status.','chatline');line.prepend(element('b','GRID GUIDE'));transcript.append(line); }
    for(const [text,query] of [['Controls','controls'],['Meet players','other players'],['Find a place','map'],['What is this?','what is the grid']]) button(text,()=>answer(query));
    const form=element('form'), input=element('input');input.placeholder='Ask about the Grid';input.maxLength=500;input.setAttribute('aria-label','Question for the Grid guide');form.append(input);
    const send=element('button','Ask');send.type='submit';form.append(send);form.onsubmit=e=>{e.preventDefault();if(input.value.trim()){answer(input.value);input.value='';}};body.append(form);
  }
  function suggest() {
    openPanel('suggest','Leave a suggestion');paragraph('Tell us what would make Aethyr better. This saves your suggestion on this Aethyr server.');
    const form=element('form'), label=element('label','Your suggestion'), text=element('textarea');text.maxLength=4000;text.required=true;label.append(text);form.append(label);
    const categoryLabel=element('label','Category'), select=element('select');for(const [value,title] of [['idea','Idea'],['bug','Something broke'],['feedback','Feedback']]) {const option=element('option',title);option.value=value;select.append(option);}categoryLabel.append(select);form.append(categoryLabel);
    const submit=element('button','Save suggestion');submit.type='submit';form.append(submit);
    form.onsubmit=async e=>{e.preventDefault();submit.disabled=true;try{const response=await fetch('/api/grid/suggestion',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({text:text.value,category:select.value,name:session.name})});if(!response.ok)throw Error();notice('Suggestion saved. Thank you.');closePanel();}catch(_){notice('Could not save. Please try again.');submit.disabled=false;}};body.append(form);
  }
  function terminal() {
    openPanel('terminal','Pylon terminal');
    paragraph(offline?'Pylon status is unavailable while exploring offline.':pylons.length?`${pylons.length} pylons online.`:'No live pylons to show right now.');
    for(const p of pylons.slice(0,64)) {
      const row=element('div','','row');row.append(element('div',String(p.name||p.slug)),element('small',`${p.family||'Computer'} · ${p.memory_gb||0} GB · ${p.busy||0} of ${p.total||0} busy`));
      for(const service of (p.services||[]).slice(0,6)) row.append(element('small',typeof service==='string'?service:String(service.model||service.name||service.kind||'')));
      body.append(row);
    }
    link('Use the OpenAgents pylon terminal ↗','https://openagents.com/home?chat=1');
    paragraph('Live status is shown here. Inference and account actions open in OpenAgents.');
  }
  function foremanPanel() {
    openPanel('foreman','Foreman');
    if(foreman) {
      const data=foreman;
      paragraph(String(data.status||data.line||data.summary||'OpenAgents work queue'));
      if(data.counts) paragraph(`${data.counts.open??0} open issues · ${data.counts.in_progress??0} in progress`);
      for(const issue of (data.issues||data.record?.issues||[]).slice(0,12)) {
        const row=element('div','','row');row.append(element('div',`#${issue.number} ${issue.title||''}`),element('small',issue.state||(issue.assigned?'In progress':'Open')));body.append(row);
      }
      if(activity) paragraph(String(activity.text||activity.status||activity.line||activity.task||''));
    } else paragraph(offline?'Foreman status is unavailable offline.':'The Foreman’s live status is unavailable right now.');
    button('Ask the Grid guide',guide);link('Visit the Foreman in OpenAgents ↗','https://openagents.com/home');
  }
  function games() {
    openPanel('games','Game Lab');paragraph('Create games with Game Lab in OpenAgents, or explore Aethyr’s worlds here.');
    link('Open Game Lab ↗','https://openagents.com/home?lab=game');
    link('Open Kiln ↗','https://openagents.com/kiln/');
    const verse=element('a','Explore The Verse','service-link');verse.href='index.html';body.append(verse);
  }
  function gym() {
    openPanel('gym','Gym boards');paragraph('The original Gym and its RESULTS and EVALS boards are preserved here as part of the Grid world.');
    link('About the OpenAgents Verse ↗','https://openagents.com/docs/verse');
  }
  function showBuilds() {
    if(panel!=='builds') openPanel('builds','Build status');else body.replaceChildren();
    for(const env of ['staging','production']) {
      body.append(element('h3',env.toUpperCase()));
      const records=[...builds.values()].filter(b=>b.env===env).sort((a,b)=>b.at-a.at).slice(0,4);
      if(!records.length) body.append(element('small',offline?'Offline':'Waiting for signed build reports…'));
      for(const [i,b] of records.entries()) {
        const row=element('div','','row');row.append(element('div',(b.commit?.slice(0,8)||b.tier)+' · '+(b.subject||b.phase.replaceAll('_',' '))));
        row.append(element('small',b.phase.replaceAll('_',' ')+' · '+new Date(b.at*1000).toLocaleString()));
        if(i===0) {
          const progress=({build_started:0,build_done:1,build_failed:0,deploying:1,promoted:2,deploy_failed:1,smoke_running:2,smoke_passed:3,smoke_failed:2,traffic_shifted:4,rolled_back:3,live:4})[b.phase];
          for(const [j,name] of ['BUILD','DEPLOY','SMOKE','SHIFT','LIVE'].entries()) row.append(element('span',name,'build-step'+(j<=progress?' done':'')));
        }body.append(row);
      }
    }
  }
  function interact() {
    if(!currentStation) return;
    ({terminal,guide,foreman:foremanPanel,suggest,games,builds:showBuilds,gym})[currentStation.id]?.();
  }
  async function services(now) {
    if(offline || serviceBusy || now-servicesAt<15000) return;
    servicesAt=now;serviceBusy=true;
    try {
      const results=await Promise.allSettled(['pylons','foreman','activity'].map(name=>fetch('/api/grid/service/'+name).then(r=>r.ok?r.json():null)));
      const data=results.map(r=>r.status==='fulfilled'&&r.value?.available?r.value.data:null);
      pylons=Array.isArray(data[0]?.pylons)?data[0].pylons.filter(p=>p&&typeof p==='object'&&p.expires_at>Date.now()/1000).slice(0,64):[];
      foreman=data[1];activity=data[2];
      G.machinesClear();machines=[];
      const groups=new Map();
      for(const p of pylons) {
        const family=['gpu','cpu','unified-memory'].indexOf(p.family), section=Math.max(0,family);
        const index=groups.get(section)||0;groups.set(section,index+1);
        const x=18+Math.floor(index/2)*2.8,z=-2+section*7+(index%2?4.25:1.25);
        G.machine(x,z,p.total?Math.min(1,p.busy/p.total):0);machines.push({p,x,z});
      }
      if(panel==='terminal') terminal();else if(panel==='foreman')foremanPanel();
    }finally{serviceBusy=false;}
  }
  function toggleMap() { if(!G)return;$('minimap').hidden=!$('minimap').hidden; if(!$('minimap').hidden) release(); }
  function map() {
    const canvas=$('minimap');if(canvas.hidden)return;
    const c=canvas.getContext('2d'),w=canvas.width;c.clearRect(0,0,w,w);c.fillStyle='#090909ed';c.fillRect(0,0,w,w);
    const at=(x,z)=>[w/2+x*2.3,w*.66-z*2.3];
    c.strokeStyle='#272727';c.lineWidth=1;
    for(let v=-64;v<=64;v+=8){let a=at(v,-64),b=at(v,64);c.beginPath();c.moveTo(...a);c.lineTo(...b);c.stroke();a=at(-64,v);b=at(64,v);c.beginPath();c.moveTo(...a);c.lineTo(...b);c.stroke();}
    c.strokeStyle='#777';c.strokeRect(...at(-9,50),18*2.3,24*2.3);
    c.font='9px system-ui';c.fillStyle='#aaa';
    for(const s of stations){const p=at(s.x,s.z);c.fillStyle='#aaa';c.fillRect(p[0]-2,p[1]-2,4,4);const names={terminal:['Pylons',5,-8],guide:['Guide',-30,9],foreman:['Foreman',10,10],suggest:['Ideas',6,7],games:['Games',-36,-2],builds:['Builds',5,-6],gym:['Gym',5,-4]};const [text,dx,dy]=names[s.id]||[s.name,5,-4];c.fillText(text,p[0]+dx,p[1]+dy);}
    for(const p of figures){c.fillStyle='#8aa';c.beginPath();c.arc(...at(p.p[0],p.p[2]),2,0,Math.PI*2);c.fill();}
    if(destination){c.strokeStyle='#e9b768';c.beginPath();c.moveTo(...at(G.x(),G.z()));for(const p of path)c.lineTo(...at(...p));c.stroke();c.fillStyle='#e9b768';c.fillRect(...at(...destination),4,4);}
    if(session.joined){const p=at(G.x(),G.z());c.save();c.translate(...p);c.rotate(G.yaw());c.fillStyle='#fff';c.beginPath();c.moveTo(0,-7);c.lineTo(-4,4);c.lineTo(4,4);c.closePath();c.fill();c.restore();}
    c.fillStyle='#999';c.fillText('N ↑ · Click to walk · WASD cancels',10,15);
  }
  $('minimap').onclick=e=>{
    join();const r=e.currentTarget.getBoundingClientRect(),x=(e.clientX-r.left)/r.width*300,z=(e.clientY-r.top)/r.height*300;
    destination=[Math.max(-60,Math.min(60,(x-150)/2.3)),Math.max(-35,Math.min(60,(198-z)/2.3))];path=[];
    const planned=GridNav.route([G.x(),G.z()],destination,machines);
    if(!planned){destination=null;notice('There’s no clear walk there. Choose another spot.');return;}
    path=planned;destination=path.at(-1);notice('Walking. Press a movement key to stop.');cv.focus({preventScroll:true});
  };
  function tick(now,dt,kx,ky,px,py) {
    if(!session)return null;
    const spawn=session.pollSpawn();
    if(spawn) { G.place(spawn.x,spawn.z,spawn.yaw); G.pause(panel?1:0); }
    peers(now);session.tick([G.x(),G.y(),G.z()],G.yaw(),G.speed(),[G.vx(),G.vy(),G.vz()]);services(now);map();
    let nearest=null,distance=Infinity;
    for(const station of stations){
      const d=Math.hypot(G.x()-station.x,G.z()-station.z);
      const sub=station.id==='foreman'?String(foreman?.status||foreman?.line||''):station.id==='builds'?[...builds.values()].sort((a,b)=>b.at-a.at)[0]?.phase?.replaceAll('_',' ')||'':'';
      const el=label('station:'+station.id,station.name,station.x,station.y,station.z,null,sub);
      if(session.joined&&d>80)el.hidden=true;
      if(session.joined&&Math.abs(G.y()-station.floorY)<2&&d<(currentStation===station?station.leave:station.enter)&&d<distance){nearest=station;distance=d;}
    }
    label('bay','PYLON BAY',29,5,21,null,pylons.length?`${pylons.length} online`:'');
    for(const [i,env] of ['staging','production'].entries()) {
      const record=[...builds.values()].filter(b=>b.env===env).sort((a,b)=>b.at-a.at)[0];
      const laneLabel=label('buildlane:'+env,env.toUpperCase(),25+i*10,10,-6.7,null,record?`${record.commit?.slice(0,7)||record.tier} · ${record.phase.replaceAll('_',' ')}`:'Waiting for reports');
      if(!session.joined||Math.hypot(G.x()-30,G.z()+7)>25)laneLabel.hidden=true;
    }
    for(const [i,m] of machines.entries())label('machine:'+i,String(m.p.name||m.p.slug),m.x,3,m.z,()=>{terminal();},`${m.p.busy||0}/${m.p.total||0} busy`);
    for(const key of [...labels.keys()])if(key.startsWith('machine:')&&Number(key.slice(8))>=machines.length){labels.get(key).remove();labels.delete(key);}
    resolveLabels();
    if(panel&&currentStation&&!nearest&&stations.some(s=>s.id===panel))closePanel();
    currentStation=nearest;
    $('interact').hidden=!nearest||!!panel;
    if(nearest)$('interact').textContent=(document.body.classList.contains('touch')?'Tap to open ':'E · ')+nearest.title;
    if(kx||ky||px||py){destination=null;path=[];join();}
    if(!session.joined||panel)return{x:0,y:0};
    if(destination&&path.length){const target=path[0];if(Math.hypot(G.x()-target[0],G.z()-target[1])<(path.length>1?.08:.3)){path.shift();if(!path.length){destination=null;notice('You’re here.');return{x:0,y:0};}}else{G.face(...target);return{x:0,y:1};}}
    return null;
  }
  function key(e) {
    if(!G)return false;
    if(e.code==='KeyE'&&currentStation&&!e.repeat){interact();return true;}
    if(e.repeat)return false;
    if(e.code==='KeyM'){toggleMap();return true;}
    if(e.code==='KeyH'){guide();return true;}
    if(['KeyW','KeyA','KeyS','KeyD','ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space'].includes(e.code)){join();destination=null;path=[];}
    return false;
  }
  $('playBtn').onclick=()=>session?.joined?watch():join();$('peopleBtn').onclick=people;$('settingsBtn').onclick=settings;
  $('mapBtn').onclick=toggleMap;$('guideBtn').onclick=guide;$('panelClose').onclick=closePanel;$('interact').onclick=interact;
  addEventListener('keydown',e=>{if(e.code==='Escape'){if(panel){e.preventDefault();closePanel();}else if(destination){destination=null;path=[];notice('Stopped.');}else $('minimap').hidden=true;}},true);
  function reroll() { if(!G)return;session.lookSeed=crypto.getRandomValues(new Uint32Array(1))[0]&0x7fffffff;G.look(session.lookSeed);session.lastState=0; }
  return {ready,join,key,tick,notice,reroll};
})();

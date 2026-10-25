// Platform bridge for the pinned, bundled Aethyr Grid. No account data crosses it.
(() => {
  'use strict';
  const boot = new URLSearchParams(location.search).get('boot');
  const post = value => window.webkit.messageHandlers.gridShell.postMessage({...value, boot});
  const originalFetch = window.fetch.bind(window);
  window.fetch = (input, options) => {
    const url = new URL(typeof input === 'string' ? input : input.url, location.href);
    if(url.protocol === 'aethyr-grid:' && url.host === 'bundle' && url.pathname === '/api/grid/suggestion' && options?.method === 'POST') {
      return window.webkit.messageHandlers.gridSuggestion.postMessage(options.body)
        .then(text => new Response(text,{status:200,headers:{'Content-Type':'application/json'}}));
    }
    return originalFetch(input,options);
  };
  addEventListener('error', () => post({phase:'failed'}));
  addEventListener('unhandledrejection', () => post({phase:'failed'}));
  addEventListener('DOMContentLoaded', () => {
    document.body.classList.add('touch');
    const style=document.createElement('style');
    style.textContent='nav a[href="index.html"]{display:none}.top .title{display:none}.top{justify-content:flex-end}.stick{width:104px;height:104px}.stick.move{left:16px}.stick.look{right:16px}body.touch .hotbar{left:16px;gap:3px}.slot{width:44px;height:54px}.slot .lbl{font-size:8px}.slot .key{display:none}.pads{right:16px;gap:4px}.pads button{padding:10px 9px;font-size:12px}.hud{max-width:70vw}';
    document.head.append(style);
    document.querySelector('#camBtn .lbl').textContent='Camera';
    document.getElementById('camBtn').setAttribute('aria-label','First person');
    document.getElementById('canvas').setAttribute('aria-label','The Grid world');
    document.getElementById('moveStick').setAttribute('aria-label','Move');
    document.getElementById('moveStick').setAttribute('role','group');
    document.getElementById('lookStick').setAttribute('aria-label','Look');
    document.getElementById('lookStick').setAttribute('role','group');
    let ready=false, maxY=0;
    const report=setInterval(() => {
      if(!window.G || !window.gridState || G.fps()<=0) return;
      if(!ready) {ready=true;post({phase:'ready'});}
      maxY=Math.max(maxY,G.y());
      if(window.__gridNativeTest) post({phase:'metrics',value:JSON.stringify({x:G.x(),y:G.y(),z:G.z(),yaw:G.yaw(),maxY,fps:G.fps(),firstPerson:!!G.firstPerson(),joined:gridState().joined,landmarks:G.metadata.landmarks.length,colliders:G.metadata.colliders.length,network:gridState().network})});
    },250);
    addEventListener('pagehide',()=>clearInterval(report),{once:true});
    // The suggestion is saved on this device by the shell's Rust module.
    const observer=new MutationObserver(()=>{
      if(document.getElementById('panelTitle').textContent==='Leave a suggestion') {
        const note=document.querySelector('#panelBody p');
        if(note && note.textContent!=='Suggestions are saved on this phone.') note.textContent='Suggestions are saved on this phone.';
      }
    });
    observer.observe(document.getElementById('panelBody'),{childList:true,subtree:true});
  });
  window.aethyrGridLifecycle=phase=>{
    if(!window.G)return;
    if(phase==='background') {
      if(window.gridState?.().joined) document.getElementById('playBtn').click();
      window.dispatchEvent(new Event('blur'));
      G.pause(1);
    } else if(phase==='close') {
      window.dispatchEvent(new Event('pagehide'));
      G.pause(1);
    } else G.pause(0);
  };
})();

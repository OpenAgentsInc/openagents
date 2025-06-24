let m=!1,C=null,o=null;const l={createConversation:async t=>(await(await fetch("/api/conversations",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({title:t})})).json()).id,addMessage:async(t,e,n)=>{await fetch(`/api/conversations/${t}/messages`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({role:e,content:n})})},getConversations:async()=>(await fetch("/api/conversations")).json(),updateConversationTitle:async(t,e)=>{await fetch(`/api/conversations/${t}`,{method:"PATCH",headers:{"Content-Type":"application/json"},body:JSON.stringify({title:e})})}};function v(t){const e=document.createElement("div");return e.textContent=t,e.innerHTML}async function E(){try{const t=await l.getConversations(),e=document.getElementById("thread-list-container");if(!e||t.length===0)return;const n=`
      <div class="mt-2">
        <div class="px-3 py-1 mb-0.5">
          <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
        </div>
        <ul class="flex flex-col gap-0.5">
          ${t.map(s=>`
            <li>
              <a href="/chat/${s.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${s.id===o?"bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]":"text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"}">
                <span>${v(s.title)}</span>
              </a>
            </li>
          `).join("")}
        </ul>
      </div>
    `;e.innerHTML=n}catch(t){console.error("Failed to load conversations:",t)}}async function $(t){if(m||!t.trim())return;const e=document.getElementById("chat-input"),n=document.getElementById("submit-button");if(!(!e||!n)){if(e.value="",e.style.height="auto",m=!0,n.disabled=!0,e.disabled=!0,!o)try{const s=t.slice(0,50)+(t.length>50?"...":"");o=await l.createConversation(s),window.history.replaceState({},"",`/chat/${o}`),await E()}catch(s){console.error("Failed to create conversation:",s),S();return}x("user",t);try{await l.addMessage(o,"user",t)}catch(s){console.error("Failed to save message:",s)}await H(t)}}function x(t,e){const n=document.getElementById("messages-container"),s=n?.querySelector("div");if(!s)return;const a=t==="user"?`
    <div class="message">
      <div class="message-avatar user">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
          <circle cx="12" cy="7" r="4"></circle>
        </svg>
      </div>
      <div class="message-content">
        <div class="message-author">You</div>
        <div class="message-body">${v(e)}</div>
      </div>
    </div>
  `:`
    <div class="message">
      <div class="message-avatar">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
          <line x1="12" y1="8" x2="12" y2="16"></line>
          <line x1="8" y1="12" x2="16" y2="12"></line>
        </svg>
      </div>
      <div class="message-content">
        <div class="message-author">Assistant</div>
        <div class="message-body">${e}</div>
      </div>
    </div>
  `;s.insertAdjacentHTML("beforeend",a),n&&(n.scrollTop=n.scrollHeight)}async function H(t){const e=window.getSelectedModel?.()||{id:"llama-4-scout-17b",provider:"cloudflare"},n={message:t,conversationId:o,model:e.id};if(e.provider==="openrouter"){const s=localStorage.getItem("openrouterApiKey");s&&(n.openrouterApiKey=s)}try{const s=e.provider==="cloudflare"?"/api/cloudflare":"/api/openrouter",a=await fetch(s,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(n)});if(!a.ok)throw new Error(`HTTP error! status: ${a.status}`);x("assistant",'<div class="thinking-indicator">Thinking...</div>');const r=document.getElementById("messages-container"),d=r?.querySelectorAll(".message"),y=d?.[d.length-1]?.querySelector(".message-body");if(!a.body||!y)throw new Error("No response body or message container");const h=a.body.getReader();C=h;const T=new TextDecoder;let g="",i="";for(;;){const{done:p,value:M}=await h.read();if(p)break;g+=T.decode(M,{stream:!0});const f=g.split(`
`);g=f.pop()||"";for(const b of f)if(b.startsWith("data: ")){const w=b.slice(6);if(w==="[DONE]")continue;try{const c=JSON.parse(w);if(c.content){i+=c.content;const B=await I(i);y.innerHTML=B,r.scrollTop=r.scrollHeight}}catch(c){console.error("Failed to parse SSE data:",c)}}}if(i&&o)try{await l.addMessage(o,"assistant",i)}catch(p){console.error("Failed to save assistant message:",p)}}catch(s){console.error("Chat error:",s);const r=document.getElementById("messages-container")?.querySelectorAll(".message"),u=r?.[r.length-1]?.querySelector(".message-body");u&&(u.innerHTML='<div class="error">Failed to get response. Please try again.</div>')}finally{C=null,S()}}async function I(t){let e=v(t);return e=e.replace(/```(\w+)?\n([\s\S]*?)```/g,(n,s,a)=>`<pre><code class="language-${s||"plaintext"}">${a.trim()}</code></pre>`),e=e.replace(/`([^`]+)`/g,"<code>$1</code>"),e=e.replace(/\*\*([^*]+)\*\*/g,"<strong>$1</strong>"),e=e.replace(/\*([^*]+)\*/g,"<em>$1</em>"),e=e.replace(/\n/g,"<br>"),e}function S(){const t=document.getElementById("chat-input"),e=document.getElementById("submit-button");t&&e&&(m=!1,e.disabled=!1,t.disabled=!1,t.focus())}window.sendMessage=$;
//# sourceMappingURL=chat.js.map

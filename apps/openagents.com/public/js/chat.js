let p=!1,S=null,r=null;const c={createConversation:async t=>(await(await fetch("/api/conversations",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({title:t})})).json()).id,addMessage:async(t,e,n)=>{await fetch(`/api/conversations/${t}/messages`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({role:e,content:n})})},getConversations:async()=>(await fetch("/api/conversations")).json(),updateConversationTitle:async(t,e)=>{await fetch(`/api/conversations/${t}`,{method:"PATCH",headers:{"Content-Type":"application/json"},body:JSON.stringify({title:e})})}};function m(t){const e=document.createElement("div");return e.textContent=t,e.innerHTML}async function B(){try{const t=await c.getConversations(),e=document.getElementById("thread-list-container");if(!e||t.length===0)return;const n=`
      <div class="mt-2">
        <div class="px-3 py-1 mb-0.5">
          <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
        </div>
        <ul class="flex flex-col gap-0.5">
          ${t.map(s=>`
            <li>
              <a href="/chat/${s.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${s.id===r?"bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]":"text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"}">
                <span>${m(s.title)}</span>
              </a>
            </li>
          `).join("")}
        </ul>
      </div>
    `;e.innerHTML=n}catch(t){console.error("Failed to load conversations:",t)}}async function I(t){if(p||!t.trim())return;const e=document.getElementById("chat-input"),n=document.getElementById("submit-button");if(!(!e||!n)){if(e.value="",e.style.height="auto",p=!0,n.disabled=!0,e.disabled=!0,!r)try{const s=t.slice(0,50)+(t.length>50?"...":"");r=await c.createConversation(s),window.history.replaceState({},"",`/chat/${r}`),await B()}catch(s){console.error("Failed to create conversation:",s),M();return}T("user",t);try{await c.addMessage(r,"user",t)}catch(s){console.error("Failed to save message:",s)}await H(t)}}function T(t,e){const n=document.getElementById("messages-container"),s=n?.querySelector("div");if(!s)return;const a=new Date().toLocaleTimeString([],{hour:"2-digit",minute:"2-digit"}),o=`
    <div class="message">
      <div class="message-block ${t}">
        <div class="message-header">
          <span class="message-role ${t}">${t==="user"?"You":"Assistant"}</span>
          <span class="message-time">${a}</span>
        </div>
        <div class="message-body">${t==="user"?m(e):e}</div>
      </div>
    </div>
  `;s.insertAdjacentHTML("beforeend",o),n&&(n.scrollTop=n.scrollHeight)}async function H(t){const e=window.getSelectedModel?.()||{id:"llama-4-scout-17b",provider:"cloudflare"},n={message:t,conversationId:r,model:e.id};if(e.provider==="openrouter"){const s=localStorage.getItem("openrouterApiKey");s&&(n.openrouterApiKey=s)}try{const s=e.provider==="cloudflare"?"/api/cloudflare/chat":"/api/openrouter/chat",a=await fetch(s,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(n)});if(!a.ok)throw new Error(`HTTP error! status: ${a.status}`);T("assistant",'<div class="thinking-indicator">Thinking...</div>');const o=document.getElementById("messages-container"),l=o?.querySelectorAll(".message"),y=l?.[l.length-1]?.querySelector(".message-body");if(!a.body||!y)throw new Error("No response body or message container");const f=a.body.getReader();S=f;const $=new TextDecoder;let u="",i="";for(;;){const{done:g,value:E}=await f.read();if(g)break;u+=$.decode(E,{stream:!0});const h=u.split(`
`);u=h.pop()||"";for(const v of h)if(v.startsWith("data: ")){const b=v.slice(6);if(b==="[DONE]")continue;try{const C=JSON.parse(b).choices?.[0]?.delta?.content;if(C){i+=C;const x=await D(i);y.innerHTML=x,o.scrollTop=o.scrollHeight}}catch(w){console.error("Failed to parse SSE data:",w)}}}if(i&&r)try{await c.addMessage(r,"assistant",i)}catch(g){console.error("Failed to save assistant message:",g)}}catch(s){console.error("Chat error:",s);const o=document.getElementById("messages-container")?.querySelectorAll(".message"),d=o?.[o.length-1]?.querySelector(".message-body");d&&(d.innerHTML='<div class="error">Failed to get response. Please try again.</div>')}finally{S=null,M()}}async function D(t){let e=m(t);return e=e.replace(/```(\w+)?\n([\s\S]*?)```/g,(n,s,a)=>`<pre><code class="language-${s||"plaintext"}">${a.trim()}</code></pre>`),e=e.replace(/`([^`]+)`/g,"<code>$1</code>"),e=e.replace(/\*\*([^*]+)\*\*/g,"<strong>$1</strong>"),e=e.replace(/\*([^*]+)\*/g,"<em>$1</em>"),e=e.replace(/\n/g,"<br>"),e}function M(){const t=document.getElementById("chat-input"),e=document.getElementById("submit-button");t&&e&&(p=!1,e.disabled=!1,t.disabled=!1,t.focus())}window.sendMessage=I;
//# sourceMappingURL=chat.js.map

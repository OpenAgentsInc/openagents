// Bounded local walking planner. No teleportation; the native controller
// still resolves collision each frame. Coordinates follow GymSite::GRID.
'use strict';
const GridNav = (() => {
  let walls=[];
  function configure(colliders) { walls=colliders.map(b=>[b.min[0],b.min[2],b.max[0],b.max[2]]); }
  const expanded=machines=>[...walls,...machines.map(m=>[m.x-.75,m.z-.6,m.x+.75,m.z+.6])].map(([x0,z0,x1,z1])=>[x0-.42,z0-.42,x1+.42,z1+.42]);
  function clear(a,b,rects) {
    for(const r of rects){let lo=0,hi=1;for(let axis=0;axis<2;axis++){
      const d=b[axis]-a[axis];
      if(Math.abs(d)<1e-8){if(a[axis]<=r[axis]||a[axis]>=r[axis+2]){lo=2;break;}}
      else{let t0=(r[axis]-a[axis])/d,t1=(r[axis+2]-a[axis])/d;if(t0>t1)[t0,t1]=[t1,t0];lo=Math.max(lo,t0);hi=Math.min(hi,t1);}
    }if(lo<hi && hi>1e-7 && lo<1-1e-7)return false;}return true;
  }
  function route(start,target,machines=[]) {
    const rects=expanded(machines), free=p=>p[0]>=-64&&p[0]<=64&&p[1]>=-40&&p[1]<=64&&clear(p,p,rects);
    if(!start.every(Number.isFinite)||!target.every(Number.isFinite))return null;
    const candidates=p=>{const out=[];for(let dx=-4;dx<=4;dx+=2)for(let dz=-4;dz<=4;dz+=2){const q=[Math.round(p[0]/2)*2+dx,Math.round(p[1]/2)*2+dz];if(free(q))out.push(q);}return out.sort((a,b)=>Math.hypot(a[0]-p[0],a[1]-p[1])-Math.hypot(b[0]-p[0],b[1]-p[1]));};
    if(!free(start)) {
      // The controller can stop exactly at a wall. Step away from its
      // contact edge before applying the planner's extra corner clearance.
      const containing=rects.find(r=>start[0]>r[0]&&start[0]<r[2]&&start[1]>r[1]&&start[1]<r[3]);
      if(!containing)return null;
      const r=containing,d=[start[0]-r[0],r[2]-start[0],start[1]-r[1],r[3]-start[1]],side=d.indexOf(Math.min(...d));
      if(d[side]>.16)return null;
      const p=[...start];
      if(side===0)p[0]=r[0]-.02;else if(side===1)p[0]=r[2]+.02;else if(side===2)p[1]=r[1]-.02;else p[1]=r[3]+.02;
      if(!free(p))return null;
      const rest=route(p,target,machines);return rest?[p,...rest]:null;
    }
    let goal=target;
    if(!free(goal)){goal=candidates(target)[0];if(!goal)return null;}
    if(clear(start,goal,rects))return[goal];
    const first=candidates(start).find(p=>clear(start,p,rects)),last=candidates(goal).find(p=>clear(goal,p,rects));
    if(!first||!last)return null;
    const key=p=>p.join(','),end=key(last),heap=[],cost=new Map([[key(first),0]]),from=new Map(),positions=new Map([[key(first),first]]);
    function push(item){heap.push(item);let i=heap.length-1;while(i){const parent=(i-1)>>1;if(heap[parent].f<=item.f)break;heap[i]=heap[parent];i=parent;}heap[i]=item;}
    function pop(){const top=heap[0],tail=heap.pop();if(heap.length){let i=0;while(i*2+1<heap.length){let child=i*2+1;if(child+1<heap.length&&heap[child+1].f<heap[child].f)child++;if(heap[child].f>=tail.f)break;heap[i]=heap[child];i=child;}heap[i]=tail;}return top;}
    const h=p=>Math.hypot(p[0]-last[0],p[1]-last[1]);push({p:first,g:0,f:h(first)});
    let found=false;
    for(let steps=0;heap.length&&steps<8000;steps++){
      const node=pop(),id=key(node.p);if(node.g!==cost.get(id))continue;if(id===end){found=true;break;}
      for(const dx of [-2,0,2])for(const dz of [-2,0,2]){if(!dx&&!dz)continue;const p=[node.p[0]+dx,node.p[1]+dz],next=key(p);if(!free(p)||!clear(node.p,p,rects))continue;const g=node.g+Math.hypot(dx,dz);if(g>=(cost.get(next)??Infinity))continue;cost.set(next,g);from.set(next,id);positions.set(next,p);push({p,g,f:g+h(p)});}
    }
    if(!found)return null;
    const points=[goal];for(let id=end;id;id=from.get(id))points.unshift(positions.get(id));
    // Collapse the grid path to the farthest visible corner each time.
    const result=[];let current=start,i=0;while(i<points.length){let far=i;while(far+1<points.length&&clear(current,points[far+1],rects))far++;result.push(points[far]);current=points[far];i=far+1;}return result;
  }
  return {route,clear,expanded,configure};
})();
if(typeof module!=='undefined')module.exports=GridNav;

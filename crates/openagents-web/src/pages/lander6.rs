//! The Grid worker bridge shared by /home, /pylons, /lander10, and /lander11.

pub(super) const RUNNER_TAIL: &str = "\
if(typeof WorkerGlobalScope!==\"undefined\"){self.onmessage=async e=>{await ready;const d=e.data;\
if(d.canvas)start_worker(d.canvas,d.sky,d.grid,d.w,d.h,d.dpr,d.time);\
else if(d.resize)resize_worker(d.w,d.h,d.dpr);\
else if(d.frames)postMessage({frames:frame_times()})}}\
else globalThis.lander5Runner={ready,pause,scene_time};";

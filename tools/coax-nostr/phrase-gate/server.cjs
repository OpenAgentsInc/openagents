// Isolated public-fixture service. No project keys, relay fetch or host imports.
const http=require('node:http'),fs=require('node:fs'),path=require('node:path');
const root=path.join(__dirname,'public'),mode=process.env.GATE_PROFILE??'baseline';
if(!['baseline','experimental'].includes(mode))throw Error('Unknown fixture profile');
const mime={'.html':'text/html; charset=utf-8','.js':'application/javascript','.json':'application/json','.wasm':'application/wasm'};
http.createServer((request,response)=>{
 const url=new URL(request.url,'http://fixture.invalid');
 if(url.pathname==='/'){response.writeHead(302,{location:'/'+mode+'/','cache-control':'no-store'});response.end();return;}
 const match=/^\/(baseline|experimental)\/(?:([a-z0-9.-]+))?$/.exec(url.pathname);
 if(!match){response.writeHead(404);response.end();return;}
 const filename=match[2]??'index.html',manifest=JSON.parse(fs.readFileSync(path.join(root,match[1],'manifest.json')));
 if(filename!=='manifest.json'&&!Object.hasOwn(manifest.assets,filename)){response.writeHead(404);response.end();return;}
 const bytes=fs.readFileSync(path.join(root,match[1],filename));response.writeHead(200,{'content-type':mime[path.extname(filename)]??'application/octet-stream','cache-control':'no-store','x-content-type-options':'nosniff'});response.end(bytes);
}).listen(Number(process.env.PORT??8080),'0.0.0.0');

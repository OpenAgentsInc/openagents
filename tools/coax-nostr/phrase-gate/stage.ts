// Assemble only the two public fixture artifacts, never compiler/test hooks.
import{mkdirSync,cpSync,copyFileSync,writeFileSync}from'node:fs';import{join,resolve}from'node:path';
const directory=resolve(process.argv[2]??'coax/bin/phrase-gate-cloud'),build=join(directory,'build');mkdirSync(directory,{recursive:true});
const child=Bun.spawn(['bun',join(import.meta.dir,'build.ts'),build,...(process.argv.includes('--release')?['--release']:[])],{stdout:'inherit',stderr:'inherit'});if(await child.exited)throw Error('Fixture build failed');
for(const name of['baseline','experimental'])cpSync(join(build,name),join(directory,'public',name),{recursive:true});
copyFileSync(join(import.meta.dir,'server.cjs'),join(directory,'server.cjs'));
writeFileSync(join(directory,'Dockerfile'),'FROM node:22-alpine\nWORKDIR /app\nCOPY public ./public\nCOPY server.cjs ./server.cjs\nUSER node\nENV PORT=8080\nCMD ["node","server.cjs"]\n');
writeFileSync(join(directory,'.dockerignore'),'build\n');console.log(directory);

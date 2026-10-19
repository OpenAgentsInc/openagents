// Cut the Android extra take (android-surface.mjs) by the four-app intro's rule: the same
// wall-clock cuts on every screen. Writes android.mp4, terminal.mp4, web.mp4 and a
// side-by-side android-sync.mp4 (terminal, web, Android) to the output folder.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import * as c from './cdp.mjs';
import { replayAt } from './replay-terminal.mjs';
import { encodeVideoAt } from './setup.mjs';

const [work, out] = process.argv.slice(2);
if (!work || !out) throw Error('Pass the captured take directory and an output directory.');
const take = JSON.parse(fs.readFileSync(path.join(work, 'take.json')));
if (!take.cleaned || !take.beats.all_finished) throw Error('Use a completed take whose disposable resources were cleaned up.');
fs.mkdirSync(out, { recursive: true });
const b = take.beats;
// Android reads the chat again every two seconds, so each cut ends after the phone's update.
const cuts = [[b.terminal_sent - 1.2, b.terminal_sent + .8],
  [b.all_updated - 1.8, b.all_updated + .2],
  [b.web_sent - 1.5, b.web_sent + .5],
  [b.all_finished - 1.7, b.all_finished + .3]];
if (cuts.some(([a, z], i) => a < b.start || z > b.end || i && a < cuts[i-1][1])) throw Error('The take needs manually chosen non-overlapping cuts.');
const epochs = cuts.flatMap(([a]) => Array.from({ length: 60 }, (_, i) => a + i / 30));
fs.writeFileSync(path.join(out, 'android-edit.json'), JSON.stringify({ chat: take.chat, cuts, epochs, seconds: epochs.length / 30, rule: 'The same wall-clock cuts apply to every application; no independent retiming.' }, null, 2));
encodeVideoAt(path.join(work, 'android-raw.mp4'), path.join(out, 'android.mp4'), epochs.map(t => t - take.androidStart));
c.encodeFramesAt(take.webFrames, path.join(out, 'android-take-web.mp4'), epochs, take.webSize);
await replayAt(path.join(work, 'terminal.cast'), path.join(out, 'android-take-terminal.mp4'), epochs.map(t => t - take.terminalStart));
const font = path.join(process.env.HOME, 'work/film-out/fonts/Geist-Regular.ttf');
const labels = path.join(out, 'android-labels.png');
execFileSync('python3', ['-c', `from PIL import Image, ImageDraw, ImageFont
import sys
im=Image.new('RGBA',(1920,1080),(0,0,0,0));d=ImageDraw.Draw(im)
f=ImageFont.truetype(sys.argv[1],24)
for text,x,y in [('Terminal',300,45),('Web',300,560),('Android',1170,45)]: d.text((x,y),text,font=f,fill=(201,201,208,255))
im.save(sys.argv[2])`, font, labels]);
const graph = [
  'color=c=0x0c0c0e:s=1920x1080:r=30:d=8[bg]',
  '[0:v]scale=690:448:force_original_aspect_ratio=decrease,pad=690:448:(ow-iw)/2:(oh-ih)/2:color=0x101012[t]',
  '[1:v]scale=690:468:force_original_aspect_ratio=decrease,pad=690:468:(ow-iw)/2:(oh-ih)/2:color=0x101012[w]',
  '[2:v]scale=420:934:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=420:934:(ow-iw)/2:(oh-ih)/2:color=0x101012[a]',
  '[bg][t]overlay=300:84[x]', '[x][w]overlay=300:600[y]', '[y][a]overlay=1170:84[z]',
  '[z][3:v]overlay=0:0,format=yuv420p[v]'
].join(';');
execFileSync('ffmpeg', ['-v','error','-y',...['android-take-terminal','android-take-web','android'].flatMap(n=>['-i',path.join(out,n+'.mp4')]),'-loop','1','-i',labels,'-filter_complex',graph,'-map','[v]','-t','8','-c:v','libx264','-preset','slow','-crf','16','-movflags','+faststart',path.join(out,'android-sync.mp4')], {stdio:'inherit'});
console.log(path.join(out, 'android-sync.mp4'));

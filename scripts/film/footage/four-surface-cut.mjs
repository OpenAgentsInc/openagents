// Render one eight-second intro from shared wall-clock cuts in a four-app take.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import * as c from './cdp.mjs';
import { replayAt } from './replay-terminal.mjs';
import { encodeVideoAt } from './setup.mjs';

const [work, out] = process.argv.slice(2);
if (!work || !out) throw Error('Pass the captured take directory and an output directory.');
const take = JSON.parse(fs.readFileSync(path.join(work, 'take.json')));
if (!take.cleaned || !take.beats.all_four_finished) throw Error('Use a completed take whose disposable resources were cleaned up.');
fs.mkdirSync(out, { recursive: true });
const b = take.beats;
const cuts = [[b.terminal_sent - 1.2, b.terminal_sent + .8],
  [b.all_four_updated - 1.8, b.all_four_updated + .2],
  [b.web_sent - 1.5, b.web_sent + .5],
  [b.all_four_finished - 1.7, b.all_four_finished + .3]];
if (cuts.some(([a, z], i) => a < b.start || z > b.end || i && a < cuts[i-1][1])) throw Error('The take needs manually chosen non-overlapping cuts.');
const epochs = cuts.flatMap(([a]) => Array.from({ length: 60 }, (_, i) => a + i / 30));
fs.writeFileSync(path.join(out, 'edit.json'), JSON.stringify({ chat: take.chat, cuts, epochs, seconds: epochs.length / 30, rule: 'The same wall-clock cuts apply to all four applications; no independent retiming.' }, null, 2));
const dimensions = file => {
  const d = JSON.parse(execFileSync('ffprobe', ['-v','error','-show_entries','stream=width,height','-of','json',file],{encoding:'utf8'}));
  return { width: d.streams[0].width, height: d.streams[0].height, dpr: 1 };
};
if (!process.argv.includes('--compose-only')) {
const desktop = JSON.parse(fs.readFileSync(path.join(work, 'desktop/frames.json'))).map(f => ({ file: path.join(work, 'desktop/frames', f.frame), t: f.epoch }));
c.encodeFramesAt(desktop, path.join(out, 'desktop.mp4'), epochs, dimensions(desktop[0].file));
c.encodeFramesAt(take.webFrames, path.join(out, 'web.mp4'), epochs, { width:1120, height:760, dpr:1 });
encodeVideoAt(path.join(work, 'phone-raw.mp4'), path.join(out, 'phone.mp4'), epochs.map(t => t - take.phoneStart));
await replayAt(path.join(work, 'terminal.cast'), path.join(out, 'terminal.mp4'), epochs.map(t => t - take.terminalStart));
}
const font = path.join(process.env.HOME, 'work/film-out/fonts/Geist-Regular.ttf');
const labels = path.join(out, 'labels.png');
execFileSync('python3', ['-c', `from PIL import Image, ImageDraw, ImageFont
import sys
im=Image.new('RGBA',(1920,1080),(0,0,0,0));d=ImageDraw.Draw(im)
f=ImageFont.truetype(sys.argv[1],24)
for text,x,y in [('Desktop',30,45),('Web',750,45),('Terminal',390,560),('iPhone',1470,45)]: d.text((x,y),text,font=f,fill=(201,201,208,255))
im.save(sys.argv[2])`, font, labels]);
// Three landscape applications and the full-height phone, with room around every screen.
const graph = [
  'color=c=0x0c0c0e:s=1920x1080:r=30:d=8[bg]',
  '[0:v]scale=690:468:force_original_aspect_ratio=decrease,pad=690:468:(ow-iw)/2:(oh-ih)/2:color=0x101012[d]',
  '[1:v]scale=690:468:force_original_aspect_ratio=decrease,pad=690:468:(ow-iw)/2:(oh-ih)/2:color=0x101012[w]',
  '[2:v]scale=420:912:force_original_aspect_ratio=decrease,pad=420:912:(ow-iw)/2:(oh-ih)/2:color=0x101012[p]',
  '[3:v]scale=690:448:force_original_aspect_ratio=decrease,pad=690:448:(ow-iw)/2:(oh-ih)/2:color=0x101012[t]',
  '[bg][d]overlay=30:84[a]', '[a][w]overlay=750:84[b]', '[b][t]overlay=390:600[c]', '[c][p]overlay=1470:84[e]',
  '[e][4:v]overlay=0:0,format=yuv420p[v]'
].join(';');
execFileSync('ffmpeg', ['-v','error','-y',...['desktop','web','phone','terminal'].flatMap(n=>['-i',path.join(out,n+'.mp4')]),'-loop','1','-i',labels,'-filter_complex',graph,'-map','[v]','-t','8','-c:v','libx264','-preset','slow','-crf','16','-movflags','+faststart',path.join(out,'episode-290-four-apps-intro.mp4')], {stdio:'inherit'});
console.log(path.join(out,'episode-290-four-apps-intro.mp4'));

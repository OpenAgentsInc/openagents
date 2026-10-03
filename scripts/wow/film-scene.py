#!/usr/bin/env python3
"""Record a private WoW set with real NPC yells, a camera cut, and bow shots."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('scene', type=Path)
parser.add_argument('--root', type=Path, default=Path.home() / 'wow-gym')
parser.add_argument('--client', type=Path)
parser.add_argument('--wine', default='wine')
parser.add_argument('--xvfb', default='Xvfb')
parser.add_argument('--ffmpeg', default='ffmpeg')
parser.add_argument('--xclip', default='xclip')
args = parser.parse_args()
os.umask(0o077)
root = args.root.resolve()
scene = json.loads(args.scene.read_text())
client = (args.client or root / 'video-client').resolve()
output = root / 'scene'
output.mkdir(exist_ok=True)
credentials = root / 'accounts.json'
if credentials.stat().st_mode & 0o077:
    raise SystemExit('accounts.json must be private (0600)')
account = scene['actor']['account']
if account != 'GYMSETUP':
    raise SystemExit('filming requires the trusted setup account, not a gym worker')
password = next(row['password'] for row in json.loads(credentials.read_text()) if row['account'] == account)
lease = (root / 'leases/account-GYMSETUP.lock').open('a+')
fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
if Path('/tmp/.X11-unix/X97').exists():
    raise SystemExit('display :97 is already in use; retire the previous filming session')
env = os.environ.copy()
env.update(DISPLAY=':97', WINEPREFIX=str(root / 'video-prefix'), WINEDEBUG='-all', WINEDLLOVERRIDES='d3d9=n,b')
env['LD_LIBRARY_PATH'] = '/run/opengl-driver/lib:' + env.get('LD_LIBRARY_PATH', '')
events = []
recorder = None
wine = None
xvfb = None
clock = None

def x(*words, **kwargs):
    subprocess.run(['xdotool', *words], env=env, check=True, **kwargs)

def press(key):
    x('keydown', key)
    time.sleep(0.15)
    x('keyup', key)
    time.sleep(0.2)

def chat(text):
    if len(text) > 255:
        raise ValueError('client chat command exceeds 255 characters')
    press('Return')
    subprocess.run([args.xclip, '-selection', 'clipboard'], input=text.encode(), env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    x('keydown', 'Control_L', 'keydown', 'v')
    time.sleep(0.15)
    x('keyup', 'v', 'keyup', 'Control_L')
    time.sleep(0.2)
    press('Return')
    time.sleep(0.4)

def lua(code):
    chat('/script ' + code)

def capture(name):
    subprocess.run([args.ffmpeg, '-y', '-f', 'x11grab', '-video_size', '1280x720', '-i', ':97', '-frames:v', '1', str(output / name)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)

def timestamp(seconds):
    milliseconds = int(seconds * 1000)
    return '%02d:%02d:%02d,%03d' % (milliseconds // 3600000, milliseconds // 60000 % 60, milliseconds // 1000 % 60, milliseconds % 1000)

try:
    xvfb = subprocess.Popen([args.xvfb, ':97', '-screen', '0', '1280x720x24', '-ac', '-nolisten', 'tcp'], env=env, stdout=(output / 'xvfb.log').open('w'), stderr=subprocess.STDOUT)
    time.sleep(1)
    wine = subprocess.Popen([args.wine, 'VanillaFixes.exe', 'WoW_tweaked.exe'], cwd=client, env=env, stdout=(output / 'wine.log').open('w'), stderr=subprocess.STDOUT)
    time.sleep(7)
    x('mousemove', '650', '395', 'click', '1', 'key', 'ctrl+a')
    x('type', '--clearmodifiers', account)
    x('key', 'Tab')
    x('type', '--clearmodifiers', '--file', '-', input=password.encode())
    x('key', 'Return')
    time.sleep(3)
    x('mousemove', '640', '670', 'click', '1')
    time.sleep(5)
    lua('LoggingChat(1)')
    lua('ClearTarget()')
    chat('.gm on')
    lua('SendChatMessage(".levelup "..(60-UnitLevel("player")),"SAY")')
    chat('.maxskill')
    lua('if not GetInventoryItemLink("player",18) then SendChatMessage(".additem %d 1","SAY") end' % scene['actor']['bow'])
    lua('if GetInventoryItemCount("player",0)<20 then SendChatMessage(".additem %d 200","SAY") end' % scene['actor']['arrows'])
    for item in ['Polished Shortbow', 'Rough Arrow']:
        lua('for i=1,16 do local s=GetContainerItemLink(0,i);if s and string.find(s,"%s") then UseContainerItem(0,i) end end' % item)
    lua('ChatFrame1:AddMessage("Bow: "..(GetInventoryItemLink("player",18) or "none").." Ammo: "..GetInventoryItemCount("player",0))')
    capture('bow-setup.png')
    position = scene['camera']['position']
    chat('.go xyzo %s %s %s %s %s' % (*position, scene['camera']['orientation'], scene['map']))
    time.sleep(3)
    chat('.gm visible on')
    chat('.gm off')
    lua('ShowNameplates();ShowFriendNameplates();SetCVar("UnitNameNPC",1)')
    lua('SetView(2);CameraZoomIn(50)')
    lua('MoveViewDownStart(0.1);local f=CreateFrame("Frame");local t=GetTime()+0.5;f:SetScript("OnUpdate",function() if GetTime()>t then MoveViewDownStop();f:SetScript("OnUpdate",nil) end end)')
    lua('MainMenuBar:Hide();MinimapCluster:Hide();PlayerFrame:Hide();BuffFrame:Hide()')
    chat('/console showTutorials 0')
    lua('for k,v in pairs(getfenv(0)) do if string.find(k,"Tutorial") and type(v)=="table" and v.Hide then v:Hide() end end')
    lua('UIErrorsFrame:Hide();TargetFrame:Hide()')
    time.sleep(2)
    capture('cinematic-ready.png')
    log = (output / 'record.log').open('w')
    recorder = subprocess.Popen([args.ffmpeg, '-y', '-f', 'x11grab', '-framerate', '30', '-video_size', '1280x720', '-i', ':97', '-t', str(scene['duration']), '-vf', 'crop=1280:688:0:30', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(output / 'anthropic-raw.mp4')], env=env, stdout=log, stderr=subprocess.STDOUT)
    clock = time.monotonic()
    cues = [(row['at'], 'dialogue', row) for row in scene['dialogue']]
    cues += [(scene['cut_at'], 'cut', {}), *[(at, 'shot', {}) for at in scene['shots']]]
    for at, kind, row in sorted(cues, key=lambda cue: cue[0]):
        time.sleep(max(0, at - (time.monotonic() - clock)))
        if kind == 'dialogue':
            lua('TargetNearestEnemy();if UnitName("target")=="Claude" then TargetNearestEnemy() end')
            chat('.npc playemote %d' % row['emote'])
            chat('.npc yell ' + row['text'])
        elif kind == 'cut':
            lua('CameraZoomOut(8)')
        else:
            lua('TargetByName("Cultist of Anthropic",true);CastSpellByName("Shoot Bow")')
        events.append({**row, 'scheduled_at': at, 'at': time.monotonic() - clock, 'kind': kind})
    recorder.wait(timeout=20)
    if recorder.returncode:
        raise RuntimeError('video capture failed')
    recorder = None
    lua('ChatFrame1:AddMessage("Ammo after shots: "..GetInventoryItemCount("player",0))')
    capture('bow-after.png')
    (output / 'film-events.json').write_text(json.dumps(events, indent=2) + '\n')
    subtitles = []
    for row in events:
        if row['kind'] == 'dialogue':
            start = row['at']
            subtitles.append('%d\n%s --> %s\nCultist of Anthropic: %s\n' % (len(subtitles) + 1, timestamp(start), timestamp(start + 3.5), row['text']))
    (output / 'anthropic.srt').write_text('\n'.join(subtitles))
    subprocess.run([args.ffmpeg, '-y', '-i', 'anthropic-raw.mp4', '-vf', "subtitles=anthropic.srt:force_style='FontSize=20,Outline=2,MarginV=24'", '-c:v', 'libx264', '-preset', 'fast', '-crf', '20', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', 'anthropic-ritual.mp4'], cwd=output, stdout=(output / 'encode.log').open('w'), stderr=subprocess.STDOUT, check=True)
    print(output / 'anthropic-ritual.mp4')
finally:
    if recorder is not None:
        recorder.terminate()
        recorder.wait(timeout=10)
    if wine is not None:
        try:
            lua('ClearTarget()')
            chat('.gm on')
            chat('.go xyzo -8949.95 -132.493 83.5312 0 0')
            time.sleep(10)
            lua('SendChatMessage(".levelup "..(1-UnitLevel("player")),"SAY")')
            chat('.gm off')
            chat('.save')
        finally:
            subprocess.run([str(Path(args.wine).with_name('wineserver')) if '/' in args.wine else 'wineserver', '-k'], env=env, check=False)
            wine.wait(timeout=10)
    if xvfb is not None:
        xvfb.terminate()
        xvfb.wait(timeout=10)
    lease.close()

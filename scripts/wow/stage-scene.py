#!/usr/bin/env python3
"""Stage a reversible filming set in the private vmangos realm database."""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('scene', type=Path)
parser.add_argument('--root', type=Path, default=Path.home() / 'wow-gym')
parser.add_argument('--mariadb', default='mariadb')
args = parser.parse_args()
os.umask(0o077)
scene = json.loads(args.scene.read_text())
root = args.root.resolve()
archive = root / 'scene'
archive.mkdir(exist_ok=True)
client = [args.mariadb, '--no-defaults', '--socket=' + str(root / 'mysql.sock'), '-u', os.environ['USER'], '-N', '-B', 'mangos']

def sql(text):
    return subprocess.check_output(client, input=text.encode()).decode()

monster, cultists = scene['monster'], scene['cultists']
assert scene['map'] == 289 and cultists['count'] == 12
assert [monster['entry'], cultists['entry']] == [900001, 900002]
room = 'map=289 AND position_x BETWEEN -50 AND 35 AND position_y BETWEEN 110 AND 170 AND position_z BETWEEN 80 AND 88 AND id NOT IN (900001,900002)'
restore = archive / 'restore-original-spawns.sql'
if not restore.exists():
    rows = sql('SELECT guid,patch_min,patch_max FROM creature WHERE ' + room).splitlines()
    restore.write_text(''.join('UPDATE creature SET patch_min=%s,patch_max=%s WHERE guid=%s;\n' % (parts[1], parts[2], parts[0]) for parts in (row.split('\t') for row in rows)))
columns = [row.split('\t')[0] for row in sql('SHOW COLUMNS FROM creature_template').splitlines() if not row.startswith('entry\t')]
quoted = ','.join('`' + column + '`' for column in columns)
statements = ['UPDATE creature SET patch_min=0,patch_max=0 WHERE ' + room + ';']
for actor in [monster, cultists]:
    if sql('SELECT COUNT(*) FROM creature_template WHERE entry=%d' % actor['entry']).strip() == '0':
        statements.append('INSERT INTO creature_template (`entry`,' + quoted + ') SELECT %d,%s FROM creature_template WHERE entry=%d ORDER BY patch DESC LIMIT 1;' % (actor['entry'], quoted, actor['source']))
    assert all(character.isalpha() or character == ' ' for character in actor['name'])
    statements.append("UPDATE creature_template SET name='%s',subname=NULL,display_scale1=%f,faction=14,npc_flags=0,ai_name='NullAI',script_name='',movement_type=0,spell_list_id=0,auras='',equipment_id=0,health_multiplier=20 WHERE entry=%d;" % (actor['name'], actor.get('scale', 1), actor['entry']))
if sql('SELECT COUNT(*) FROM creature WHERE id IN (900001,900002)').strip() == '0':
    x, y, z = monster['position']
    statements.append('INSERT INTO creature(id,map,position_x,position_y,position_z,orientation,wander_distance,movement_type) VALUES(900001,289,%f,%f,%f,0,0,0);' % (x, y, z))
    for index in range(cultists['count']):
        angle = index * 2 * math.pi / cultists['count']
        statements.append('INSERT INTO creature(id,map,position_x,position_y,position_z,orientation,wander_distance,movement_type) VALUES(900002,289,%f,%f,%f,%f,0,0);' % (x + cultists['radius'] * math.cos(angle), y + cultists['radius'] * math.sin(angle), z, (angle + math.pi) % (2 * math.pi)))
        statements.append('INSERT INTO creature_addon(guid,patch,emote_state) VALUES(LAST_INSERT_ID(),0,%d);' % (68 if index % 3 == 0 else 193))
actor_name = scene['actor']['character']
assert actor_name.isalpha() and scene['actor']['account'] == 'GYMSETUP'
actor_guid = sql("SELECT guid FROM characters.characters WHERE name='%s'" % actor_name).strip()
if not actor_guid.isdigit():
    raise SystemExit('create the trusted filming character before staging')
# Pin only the bow proficiency and Shoot ability needed by this filming actor.
statements.append('INSERT IGNORE INTO characters.character_spell(guid,spell,active,disabled) VALUES(%s,264,1,0),(%s,2480,1,0);' % (actor_guid, actor_guid))
statements.append('INSERT IGNORE INTO characters.character_skills(guid,skill,value,`max`) VALUES(%s,45,5,5);' % actor_guid)
sql('\n'.join(statements))
assert sql('SELECT COUNT(*) FROM creature WHERE id IN (900001,900002)').strip() == '13'
(archive / 'scene.json').write_text(json.dumps(scene, indent=2) + '\n')
print('Staged Claude and twelve Cultist of Anthropic NPCs. Restart the private realm to load the set.')

# Firm reason: 6 (Unreal's Sequencer editor SDK has no C ABI).
# Read channel/default/key values; never evaluate or render the sequence.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-source'
rows = []
for shot in ['0050', '0060', '0100', '0270', '0290', '0300', '0310', '0330']:
    sequence = unreal.load_asset('/Game/Slay/Sequences/TF/Shots/TF' + shot + '/TF' + shot + '_01')
    result = {'shot': shot, 'bindings': []}
    for binding in sequence.get_bindings():
        entry = {'name': binding.get_name(), 'tracks': []}
        for track in binding.get_tracks():
            t = {'name': str(track.get_display_name()), 'class': track.get_class().get_name(), 'sections': []}
            for section in track.get_sections():
                s = {'class': section.get_class().get_name(), 'channels': []}
                if section.has_start_frame():
                    s['start'] = section.get_start_frame()
                if section.has_end_frame():
                    s['end'] = section.get_end_frame()
                if isinstance(section, unreal.MovieSceneSkeletalAnimationSection):
                    params = section.get_editor_property('params')
                    s['animation'] = params.animation.get_path_name() if params.animation else None
                    s['offset'] = str(params.start_frame_offset)
                for channel in section.get_all_channels():
                    c = {'name': str(channel.channel_name), 'class': channel.get_class().get_name(), 'keys': []}
                    if hasattr(channel, 'get_default'):
                        c['default'] = str(channel.get_default())
                    if hasattr(channel, 'get_keys'):
                        for key in channel.get_keys():
                            time = key.get_time(time_unit=unreal.MovieSceneTimeUnit.DISPLAY_RATE)
                            c['keys'].append({'frame': time.frame_number.value, 'subframe': time.sub_frame, 'value': str(key.get_value())})
                    s['channels'].append(c)
                t['sections'].append(s)
            entry['tracks'].append(t)
        result['bindings'].append(entry)
    rows.append(result)
(root / 'shot-channels.json').write_text(json.dumps(rows, indent=2))
unreal.log('SLAY_CHANNELS_RECEIPT ' + str(len(rows)))

# Robot Lab room

The room uses the same checked scenes, saved recordings, and drawing code as
chat. It contains two SO-101 inspection benches, colored blocks, and hollow
cups. Task physics stays on the configured simulation host; room scenery cannot
change a recorded task outcome.

Enter the Robot Lab portal from the plaza, or run `verse --robot-lab`.
**Prev** and **Next** choose a setting; **−** and **+** change the scene or
attempt, playback speed, selected joint, joint pose, or experiment. Recorded
poses stay read-only. **Play**, **Pause**, **Step**, and **Reset** replay saved
frames without starting inference. **Plaza** returns and releases the room.

To open the same private lab as chat, pass `--robot-lab-config FILE`:

```json
{
  "root": "/private/robot-files",
  "project": "PROJECT",
  "subject": {"document": "SCENE_OR_ATTEMPT", "version": "VERSION_SHA"},
  "control_url": null,
  "watch_job": null
}
```

The file store reads and checks immutable documents and assets. Pose edits
append versions, preserving previous chat references. File reads and commands
run off the frame thread. `watch_job` follows saved physical steps from a run
started in chat; finished attempts open at their last frame for replay.

Set `control_url` to the explicitly configured loopback workbench service to
start or control a saved experiment. The room checks that the service serves
the same project and offers execution controls. **Start** begins an interactive
attempt, paused. **Run**, **Pause**, **Step**, **Reset**, and **Stop** call the
host's typed controls. Reset preserves the previous recording. Leaving pauses
an interactive attempt started here; watching or leaving a batch does not stop
it. Manual recorder experiments are not offered through this runner.

Capture the room without opening a window:

```sh
verse --robot-lab --capture /private/robot-room.png --size 1280x720
verse --robot-lab --robot-lab-config /private/room.json \
  --capture /private/recorded-room.png --size 1280x720
```

Native default inspection reads the pinned licensed SO-101 catalog. A device
without those local files supplies checked scene and mesh data through
`Room::new` and `Room::open`. It does not fetch model code or infer from a view.

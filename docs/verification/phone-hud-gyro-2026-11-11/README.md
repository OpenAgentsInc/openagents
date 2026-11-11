# Upright phone HUD and gyro aim (#13189)

Headless Chrome shots of `/worlds/archery-range`, and the iPhone 17 Pro simulator.

| Shot | What |
| --- | --- |
| `before-390.jpg`, `before-430.jpg` | Before: the full tally and its four buttons, two bar rows with the class plate, the GFX, FPS and place lines |
| `after-390.jpg`, `after-430.jpg` | After: the portrait, one tally line, GYRO and JUMP, the stick and one bar row ending in MORE. About 73% of the screen is clear world at 390x844, counting the stick's ring as covered |
| `after-430-more.jpg` | MORE opened: the rest of the bar in a row over it |
| `after-430-tally.jpg` | The tally line tapped: the whole tally with FOOT, HORSE, DRAGON and RESET |
| `after-430-gyro.jpg` | GYRO's menu: on or off, while aiming or always, speed, up-down |
| `before-desk.jpg`, `after-desk.jpg` | Desktop at 1280x800, unchanged |
| `gyro-turn-1.jpg`, `gyro-turn-3.jpg` | `motion-turn-left.json` (one second at 60 Hz: 45 deg/s left, the top tilting 10 deg/s, with noise) played as `devicemotion` events with GYRO set to ALWAYS: the camera's turn went 0 to 0.653 rad and its tilt 0.350 to 0.494 rad (probes 843, 844) |
| `ios-gyro-1-drawn.jpg`, `ios-gyro-2-turned.jpg` | The app, `--offer-gyro --do "...down:1:109,782;...;tilt:0.7,0.25:1.5"`: the bow drawn, then the scripted turn through the page host turns the aim left and down |

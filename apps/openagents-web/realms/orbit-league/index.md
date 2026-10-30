# Orbit League

A Verse realm at https://openagents.com/realms/orbit-league: electric roadsters playing ball in a glass arena in orbit. You drive the blue car against an orange bot; score more goals in five minutes. Every 90 seconds the station spins down and the arena goes zero-g for half a minute.

## How to play

- Keyboard: W and S (or the up and down arrows) drive and reverse, A and D steer, Space jumps (press again in the air for a second jump), Shift boosts, C switches between the chase camera and ball cam, Escape goes back to the galaxy.
- Phone: drag the left thumb to steer and drive; JUMP and BOOST are at the bottom right, BALL CAM at the top right.
- Drive into the ball to hit it. Boost fills slowly while your wheels are on the ground.
- In zero-g the ball and the cars float. Boost is how you get anywhere; in the air W and S pitch the car, A and D turn it.
- A tie at full time goes to golden goal. PLAY AGAIN (or Enter) starts a new match.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The physics is the Verse engine's space primitives (`docs/verse/space.md`): `verse/vehicle.coax` (wheels, suspension, grip, jumps), `verse/contact.coax` (the ball against the cars, the walls and the goal frames), `verse/gravity.coax` (the station's spin as a gravity field) and `verse/thrust.coax` (the boost), on `verse/rigid.coax`.
- `apps/openagents-web/orbitleague.coax` is the game: the match, the bot, the camera and the HUD. `orbitleague-face.c` binds it for the browser and `orbitleague.wgsl` traces the arena, the cars, the ball and the space round the station.
- The design is `docs/verse/game-ideas/orbit-league.md`. The page is written from `realms.json` by `coax/web/realms.ts`; the contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.

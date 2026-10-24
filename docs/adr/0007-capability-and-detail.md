# 0007: One capability and detail language

Status: Accepted in part. The owner asked for "a shared LOD and capability
language" across devices. The data model below is the starting design;
names and curves may change as it is built.

## Decision

- **One device profile** describes what a device can do: GPU adapter class,
  WebGPU features and limits, memory budget, display size, scale and refresh
  rate, input kinds, power and thermal state, network, and user preferences
  (reduced motion, reduced transparency, text scale). It is probed, never
  guessed from the user agent.
- **One detail controller** turns the profile and measured frame time into a
  single `Detail` value from 0 (floor) to 1 (full). It uses GPU timestamps
  when available and frame pacing otherwise, with hysteresis.
- **Knobs map `Detail` to settings**, for worlds (render scale, anti-aliasing,
  occlusion, cluster LOD bias, crowd tier distances, shadows, foliage) and for
  the interface (backdrop blur, shadows, motion, effects).
- **Floors are hard.** Text, focus indicators, and hit targets have no knob.
  User preferences are limits, not inputs. A zone declares its knobs and
  floors in its manifest. The shell refuses to open a zone whose floor the
  device can't meet, and says why.
- **Every frame report and failure report** includes the profile and the
  current `Detail`.

## Not yet decided

The pinned qualification devices and the frame budget per device class are
set during qualification.

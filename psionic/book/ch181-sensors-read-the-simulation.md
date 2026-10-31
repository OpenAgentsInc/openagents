# 181. Sensors read the simulation

> **Status: unimplemented — design draft.** Nothing here runs yet.

A policy never sees the true state; it sees what sensors report, late, noisy
and at their own rates. A simulator that hands a policy perfect state trains
something that fails on hardware. Sensors in this engine are therefore
read-only functions of state with their own clocks, noise models and
delays, stamped the same way Coax R18 stamps real ones.

## What this chapter teaches

- Proprioception: joint positions, velocities and torques.
- Inertial measurement units from body accelerations and rates, with bias
  and noise.
- Contact and force/torque sensors, and tactile arrays.
- Ray-cast range sensors: lidar and depth.
- Sample-and-hold, delay and noise models; stamps.

## The models

An IMU at point `r` on body `B` reports specific force and rate in its frame:

```jai
a_meas = Rᵀ (a_B + α × r + ω × (ω × r) − g) + b_a + n_a
ω_meas = Rᵀ ω + b_g + n_g,   ḃ = n_b                 // random-walk bias
```

Lidar casts rays against the scene's geometry (chapter 169's queries or a
BVH of triangles) and returns the first hit distance, with range noise and
dropout. A tactile array samples contact pressure over a pad's surface from
the contact solver's forces. Every reading has a sample time, a delay and a
noise draw from a per-sensor counter-based stream.

## The design in Psionic

Sensors are read-only stages after the physics in chapter 165's schedule
and cannot write state (an effect restriction, R11). Each sensor has a
rate; between samples, readings hold. Readings carry R18 stamps: source,
sample time, delay and calibration identity. Tactile models are labelled
by whether they are calibrated against a real sensor; uncalibrated ones say
so in their provenance (R20).

## What it builds

- `psionic/lib/sim/sensors.coax`: joint, IMU, force/torque, contact,
  tactile, lidar and depth ray casting, noise and delay models.

## Acceptance checks and oracles

- IMU on a body in known motion (constant spin, free fall, circular path)
  against analytic specific force and rate.
- Lidar against analytic distances to planes, spheres and boxes.
- Force/torque at a fixed joint equals the static load.
- Noise statistics match declared models; reading streams reproducible
  from seed; sensors never change the state digest.

## How Genesis does it

Genesis provides IMU, contact, force, tactile (uncalibrated), lidar and
depth ray-cast sensors and a temperature grid
([sensors](https://genesis-world.readthedocs.io/en/latest/user_guide/advanced_topics/sensors.html)).
We plan the same set, with stamps, delays and calibration provenance as
part of every reading.

## Dependencies

Chapters 165 to 176; ray queries from 169. From Coax: R11 effects, R13
clocks, R18 stamped sensors, R20 provenance.

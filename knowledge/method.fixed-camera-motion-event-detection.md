---
id: method.fixed-camera-motion-event-detection
version: 1
kind: method
title: Detect a brief motion event from fixed-camera video
summary: >-
  For a fixed-camera clip with a known empty-scene background, detect a moving
  subject from background-subtracted masks, derive a trajectory, and infer
  event boundaries from motion and contact cues. Applies to short events such
  as jumps or crossings when generic object detection is unavailable.
tags: [computer-vision, video, background-subtraction, event-detection]
applies_when: >-
  A coding task asks for event frame indices from a fixed-view video with a
  stable or initially empty background.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - video-processing
  cites:
    - "Bradski and Kaehler, Learning OpenCV: Computer Vision with the OpenCV Library, 1st ed., Ch. 10, Video Analysis"
    - Moeslund, Hilton, and Krüger, A Survey of Advances in Vision-Based Human Motion Capture and Analysis, Computer Vision and Image Understanding 104(2–3), 2006, sections 2–3
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
Use a staged pipeline: inspect clip dimensions, frame rate, and a contact sheet; define the event operationally in frame terms; estimate a background from an empty frame; blur before differencing; compensate modest global illumination changes; threshold the residual; remove small components and merge nearby moving fragments; then select a plausible subject component using size, position, and continuity. Reduce each detection to robust motion features (for example, horizontal position and lowest-foot location) and reject missing or implausible observations rather than silently treating them as real.

Locate a meaningful spatial event such as crossing a known obstacle, then examine a time window around it. Estimate local travel speed robustly over multiple frames, not from adjacent detections alone. Infer takeoff and landing by combining vertical excursion with ground-contact evidence before and after the apex. Distinguish the requested boundary convention explicitly—for example, first airborne frame versus last ground-contact frame—and use that convention consistently. Missing-detection gaps, moving shadows, obstacle pixels, compression noise, partial occlusion, and the subject touching the image border can all fragment masks; test these conditions rather than assuming the largest raw connected component is the subject.

Keep file decoding, observation extraction, event inference, and serialization separate. Validate the output syntax/schema independently from the vision logic. Test transformed clips that should preserve or predictably transform frame indices: temporal trimming or padding, frame-rate changes, mirroring, illumination shifts, and event-free footage.

Sources: Gary Bradski and Adrian Kaehler, *Learning OpenCV: Computer Vision with the OpenCV Library*, 1st ed., Ch. 10, “Video Analysis”; Thomas B. Moeslund, Anja Hilton, and Volker Krüger, “A Survey of Advances in Vision-Based Human Motion Capture and Analysis,” *Computer Vision and Image Understanding* 104(2–3), 2006, sections 2–3.

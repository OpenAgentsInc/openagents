// Character animation: per-bone blending of the baked clips (issue #9).
//
// assets/anims.bin holds every clip baked at 30 fps for each body as each
// joint's skinning transform (pose world times inverse bind, rigid).
// assets/skeleton.bin (tools/pack_skeleton.py) adds each joint's parent and
// where it sits in the bind pose. That is enough to blend per bone without
// rebuilding the bind pose: a joint's motion relative to its parent,
// C = skin(parent)^-1 * skin(joint), turns the bone about the joint's bind
// position (only the root and pelvis also move), so rotations blend there
// (nlerp, which up to a constant on each side is the same as blending the
// joint's local rotation) and the skinning transforms chain back up from
// the blended C's. One layer at weight 1 gives back the baked transforms
// exactly.
//
// A Pose is up to four layers over the bind pose, applied in order:
//   - an override layer blends its clip in by its weight times its mask's
//     weight at each joint (full body, upper or lower body);
//   - an additive layer adds its clip's change from a reference frame (a
//     hit flinch) on top of what the layers below made.
// Every layer is sampled between its two nearest baked frames, so a 30 fps
// bake moves smoothly at 120 Hz. Joints can be hidden with everything below
// them (a severed arm, a lost head): their vertices fold into the joint.
//
// The engine evaluates poses on the GPU (engine.cpp, the pose pass) with the
// same math as evaluate() here, which the native tests check against the
// bake, and which a world uses on the CPU to attach things to a hand.
#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "../vec3.h"

namespace anim {

constexpr int kMaxLayers = 4;
constexpr int kMaxJoints = 96;  // hidden-joint bits in a Pose

enum Mask : uint8_t { kFullBody, kUpperBody, kLowerBody, kMaskCount };

struct Quat {
  float x = 0, y = 0, z = 0, w = 1;
};
Quat mul(Quat a, Quat b);
Quat conj(Quat q);
Vec3 rotate(Quat q, Vec3 v);
// Normalized linear blend, taking the short way round.
Quat nlerp(Quat a, Quat b, float t);

// A rigid transform: v' = q v + t.
struct Rigid {
  Quat q;
  Vec3 t{0, 0, 0};
  bool hidden = false;  // folded into the point t
};
Vec3 apply(const Rigid& r, Vec3 v);

struct Layer {
  int clip = -1;
  // The clip's frame at time 0, in frames (fractional): the layer shows frame
  // t * fps + phase, wrapped round when it loops, held at the ends when not.
  float phase = 0;
  float weight = 1;
  Mask mask = kFullBody;
  bool loop = true;
  bool additive = false;
  float refFrame = 0;    // an additive layer's rest frame within its clip
  bool inPlace = false;  // drop the pelvis's travel across the floor
};

struct Pose {
  Layer layers[kMaxLayers];
  int count = 0;
  uint32_t hidden[3] = {0, 0, 0};  // a bit per joint: hidden with its children
  // Adds a layer; past kMaxLayers the last one is replaced.
  void add(const Layer& l);
  void hide(int joint);
  bool isHidden(int joint) const;
  // One looping clip, or a cross-fade into `clip` from `fromClip`, which
  // still shows `fromWeight` (0 to 1).
  static Pose clip(int clip, float phase, int fromClip = -1, float fromPhase = 0, float fromWeight = 0,
                   bool loop = true, bool fromLoop = true);
};

struct ClipInfo {
  std::string name;
  uint32_t firstFrame = 0, frameCount = 0;
};

// The baked clips and the skeleton.
struct Data {
  int joints = 0, bodies = 0;
  uint32_t framesPerBody = 0;
  float fps = 30;
  std::vector<ClipInfo> clips;
  // Per body, frame and joint, 8 halves: rotation xyzw, translation xyz, 0.
  std::vector<uint16_t> texels;
  std::vector<std::string> names;
  std::vector<int> parent;
  std::vector<Vec3> bind;    // per body * joints + joint: the bind position
  std::vector<uint8_t> moves;  // joints whose bind position travels (root, pelvis)
  std::vector<float> masks;  // per mask * joints + joint: its weight there
  int maxDepth = 0;          // joints from the deepest one up to a root

  int joint(const char* name) const;  // -1 when there is none
  int clipId(const char* name) const;
  // A baked skinning transform.
  Rigid skin(int body, uint32_t frame, int joint) const;
  float mask(Mask m, int joint) const { return masks[(size_t)m * joints + joint]; }
};

// Reads anims.bin and skeleton.bin. Returns false with a reason on a bad file.
bool load(const std::vector<char>& anims, const std::vector<char>& skeleton, Data& out, std::string* error);
float halfToFloat(uint16_t h);
uint16_t floatToHalf(float f);

// Where a layer is in its clip at time t: frames a and b and how far
// between them (b equals a at a held end).
void framesAt(const Layer& l, uint32_t frameCount, float t, float fps, uint32_t& a, uint32_t& b, float& f);

// The pose's skinning transform for one joint of one body at time t.
Rigid evaluate(const Data& d, int body, const Pose& pose, float t, int joint);
// The same for every joint (out has d.joints entries).
void evaluateAll(const Data& d, int body, const Pose& pose, float t, std::vector<Rigid>& out);
// Where a joint is in the character's own frame (feet at the origin, facing
// +z) in this pose.
Vec3 jointPosition(const Data& d, int body, const Pose& pose, float t, int joint);
// Root motion: how far a clip's pelvis has travelled across the floor (x, z)
// from its first frame by frame `frame`.
Vec3 travel(const Data& d, int body, int clip, float frame);

}  // namespace anim

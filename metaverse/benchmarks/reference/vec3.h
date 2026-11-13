// The engine's 3-vector and its few operations, kept free of GL so native
// code (the ECS, rules, src/physics/ and their unit tests) can use it without
// Emscripten. engine.h re-exports it.
#pragma once
#include <cmath>

struct Vec3 {
  float x, y, z;
};

inline Vec3 operator-(Vec3 a, Vec3 b) { return {a.x - b.x, a.y - b.y, a.z - b.z}; }
inline Vec3 operator+(Vec3 a, Vec3 b) { return {a.x + b.x, a.y + b.y, a.z + b.z}; }
inline Vec3 operator*(Vec3 a, float s) { return {a.x * s, a.y * s, a.z * s}; }
inline Vec3 cross(Vec3 a, Vec3 b) { return {a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x}; }
inline float dot(Vec3 a, Vec3 b) { return a.x * b.x + a.y * b.y + a.z * b.z; }
inline Vec3 normalize(Vec3 v) {
  float l = std::sqrt(dot(v, v));
  return {v.x / l, v.y / l, v.z / l};
}

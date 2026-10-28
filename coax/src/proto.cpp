// Host boundary for the Coax descriptor importer: files, owned buffers and
// reproducible fingerprints only. Parsing, validation and generation are Coax.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include <array>
#include <memory>
#include <string_view>
#ifndef COAX_BOOTSTRAP_IMPORTER
#include "proto_importer.h"
#endif
namespace coax::proto {
using View = std::string_view;
static constexpr size_t MAX_DESCRIPTOR = 8 * 1024 * 1024;
static constexpr size_t MAX_SOURCE = 8 * 1024 * 1024;
#include "proto_runtime.inc"
// SHA-256 is used only for reproducible input fingerprints, never admission or
// signature verification. All words are unsigned; padding is host-independent.
[[maybe_unused]] static std::string sha256(View input) {
  static constexpr uint32_t k[] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
  };
  std::array<uint32_t,8> h = {0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19};
  size_t size = (input.size() + 9 + 63) & ~(size_t)63;
  std::vector<uint8_t> padded(size, 0);
  if (!input.empty()) memcpy(padded.data(), input.data(), input.size());
  padded[input.size()] = 0x80;
  uint64_t bits = (uint64_t)input.size() * 8;
  for (size_t i = 0; i < 8; i++) padded[size - 1 - i] = (uint8_t)(bits >> (8 * i));
  auto rotate = [](uint32_t v, int n) { return (v >> n) | (v << (32 - n)); };
  for (size_t at = 0; at < size; at += 64) {
    uint32_t w[64];
    for (size_t i = 0; i < 16; i++) {
      size_t p = at + i * 4;
      w[i] = (uint32_t)padded[p] << 24 | (uint32_t)padded[p+1] << 16 | (uint32_t)padded[p+2] << 8 | padded[p+3];
    }
    for (size_t i = 16; i < 64; i++) {
      uint32_t x = w[i-15], y = w[i-2];
      w[i] = w[i-16] + (rotate(x,7)^rotate(x,18)^(x>>3)) + w[i-7] + (rotate(y,17)^rotate(y,19)^(y>>10));
    }
    uint32_t a=h[0],b=h[1],c=h[2],d=h[3],e=h[4],f=h[5],g=h[6],z=h[7];
    for (size_t i = 0; i < 64; i++) {
      uint32_t t1 = z + (rotate(e,6)^rotate(e,11)^rotate(e,25)) + ((e&f)^(~e&g)) + k[i] + w[i];
      uint32_t t2 = (rotate(a,2)^rotate(a,13)^rotate(a,22)) + ((a&b)^(a&c)^(b&c));
      z=g;g=f;f=e;e=d+t1;d=c;c=b;b=a;a=t1+t2;
    }
    h[0]+=a;h[1]+=b;h[2]+=c;h[3]+=d;h[4]+=e;h[5]+=f;h[6]+=g;h[7]+=z;
  }
  std::string hex;
  const char *digits = "0123456789abcdef";
  for (uint32_t word : h) for (int i = 7; i >= 0; i--) hex += digits[(word >> (4*i)) & 15];
  return hex;
}

[[maybe_unused]] static std::string runtime_program() {
  View core(PROTO_CORE_SOURCE);
  size_t end=core.find("// @proto-runtime-checks\n");
  if(end==View::npos) return {};
  return std::string(core.substr(0,end))+PROTO_MEMORY_SOURCE;
}

struct Result { std::string source, provenance, error; };
static Result import_descriptor(View raw,const std::string &prefix,const std::string &compiler) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)raw;(void)prefix;(void)compiler;
  return {{},{},"proto imports are disabled in the build-only stage0 compiler"};
#else
  Result result;
  try {
    const auto caps=D_capacities();
    std::vector<coax_t_D_Type> types((size_t)caps.types), enums((size_t)caps.types);
    std::vector<coax_t_D_Field> fields((size_t)caps.fields);
    std::vector<coax_t_D_Value> values((size_t)caps.values);
    std::vector<coax_string> groups((size_t)caps.groups), files((size_t)caps.files), deps((size_t)caps.deps), symbols((size_t)caps.symbols);
    // Allocate without clearing: Coax writes every byte before exposing it.
    std::unique_ptr<uint8_t[]> arena(new uint8_t[(size_t)caps.arena]), output(new uint8_t[(size_t)caps.output]);
    coax_t_D_State state{};
    state.types={types.data(),(int64_t)types.size()};state.enums={enums.data(),(int64_t)enums.size()};
    state.fields={fields.data(),(int64_t)fields.size()};state.values={values.data(),(int64_t)values.size()};
    state.groups={groups.data(),(int64_t)groups.size()};state.files={files.data(),(int64_t)files.size()};
    state.deps={deps.data(),(int64_t)deps.size()};state.symbols={symbols.data(),(int64_t)symbols.size()};
    state.arena={arena.get(),caps.arena};state.output={output.get(),caps.output};
    const std::string runtime=runtime_program();
    int64_t count=G_generate({&state,1},{raw.data(),(int64_t)raw.size()},
        {prefix.data(),(int64_t)prefix.size()},{runtime.data(),(int64_t)runtime.size()});
    if(state.error.count) {result.error.assign(state.error.data,(size_t)state.error.count);return result;}
    std::string compiler_id=compiler;
    for(char &c:compiler_id) if(c==' ') c='-';
    result.provenance="// coax-proto-v3 generator=coax0-coax-v1 compiler="+compiler_id+
        " importer_sha256="+sha256(PROTO_IMPORTER_SOURCE)+" descriptor_sha256="+sha256(raw)+" runtime_sha256="+sha256(runtime)+" prefix="+prefix;
    if(count<0 || (uint64_t)count>MAX_SOURCE-result.provenance.size()-1) {
      result.error="generated source budget";result.provenance.clear();return result;
    }
    result.source=result.provenance+"\n";
    result.source.append((const char*)output.get(),(size_t)count);
  } catch(const std::bad_alloc &) {result.source.clear();result.provenance.clear();result.error="descriptor allocation failed";}
  return result;
#endif
}
static Result import_file(const std::string &path,const std::string &prefix,const std::string &compiler) {
  FILE *file=fopen(path.c_str(),"rb");
  if (!file) return {{},{},"cannot read proto descriptor: "+path};
  std::string raw;
  char buffer[65536];
  size_t count;
  while ((count=fread(buffer,1,sizeof buffer,file))!=0) {
    if (count>MAX_DESCRIPTOR-raw.size()) {
      fclose(file);
      return {{},{},"descriptor byte budget exceeded"};
    }
    raw.append(buffer,count);
  }
  bool ok=!ferror(file);
  fclose(file);
  if (!ok) return {{},{},"cannot read proto descriptor: "+path};
  return import_descriptor(raw,prefix,compiler);
}
} // namespace coax::proto

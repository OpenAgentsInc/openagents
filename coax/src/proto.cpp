// Native FileDescriptorSet importer. No protobuf library, subprocess, network,
// interpreter, or installed runtime files. The generated program remains Coax.
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include <array>
#include <functional>
#include <limits>
#include <map>
#include <set>
#include <stdexcept>
#include <string_view>
#include <unordered_map>
#include <unordered_set>

namespace coax::proto {
using View = std::string_view;
static constexpr size_t MAX_DESCRIPTOR = 8 * 1024 * 1024;
static constexpr size_t MAX_SOURCE = 8 * 1024 * 1024;
#include "proto_runtime.inc"

struct Error : std::runtime_error { using std::runtime_error::runtime_error; };
static void require(bool condition, const std::string &message) {
  if (!condition) throw Error(message);
}
static bool alpha(char c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c == '_'; }
static bool digit(char c) { return c >= '0' && c <= '9'; }
static bool ident_char(char c) { return alpha(c) || digit(c); }
static bool identifier(View s) {
  if (s.empty() || s.size() > 256 || !alpha(s[0])) return false;
  for (char c : s) if (!ident_char(c)) return false;
  return true;
}
static bool utf8(View bytes) {
  size_t at = 0;
  while (at < bytes.size()) {
    uint8_t a = (uint8_t)bytes[at++];
    if (a < 128) continue;
    size_t needed = 0;
    uint8_t low = 128, high = 191;
    if (a >= 194 && a <= 223) needed = 1;
    else if (a >= 224 && a <= 239) {
      needed = 2;
      if (a == 224) low = 160;
      if (a == 237) high = 159;
    } else if (a >= 240 && a <= 244) {
      needed = 3;
      if (a == 240) low = 144;
      if (a == 244) high = 143;
    } else return false;
    if (needed > bytes.size() - at || (uint8_t)bytes[at] < low || (uint8_t)bytes[at] > high) return false;
    at++;
    for (size_t i = 1; i < needed; i++, at++)
      if ((uint8_t)bytes[at] < 128 || (uint8_t)bytes[at] > 191) return false;
  }
  return true;
}

// SHA-256 is used only for reproducible input fingerprints, never admission or
// signature verification. All words are unsigned; padding is host-independent.
static std::string sha256(View input) {
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

struct WireField {
  uint32_t number = 0;
  uint8_t wire = 0;
  uint64_t integer = 0;
  View bytes;
};
using Fields = std::vector<WireField>;

struct Reader {
  size_t visited = 0;
  static uint64_t varint(View input, size_t &at) {
    uint64_t value = 0;
    for (int shift = 0; shift <= 63; shift += 7) {
      require(at < input.size(), "truncated descriptor varint");
      uint8_t b = (uint8_t)input[at++];
      require(shift != 63 || b <= 1, "descriptor varint overflow");
      value |= (uint64_t)(b & 127) << shift;
      if (b < 128) return value;
    }
    throw Error("descriptor varint overflow");
  }
  Fields fields(View input) {
    Fields result;
    size_t at = 0;
    while (at < input.size()) {
      require(result.size() < 40000 && ++visited <= 400000, "descriptor field/work budget");
      uint64_t key = varint(input, at), number = key >> 3;
      require(number && number <= 536870911, "invalid descriptor field number");
      WireField f;
      f.number = (uint32_t)number;
      f.wire = (uint8_t)(key & 7);
      if (f.wire == 0) f.integer = varint(input, at);
      else {
        uint64_t count = f.wire == 1 ? 8 : f.wire == 5 ? 4 : 0;
        if (f.wire == 2) count = varint(input, at);
        else require(f.wire == 1 || f.wire == 5, "descriptor groups are unsupported");
        require(count <= input.size() - at, "truncated descriptor field length");
        f.bytes = input.substr(at, (size_t)count);
        at += (size_t)count;
      }
      result.push_back(f);
    }
    return result;
  }
};
static const WireField *one(const Fields &fields, uint32_t number) {
  const WireField *found = nullptr;
  for (const WireField &f : fields) if (f.number == number) {
    require(!found, "duplicate descriptor metadata field");
    found = &f;
  }
  return found;
}
static uint64_t integer(const Fields &fields, uint32_t number, uint64_t fallback = 0) {
  const WireField *f = one(fields, number);
  if (!f) return fallback;
  require(f->wire == 0, "descriptor integer wire type");
  return f->integer;
}
static View bytes(const WireField &field) {
  require(field.wire == 2, "descriptor bytes wire type");
  return field.bytes;
}
static View bytes(const Fields &fields, uint32_t number) {
  const WireField *f = one(fields, number);
  return f ? bytes(*f) : View{};
}
static std::string text(View value) {
  require(value.size() <= 16384 && utf8(value), "invalid/oversized descriptor UTF-8 string");
  return std::string(value);
}
static std::string ident(const Fields &fields, uint32_t number) {
  View value = bytes(fields, number);
  require(identifier(value), "invalid/oversized descriptor identifier");
  return std::string(value);
}
static bool has(const Fields &fields, uint32_t number) {
  for (const WireField &f : fields) if (f.number == number) return true;
  return false;
}

struct Options {
  std::map<uint32_t,WireField> values;
  uint64_t n(uint32_t tag) const {
    auto it = values.find(tag);
    if (it == values.end()) return 0;
    require(it->second.wire == 0, "invalid numeric policy option");
    return it->second.integer;
  }
  View str(uint32_t tag) const {
    auto it = values.find(tag);
    return it == values.end() ? View{} : bytes(it->second);
  }
  bool has(uint32_t tag) const { return values.count(tag) != 0; }
};
static Options options(Reader &reader, const Fields &fields, uint32_t tag, bool message) {
  Options result;
  const WireField *raw = one(fields, tag);
  if (!raw) return result;
  for (const WireField &f : reader.fields(bytes(*raw))) {
    bool allowed = message ? (f.number==51000 || f.number==51001 || f.number==51006)
                           : (f.number==51002 || f.number==51003 || f.number==51004 || f.number==51005 || f.number==51007 || f.number==51008);
    require(f.number < 999 || allowed,
            "unsupported policy option " + std::to_string(f.number));
    require(!result.has(f.number), "duplicate descriptor option");
    if (f.number==51000 || f.number==51001 || f.number==51002 || f.number==51003 || f.number==51004 || f.number==51008) {
      require(f.wire == 0 && f.integer <= INT64_MAX, "invalid numeric policy option");
      if (f.number == 51004) require(f.integer <= 1, "invalid required_value option");
    }
    if (f.number==51005 || f.number==51006 || f.number==51007) (void)text(bytes(f));
    result.values.emplace(f.number, f);
  }
  return result;
}

struct Field {
  std::string name, type_name;
  uint32_t number = 0;
  int label = 0, kind = 0, oneof = -1;
  bool optional = false;
  Options opts;
};
struct Message {
  std::string full, name;
  std::vector<Field> fields;
  std::vector<std::string> groups;
  Options opts;
};
struct Enum {
  std::string full, name;
  std::vector<std::pair<std::string,int32_t>> values;
};
struct TypeRef { bool message; size_t index; };

struct Descriptor {
  Reader reader;
  std::vector<Message> messages;
  std::vector<Enum> enums;
  std::unordered_map<std::string,TypeRef> types;
  size_t enum_values = 0, field_count = 0;

  void add_enum(View data, const std::string &parent) {
    Fields p = reader.fields(data);
    Enum e;
    e.full = parent + "." + ident(p, 1);
    std::set<std::string> names;
    for (const WireField &f : p) if (f.number == 2) {
      Fields q = reader.fields(bytes(f));
      require(one(q,2), "missing enum number");
      uint64_t raw = integer(q,2);
      require(raw <= INT32_MAX || raw >= UINT64_MAX - (uint64_t)INT32_MAX, "enum value outside int32");
      int64_t value = (int64_t)(raw & UINT64_C(0xffffffff));
      if (value > INT32_MAX) value -= INT64_C(1) << 32;
      std::string name = ident(q,1);
      require(names.insert(name).second, "duplicate enum value name");
      e.values.emplace_back(name,(int32_t)value);
      require(++enum_values <= 8192, "descriptor enum budget exceeded");
    }
    require(!e.values.empty() && e.values[0].second == 0, "proto3 enum must start at zero");
    require(enums.size() < 256, "descriptor enum budget exceeded");
    enums.push_back(std::move(e));
  }

  void add_message(View data, const std::string &parent, int depth = 0) {
    require(depth < 32, "descriptor nesting budget");
    Fields p = reader.fields(data);
    Message m;
    m.full = parent + "." + ident(p,1);
    m.opts = options(reader,p,7,true);
    require(!m.opts.n(7), "maps are not supported: " + m.full);
    require(!has(p,5) && !has(p,6), "message extensions are not supported: " + m.full);
    for (const WireField &f : p) if (f.number == 8) {
      m.groups.push_back(ident(reader.fields(bytes(f)),1));
      require(m.groups.size() <= 256, "descriptor oneof budget");
    }
    std::set<uint64_t> numbers;
    std::set<std::string> names;
    for (const WireField &raw : p) if (raw.number == 2) {
      Fields q = reader.fields(bytes(raw));
      Field f;
      f.name = ident(q,1);
      uint64_t number = integer(q,3), label = integer(q,4), kind = integer(q,5);
      require(number >= 1 && number <= 536870911 && !(number >= 19000 && number <= 19999) &&
              numbers.insert(number).second && names.insert(f.name).second, "invalid/duplicate field in " + m.full);
      require((label == 1 || label == 3) && kind >= 1 && kind <= 18 && kind != 10 && !has(q,2) && !has(q,7),
              "unsupported field shape: " + m.full + "." + f.name);
      f.number = (uint32_t)number;
      f.label = (int)label;
      f.kind = (int)kind;
      f.type_name = text(bytes(q,6));
      f.opts = options(reader,q,8,false);
      require(!f.opts.has(51007) || f.opts.str(51007)=="nonempty-no-nul", "unsupported grammar option");
      if (one(q,9)) {
        uint64_t group = integer(q,9);
        require(group < m.groups.size() && f.label != 3, "invalid oneof");
        f.oneof = (int)group;
      }
      uint64_t optional = integer(q,17);
      require(optional <= 1, "invalid proto3 optional flag");
      f.optional = optional != 0;
      require(!f.optional || f.oneof >= 0, "proto3 optional lacks synthetic oneof");
      m.fields.push_back(std::move(f));
      require(m.fields.size() <= 256 && ++field_count <= 2048, "descriptor field budget exceeded");
    }
    require(messages.size() < 256, "descriptor message budget exceeded");
    std::string full = m.full;
    messages.push_back(std::move(m));
    for (const WireField &f : p) {
      if (f.number == 3) add_message(bytes(f),full,depth+1);
      if (f.number == 4) add_enum(bytes(f),full);
    }
  }

  const std::string &type_name(const TypeRef &ref) const { return ref.message ? messages[ref.index].name : enums[ref.index].name; }
  void parse(View raw, const std::string &prefix) {
    require(raw.size() <= MAX_DESCRIPTOR, "descriptor byte budget exceeded");
    require(identifier(prefix), "invalid/oversized import prefix");
    std::set<std::string> file_names;
    std::vector<std::string> dependencies;
    for (const WireField &file : reader.fields(raw)) if (file.number == 1) {
      Fields p = reader.fields(bytes(file));
      std::string name = text(bytes(p,1)), package = text(bytes(p,2));
      require(!name.empty() && name.find('\0') == std::string::npos && name.size() <= 4096, "invalid descriptor file name");
      require(file_names.insert(name).second, "duplicate descriptor file");
      require(file_names.size() <= 256, "descriptor file budget exceeded");
      for (const WireField &f : p) if (f.number == 3) dependencies.push_back(text(bytes(f)));
      if (name=="google/protobuf/descriptor.proto" || name=="openagents/options/v1alpha1/options.proto") continue;
      require(bytes(p,12)=="proto3" && !has(p,7) && !has(p,14), "only proto3 data descriptors are supported: " + name);
      require(package.size() <= 4096, "descriptor package budget");
      if (!package.empty()) {
        size_t at = 0, parts = 0;
        while (at <= package.size()) {
          size_t end = package.find('.',at);
          require(identifier(View(package).substr(at,end==std::string::npos ? end : end-at)) && ++parts <= 32, "invalid/oversized package");
          if (end==std::string::npos) break;
          at=end+1;
        }
      }
      std::string parent = package.empty() ? "" : "." + package;
      for (const WireField &f : p) {
        if (f.number == 4) add_message(bytes(f),parent);
        if (f.number == 5) add_enum(bytes(f),parent);
      }
    }
    for (const std::string &dep : dependencies) require(file_names.count(dep), "missing descriptor file dependency: " + dep);
    require(!messages.empty(), "no runtime messages in descriptor");
    std::map<std::string,size_t> counts;
    auto leaf = [](const std::string &full) { return full.substr(full.rfind('.')+1); };
    for (const Message &m : messages) counts[leaf(m.full)]++;
    for (const Enum &e : enums) counts[leaf(e.full)]++;
    std::set<std::string> names;
    auto bind = [&](auto &values, bool message) {
      for (size_t i=0;i<values.size();i++) {
        auto &v=values[i];
        std::string short_name=leaf(v.full), alias=counts[short_name]==1 ? short_name : v.full.substr(1);
        for (char &c : alias) if (c=='.') c='_';
        v.name=prefix+"_"+alias;
        require(names.insert(v.name).second, "generated name collision; use narrower profiles");
        require(types.emplace(v.full,TypeRef{message,i}).second, "duplicate fully-qualified type");
      }
    };
    bind(messages,true);bind(enums,false);
    for (const Message &m : messages) for (const Field &f : m.fields) {
      if (f.kind==11 || f.kind==14) {
        auto it=types.find(f.type_name);
        require(it!=types.end(), "missing descriptor dependency: "+f.type_name);
        require(it->second.message==(f.kind==11), "descriptor type mismatch");
      } else require(f.type_name.empty(), "scalar has type_name");
      require(!f.opts.n(51002) || f.kind==9 || f.kind==12, "max_bytes requires bytes/string");
      require(!f.opts.n(51008) || f.kind==9 || f.kind==12, "exact_bytes requires bytes/string");
      require(!f.opts.n(51003) || f.label==3, "max_count requires repeated field");
      require(!f.opts.has(51007) || f.kind==9, "grammar requires string");
      if (f.optional) {
        size_t count=0;
        for (const Field &x : m.fields) if (x.oneof==f.oneof) count++;
        require(count==1, "invalid synthetic oneof");
      }
    }
  }
};

struct Scalar { const char *type, *option; int wire; };
static Scalar scalar(const Field &f) {
  switch (f.kind) {
    case 1: return {"f64","F64",1};
    case 2: return {"f32","F32",5};
    case 3: return {"s64","S64",0};
    case 4: return {"u64","U64",0};
    case 5: return {"s32","S32",0};
    case 6: return {"u64","U64",1};
    case 7: return {"u32","U32",5};
    case 8: return {"bool","Bool",0};
    case 9: return {"string","String",2};
    case 12: return {"[] u8","Bytes",2};
    case 13: return {"u32","U32",0};
    case 14: return {"s32","S32",0};
    case 15: return {"s32","S32",5};
    case 16: return {"s64","S64",1};
    case 17: return {"s32","S32",0};
    case 18: return {"s64","S64",0};
    default: throw Error("unimplemented descriptor scalar");
  }
}
static std::string join(const std::vector<std::string> &values, View separator) {
  std::string out;
  for (const std::string &v : values) {
    if (!out.empty()) out += separator;
    out += v;
  }
  return out;
}
// The checked source is embedded verbatim at compiler build time. Checks are
// compiler tests, not part of every imported application's authority boundary.
static std::string runtime_program() {
  View core(PROTO_CORE_SOURCE);
  size_t end=core.find("// @proto-runtime-checks\n");
  require(end!=View::npos, "missing wire runtime check boundary");
  return std::string(core.substr(0,end))+PROTO_MEMORY_SOURCE;
}
static std::string runtime_source(const std::string &prefix) {
  const std::string program=runtime_program();
  View input(program);
  std::string out;
  size_t at = 0;
  while (at < input.size()) {
    size_t start = at;
    if (input.substr(at,2)=="//") {
      at = input.find('\n',at);
      if (at==View::npos) at=input.size();
    } else if (input.substr(at,2)=="/*") {
      int depth=1;at+=2;
      while (at<input.size() && depth) {
        if (input.substr(at,2)=="/*") {depth++;at+=2;}
        else if (input.substr(at,2)=="*/") {depth--;at+=2;}
        else at++;
      }
    } else if (input[at]=='"' || input[at]=='\'') {
      char quote=input[at++];
      while (at<input.size()) {
        if (input[at]=='\\') at=std::min(at+2,input.size());
        else if (input[at++]==quote) break;
      }
    } else if (alpha(input[at])) {
      while (at<input.size() && ident_char(input[at])) at++;
      View token=input.substr(start,at-start);
      if (token.substr(0,3)=="PB_") {
        out+=prefix;
        out+=token.substr(3);
        continue;
      }
    } else at++;
    out+=input.substr(start,at-start);
  }
  return out;
}

struct Generator {
  const Descriptor &descriptor;
  std::string p, output;
  using Failure = std::function<std::string(const std::string &)>;
  Generator(const Descriptor &d,const std::string &prefix) : descriptor(d),p(prefix+"__pb_") {}
  void emit(const std::string &s) {
    require(s.size() < MAX_SOURCE && output.size() + s.size() + 1 <= MAX_SOURCE, "generated source budget");
    output+=s;output+='\n';
  }
  std::string typ(const Field &f) const {
    return f.kind==11 ? descriptor.type_name(descriptor.types.at(f.type_name)) : scalar(f).type;
  }
  std::string option(const Field &f) const { return f.kind==11 ? typ(f)+"_Option" : p+scalar(f).option; }
  int wt(const Field &f) const { return f.kind==11 ? 2 : scalar(f).wire; }
  std::string value(const Field &f,const std::string &raw="f") const {
    std::string r=raw+".raw",data=raw+".payload";
    switch (f.kind) {
      case 11: return typ(f)+".{wire="+data+"}";
      case 9: return "cast(string) "+data;
      case 12: return data;
      case 3: case 5: case 14: case 15: case 16: return "cast,trunc("+typ(f)+") "+r;
      case 4: case 6: return r;
      case 7: case 13: return "cast(u32) "+r;
      case 8: return r+"!=0";
      case 17: case 18: return "cast("+typ(f)+") "+p+"sint("+r+")";
      case 1: return "f64_from_bits("+r+")";
      case 2: return "f32_from_bits(cast(u32) "+r+")";
      default: throw Error("unimplemented descriptor scalar");
    }
  }
  void constraints(const Field &f,const Failure &fail) {
    emit("if f.wt!="+std::to_string(wt(f))+" { "+fail(p+"WIRE")+" }");
    if (f.kind==7 || f.kind==13 || f.kind==17) emit("if f.raw>4294967295 { "+fail(p+"VALUE")+" }");
    if (f.kind==8) emit("if canonical && f.raw>1 { "+fail(p+"VALUE")+" }");
    if (f.kind==9) emit("if !"+p+"utf8(f.payload) { "+fail(p+"UTF8")+" }");
    if (f.kind==9 || f.kind==12) {
      for (auto [tag,op] : {std::pair<uint32_t,const char *>{51002,">"},{51008,"!="}})
        if (f.opts.n(tag)) emit("if f.payload.count"+std::string(op)+std::to_string(f.opts.n(tag))+" { "+fail(p+"LIMIT")+" }");
      if (f.opts.has(51007)) emit("if !"+p+"no_nul(f.payload) { "+fail(p+"VALUE")+" }");
    }
    if ((f.kind==1 || f.kind==2) && f.opts.str(51005)=="rad")
      emit("x:="+value(f)+";if x!=x || x < -3.141592653589793 || x>3.141592653589793 { "+fail(p+"VALUE")+" }");
  }
  static std::set<int> real_groups(const Message &m) {
    std::set<int> out;
    for (const Field &f : m.fields) if (f.oneof>=0 && !f.optional) out.insert(f.oneof);
    return out;
  }
  void check(const Message &m) {
    const std::string &n=m.name;
    Failure fail=[&](const std::string &error) {return "return "+p+"Check.{error="+error+"};";};
    emit(n+"__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->"+p+"Check {");
    emit("if depth>"+p+"MAX_DEPTH || wire.count<0 || wire.count>"+p+"MAX_BYTES {return "+p+"Check.{error="+p+"LIMIT};}");
    if (m.opts.n(51001)) emit("if wire.count>"+std::to_string(m.opts.n(51001))+" {return "+p+"Check.{error="+p+"LIMIT};}");
    emit("left:=budget;pos:=0;");
    for (const Field &f : m.fields) emit("count_"+f.name+":int=0;");
    std::set<int> groups=real_groups(m);
    for (int group : groups) emit("one_"+std::to_string(group)+":u32=0;");
    emit("while pos<wire.count {");
    emit("if left<=0 {return "+p+"Check.{error="+p+"LIMIT};} left-=1;");
    emit("f:="+p+"scan(wire,pos);if f.error!=0 {return "+p+"Check.{error=f.error};} if canonical && !"+p+"minimal(wire,f) {return "+p+"Check.{error="+p+"WIRE};} pos=f.next;");
    for (size_t index=0;index<m.fields.size();index++) {
      const Field &f=m.fields[index];
      std::string number=std::to_string(f.number),wire=std::to_string(wt(f)),group=std::to_string(f.oneof);
      emit(std::string(index==0 ? "if" : "else if")+" f.number=="+number+" {");
      if (f.label!=3) emit("if count_"+f.name+">0 {"+fail(p+"DUPLICATE")+"}");
      if (groups.count(f.oneof) && !f.optional)
        emit("if one_"+group+"!=0 {"+fail(p+"DUPLICATE")+"} one_"+group+"="+number+";");
      if (f.label==3 && wt(f)!=2) {
        emit("if f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {");
        emit("if left<=0 {"+fail(p+"LIMIT")+"} left-=1; f="+p+"scalar(packed,at,"+wire+");if f.error!=0 {"+fail("f.error")+"}");
        if (wt(f)==0) emit("if canonical && "+p+"vsize(f.raw)!=f.next-at {"+fail(p+"WIRE")+"}");
        emit("at=f.next;");
        emit("f.wt="+wire+";");
        constraints(f,fail);emit("count_"+f.name+"+=1;");
        emit("}} else {");constraints(f,fail);emit("count_"+f.name+"+=1;");emit("}");
      } else {
        constraints(f,fail);emit("count_"+f.name+"+=1;");
        if (f.kind==11)
          emit("child:="+typ(f)+"__check(f.payload,depth+1,left,true,canonical);if child.error!=0 {"+fail("child.error")+"} left=child.left;");
      }
      if (f.opts.n(51003)) emit("if count_"+f.name+">"+std::to_string(f.opts.n(51003))+" {"+fail(p+"LIMIT")+"}");
      emit("}");
    }
    emit("}");
    for (const Field &f : m.fields) if (f.opts.n(51004)) emit("if required && count_"+f.name+"==0 {"+fail(p+"REQUIRED")+"}");
    emit("return "+p+"Check.{left=left};}");
  }
  void getter(const Message &m,const Field &f) {
    const std::string &n=m.name;
    std::string o=option(f);
    emit(n+"_get_"+f.name+" :: (msg:"+n+")->"+o+" {");
    emit("checked:="+n+"_decode(msg.wire);if msg.error!=0 || checked.error!=0 {return "+o+".{error=msg.error|checked.error};}");
    emit("pos:=0;while pos<msg.wire.count {f:="+p+"scan(msg.wire,pos);pos=f.next;if f.number=="+std::to_string(f.number)+" {return "+o+".{value="+value(f)+",present=true};}}");
    emit("return "+o+".{};}");
  }
  void iterator(const Message &m,const Field &f) {
    const std::string &n=m.name;
    std::string iter=n+"_Iter_"+f.name,t=typ(f),wire=std::to_string(wt(f));
    emit(iter+" :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:"+t+";has_value:bool;done:bool;error:u32;}");
    emit(n+"_iter_"+f.name+" :: (msg:"+n+")->"+iter+" {checked:="+n+"_decode(msg.wire);if msg.error!=0 || checked.error!=0 {return "+iter+".{error=msg.error|checked.error,done=true};}return "+n+"_next_"+f.name+"("+iter+".{wire=msg.wire});}");
    emit(n+"_next_"+f.name+" :: (old:"+iter+")->"+iter+" {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {");
    if (wt(f)!=2)
      emit("if iterator.packed_at<iterator.packed.count {f:="+p+"scalar(iterator.packed,iterator.packed_at,"+wire+");if f.error!=0 {iterator.error=f.error;iterator.done=true;return iterator;}iterator.packed_at=f.next;iterator.value="+value(f)+";iterator.has_value=true;return iterator;}");
    emit("f:="+p+"scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!="+std::to_string(f.number)+" {continue;}");
    if (wt(f)!=2) emit("if f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}");
    emit("iterator.value="+value(f)+";iterator.has_value=true;return iterator;}return iterator;}");
  }
  void writer(const Message &m,const Field &f) {
    const std::string &n=m.name;
    std::string t=typ(f),wire=std::to_string(wt(f)),number=std::to_string(f.number);
    std::vector<std::string> siblings;
    if (f.oneof>=0 && !f.optional) {
      for (const Field &x : m.fields) if (x.oneof==f.oneof) siblings.push_back(std::to_string(x.number));
    } else siblings.push_back(number);
    for (bool clear : {false,true}) {
      std::string method=clear ? "clear" : f.label==3 ? "append" : "set";
      std::string params="msg:"+n+","+(clear ? "" : "value:"+t+",")+"out:[] u8";
      emit(n+"_"+method+"_"+f.name+" :: ("+params+")->"+n+" {");
      emit("if msg.error!=0 {return "+n+".{error=msg.error};} checked:="+n+"__check(msg.wire,0,"+p+"MAX_FIELDS,false,false);if checked.error!=0 {return "+n+".{error=checked.error};}");
      std::vector<std::string> targets=clear ? std::vector<std::string>{number} : f.label==3 ? std::vector<std::string>{} : siblings;
      if (targets.empty()) emit("removed_fields:[] u32;");
      else emit("removed_fields:= ["+std::to_string(targets.size())+"] u32.["+join(targets,",")+"];");
      std::string removed=targets.empty() ? "removed_fields" : "removed_fields[..]";
      emit("data:[] u8;txt:=\"\";raw:u64=0;is_text:=false;");
      if (!clear) {
        if (f.kind==11) emit("child:="+t+"_decode(value.wire);if value.error!=0 || child.error!=0 {return "+n+".{error=value.error|child.error};} data=value.wire;");
        else if (f.kind==9) emit("txt=value;is_text=true;");
        else if (f.kind==12) emit("data=value;");
        else if (f.kind==1 || f.kind==2) emit("raw=cast(u64) "+std::string(f.kind==1 ? "f64_bits" : "f32_bits")+"(value);");
        else if (f.kind==17 || f.kind==18) emit("raw="+p+"zigzag(cast(s64)value);");
        else if (f.kind==8) emit("if value {raw=1;}");
        else if (f.kind==3 || f.kind==5 || f.kind==14 || f.kind==15 || f.kind==16) emit("raw=cast,trunc(u64) value;");
        else emit("raw=cast(u64) value;");
      }
      emit("result:="+p+"rewrite(msg.wire,"+removed+","+number+","+wire+",raw,data,txt,is_text,"+(clear ? "true" : "false")+",out);");
      emit("if result.error!=0 {return "+n+".{error=result.error};} final:="+n+"__check(result.wire,0,"+p+"MAX_FIELDS,false,false);if final.error!=0 {return "+n+".{error=final.error};}return "+n+".{wire=result.wire};}");
    }
  }
  void api(const Message &m) {
    const std::string &n=m.name;
    emit(n+"_new :: ()->"+n+" {return "+n+".{};}");
    for (bool minimal : {false,true}) {
      std::string method=minimal ? "_decode_minimal" : "_decode";
      emit(n+method+" :: (wire:[] u8)->"+n+" {c:="+n+"__check(wire,0,"+p+"MAX_FIELDS,true,"+(minimal ? "true" : "false")+");if c.error!=0 {return "+n+".{error=c.error};}return "+n+".{wire=wire};}");
    }
    emit(n+"_encode :: (msg:"+n+",out:[] u8)->"+p+"View {if msg.error!=0 {return "+p+"View.{error=msg.error};} c:="+n+"__check(msg.wire,0,"+p+"MAX_FIELDS,true,false);if c.error!=0 {return "+p+"View.{error=c.error};} return "+p+"copy(msg.wire,out);}");
    for (int group : real_groups(m)) {
      std::vector<std::string> targets;
      for (const Field &f : m.fields) if (f.oneof==group) targets.push_back("f.number=="+std::to_string(f.number));
      emit(n+"_case_"+m.groups[(size_t)group]+" :: (msg:"+n+")->"+p+"U32 {checked:="+n+"_decode(msg.wire);if msg.error!=0 || checked.error!=0 {return "+p+"U32.{error=msg.error|checked.error};}pos:=0;while pos<msg.wire.count {f:="+p+"scan(msg.wire,pos);pos=f.next;if "+join(targets," || ")+" {return "+p+"U32.{value=f.number,present=true};}} return "+p+"U32.{};}");
    }
    std::vector<std::string> known;
    for (const Field &f : m.fields) known.push_back("f.number=="+std::to_string(f.number));
    std::string tests=known.empty() ? "false" : join(known," || ");
    emit(n+"_unknown :: (msg:"+n+",cursor:int)->"+p+"Field {checked:="+n+"_decode(msg.wire);if msg.error!=0 || checked.error!=0 {return "+p+"Field.{error=msg.error|checked.error};}pos:=cursor;while true {f:="+p+"scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !("+tests+") {return f;}}return "+p+"Field.{done=true};}");
    for (const Field &f : m.fields) {
      emit(n+"_FIELD_"+f.name+" :: "+std::to_string(f.number)+";");
      if (f.label==3) iterator(m,f); else getter(m,f);
      writer(m,f);
    }
  }
  std::string generate() {
    emit(runtime_source(p));
    emit("// @proto-descriptor-declarations");
    for (const Enum &e : descriptor.enums) {
      std::vector<std::string> known;
      for (const auto &[name,value] : e.values) {
        emit(e.name+"_"+name+" :: "+std::to_string(value)+";");
        known.push_back("value=="+std::to_string(value));
      }
      emit(e.name+"_known :: (value:s32)->bool {return "+join(known," || ")+";}");
    }
    for (const Message &m : descriptor.messages) {
      emit("// "+m.full+" -> "+m.name);
      emit(m.name+" :: struct {wire:[] u8;error:u32;}");
      emit(m.name+"_Option :: struct {value:"+m.name+";present:bool;error:u32;}");
    }
    for (const Message &m : descriptor.messages) {check(m);api(m);}
    std::set<std::string> declared;
    size_t at=0;
    while (at<output.size()) {
      size_t end=at;
      if (alpha(output[at])) {
        while (end<output.size() && ident_char(output[end])) end++;
        if (output.compare(end,3," ::")==0)
          require(declared.insert(output.substr(at,end-at)).second, "generated symbol collision; select a narrower descriptor profile");
      }
      at=output.find('\n',end);
      if (at==std::string::npos) break;
      at++;
    }
    return std::move(output);
  }
};

struct Result {
  std::string source, provenance, error;
};
static Result import_descriptor(View raw,const std::string &prefix,const std::string &compiler) {
  Result result;
  try {
    Descriptor descriptor;
    descriptor.parse(raw,prefix);
    std::string compiler_id=compiler;
    for (char &c : compiler_id) if (c==' ') c='-';
    result.provenance="// coax-proto-v2 generator=coax0-native-v1 compiler="+compiler_id+
        " descriptor_sha256="+sha256(raw)+" runtime_sha256="+sha256(runtime_program())+" prefix="+prefix;
    result.source=Generator(descriptor,prefix).generate();
    require(result.source.size()+result.provenance.size()+1<=MAX_SOURCE,"generated source budget");
    result.source=result.provenance+"\n"+result.source;
  } catch (const Error &e) {
    result.source.clear();result.provenance.clear();result.error=e.what();
  } catch (const std::bad_alloc &) {
    result.source.clear();result.provenance.clear();result.error="descriptor allocation failed";
  }
  return result;
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

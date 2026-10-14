// dna_dump: a MetaHuman DNA file's meshes as OBJ, optionally posed by
// RigLogic, for Draconia's private import into Verse
// (docs/verse/draconia/metahuman-study.md).
//
//   dna_dump HEAD.dna OUT_DIR [--lod N] [--controls CONTROLS.txt] [--list-controls]
//
// Writes OUT_DIR/<mesh>.obj for every mesh in the level of detail (default
// 0), with positions (centimetres, as the DNA stores them), texture
// coordinates and normals, and OUT_DIR/joints.txt (name, parent, neutral
// translation and rotation). CONTROLS.txt holds lines `raw_control value`
// (the DNA's raw control names, such as CTRL_expressions_mouthCornerPullL);
// when given, RigLogic evaluates them and the meshes are written posed:
// blend shapes applied, then joint deltas by linear blend skinning.
//
// Links Epic's OpenRigLogic (MIT): build it, then
//   c++ -std=c++17 -O2 dna_dump.cpp -I$ORL/include -L$ORL/build -lriglogic413_5_0 -o dna_dump
// (scripts/unreal/build_dna_dump.sh). The DNA files are MetaHuman assets:
// private, never committed, never used to train AI models.
#include <riglogic/RigLogic.h>

#include <cmath>
#include <cstdio>
#include <fstream>
#include <map>
#include <sstream>
#include <string>
#include <vector>

struct M4 {
    double m[16];
};
static M4 identity() {
    M4 r{};
    r.m[0] = r.m[5] = r.m[10] = r.m[15] = 1;
    return r;
}
static M4 mul(const M4& a, const M4& b) {
    M4 r{};
    for (int i = 0; i < 4; ++i)
        for (int j = 0; j < 4; ++j) {
            double s = 0;
            for (int k = 0; k < 4; ++k) s += a.m[i * 4 + k] * b.m[k * 4 + j];
            r.m[i * 4 + j] = s;
        }
    return r;
}
// Row-major, column vectors: p' = M p. Euler XYZ in degrees (rotate about
// X, then Y, then Z), as RigLogic's joint attributes.
static M4 trs(double tx, double ty, double tz, double rx, double ry, double rz, double sx = 1, double sy = 1, double sz = 1) {
    const double d = M_PI / 180.0;
    double cx = cos(rx * d), sx_ = sin(rx * d), cy = cos(ry * d), sy_ = sin(ry * d), cz = cos(rz * d), sz_ = sin(rz * d);
    // R = Rz * Ry * Rx
    double r[9] = {cy * cz, cz * sx_ * sy_ - cx * sz_, cx * cz * sy_ + sx_ * sz_,
                   cy * sz_, cx * cz + sx_ * sy_ * sz_, -cz * sx_ + cx * sy_ * sz_,
                   -sy_, cy * sx_, cx * cy};
    M4 m = identity();
    for (int i = 0; i < 3; ++i) {
        m.m[i * 4 + 0] = r[i * 3 + 0] * sx;
        m.m[i * 4 + 1] = r[i * 3 + 1] * sy;
        m.m[i * 4 + 2] = r[i * 3 + 2] * sz;
    }
    m.m[3] = tx;
    m.m[7] = ty;
    m.m[11] = tz;
    return m;
}
static M4 inverse_rigid(const M4& a) {
    // General 4x4 inverse (Gauss-Jordan); joint matrices may carry scale.
    double m[4][8];
    for (int i = 0; i < 4; ++i)
        for (int j = 0; j < 8; ++j) m[i][j] = j < 4 ? a.m[i * 4 + j] : (j - 4 == i ? 1 : 0);
    for (int c = 0; c < 4; ++c) {
        int p = c;
        for (int r = c + 1; r < 4; ++r)
            if (fabs(m[r][c]) > fabs(m[p][c])) p = r;
        for (int j = 0; j < 8; ++j) std::swap(m[c][j], m[p][j]);
        double v = m[c][c];
        for (int j = 0; j < 8; ++j) m[c][j] /= v;
        for (int r = 0; r < 4; ++r)
            if (r != c) {
                double f = m[r][c];
                for (int j = 0; j < 8; ++j) m[r][j] -= f * m[c][j];
            }
    }
    M4 r{};
    for (int i = 0; i < 4; ++i)
        for (int j = 0; j < 4; ++j) r.m[i * 4 + j] = m[i][j + 4];
    return r;
}

int main(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: dna_dump HEAD.dna OUT_DIR [--lod N] [--controls FILE]\n");
        return 2;
    }
    std::string path = argv[1], out = argv[2], controls;
    int lod = 0;
    for (int i = 3; i < argc; ++i) {
        std::string a = argv[i];
        if (a == "--lod" && i + 1 < argc) lod = atoi(argv[++i]);
        else if (a == "--controls" && i + 1 < argc) controls = argv[++i];
    }
    auto stream = rl4::makeScoped<rl4::FileStream>(path.c_str(), rl4::FileStream::AccessMode::Read, rl4::FileStream::OpenMode::Binary);
    auto reader = rl4::makeScoped<rl4::BinaryStreamReader>(stream.get());
    reader->read();
    if (!rl4::Status::isOk()) {
        fprintf(stderr, "dna: %s\n", rl4::Status::get().message);
        return 1;
    }
    const auto* r = reader.get();
    for (int i = 3; i < argc; ++i)
        if (std::string(argv[i]) == "--list-controls") {
            for (std::uint16_t c = 0; c < r->getRawControlCount(); ++c) printf("%s\n", r->getRawControlName(c).c_str());
            return 0;
        }
    const std::uint16_t joints = r->getJointCount();
    // Neutral joint transforms.
    auto tx = r->getNeutralJointTranslationXs(), ty = r->getNeutralJointTranslationYs(), tz = r->getNeutralJointTranslationZs();
    auto rx = r->getNeutralJointRotationXs(), ry = r->getNeutralJointRotationYs(), rz = r->getNeutralJointRotationZs();
    std::vector<M4> neutral(joints), posed(joints);
    std::vector<int> parent(joints);
    for (std::uint16_t j = 0; j < joints; ++j) parent[j] = r->getJointParentIndex(j) == j ? -1 : r->getJointParentIndex(j);
    {
        std::ofstream f(out + "/joints.txt");
        for (std::uint16_t j = 0; j < joints; ++j)
            f << r->getJointName(j).c_str() << ' ' << parent[j] << ' ' << tx[j] << ' ' << ty[j] << ' ' << tz[j] << ' ' << rx[j] << ' ' << ry[j] << ' ' << rz[j] << '\n';
    }
    std::vector<float> blend(r->getBlendShapeChannelCount(), 0.0f);
    std::vector<float> jointOut;
    bool pose = !controls.empty();
    if (pose) {
        auto rig = rl4::makeScoped<rl4::RigLogic>(reader.get());
        auto inst = rl4::makeScoped<rl4::RigInstance>(rig.get());
        inst->setLOD(static_cast<std::uint16_t>(lod));
        std::map<std::string, std::uint16_t> raw;
        for (std::uint16_t i = 0; i < r->getRawControlCount(); ++i) raw[r->getRawControlName(i).c_str()] = i;
        std::ifstream f(controls);
        std::string line;
        while (std::getline(f, line)) {
            std::istringstream s(line);
            std::string name;
            float v;
            if (line.empty() || line[0] == '#' || !(s >> name >> v)) continue;
            auto it = raw.find(name);
            if (it == raw.end()) {
                fprintf(stderr, "unknown raw control %s\n", name.c_str());
                continue;
            }
            inst->setRawControl(it->second, v);
        }
        rig->calculate(inst.get());
        auto bs = inst->getBlendShapeOutputs();
        // Blend shape outputs are indexed by the LOD's channel list.
        auto lodChannels = r->getBlendShapeChannelIndicesForLOD(static_cast<std::uint16_t>(lod));
        for (std::size_t i = 0; i < bs.size() && i < lodChannels.size(); ++i) blend[lodChannels[i]] = bs[i];
        auto jo = inst->getJointOutputs();
        jointOut.assign(jo.begin(), jo.end());
        fprintf(stderr, "riglogic: %zu blend shape outputs, %zu joint values\n", bs.size(), jo.size());
    }
    // Joint world matrices, neutral and posed. RigLogic's joint outputs are
    // deltas from the neutral pose, 9 values per joint: translation (cm),
    // Euler rotation (degrees) and scale (0 = unchanged), as the Maya and
    // Unreal rigs add them to the joint's neutral attributes.
    const std::size_t stride = jointOut.empty() ? 9 : jointOut.size() / joints;
    for (std::uint16_t j = 0; j < joints; ++j) {
        M4 l = trs(tx[j], ty[j], tz[j], rx[j], ry[j], rz[j]);
        M4 lp = l;
        if (!jointOut.empty() && stride >= 9) {
            const float* o = &jointOut[j * stride];
            lp = trs(tx[j] + o[0], ty[j] + o[1], tz[j] + o[2], rx[j] + o[3], ry[j] + o[4], rz[j] + o[5], 1 + o[6], 1 + o[7], 1 + o[8]);
        }
        neutral[j] = parent[j] < 0 ? l : mul(neutral[parent[j]], l);
        posed[j] = parent[j] < 0 ? lp : mul(posed[parent[j]], lp);
    }
    std::vector<M4> skin(joints);
    for (std::uint16_t j = 0; j < joints; ++j) skin[j] = mul(posed[j], inverse_rigid(neutral[j]));

    auto meshes = r->getMeshIndicesForLOD(static_cast<std::uint16_t>(lod));
    for (auto m : meshes) {
        std::string name = r->getMeshName(m).c_str();
        auto xs = r->getVertexPositionXs(m), ys = r->getVertexPositionYs(m), zs = r->getVertexPositionZs(m);
        std::vector<double> p(xs.size() * 3);
        for (std::size_t i = 0; i < xs.size(); ++i) {
            p[i * 3] = xs[i];
            p[i * 3 + 1] = ys[i];
            p[i * 3 + 2] = zs[i];
        }
        if (pose) {
            for (std::uint16_t t = 0; t < r->getBlendShapeTargetCount(m); ++t) {
                float w = blend[r->getBlendShapeChannelIndex(m, t)];
                if (w == 0.0f) continue;
                auto vi = r->getBlendShapeTargetVertexIndices(m, t);
                auto dx = r->getBlendShapeTargetDeltaXs(m, t), dy = r->getBlendShapeTargetDeltaYs(m, t), dz = r->getBlendShapeTargetDeltaZs(m, t);
                for (std::size_t k = 0; k < vi.size(); ++k) {
                    p[vi[k] * 3] += w * dx[k];
                    p[vi[k] * 3 + 1] += w * dy[k];
                    p[vi[k] * 3 + 2] += w * dz[k];
                }
            }
            if (!jointOut.empty() && r->getSkinWeightsCount(m) == xs.size()) {
                for (std::size_t i = 0; i < xs.size(); ++i) {
                    auto ws = r->getSkinWeightsValues(m, static_cast<std::uint32_t>(i));
                    auto js = r->getSkinWeightsJointIndices(m, static_cast<std::uint32_t>(i));
                    double q[3] = {0, 0, 0}, v[3] = {p[i * 3], p[i * 3 + 1], p[i * 3 + 2]};
                    double total = 0;
                    for (std::size_t k = 0; k < ws.size(); ++k) {
                        const M4& s = skin[js[k]];
                        for (int a = 0; a < 3; ++a) q[a] += ws[k] * (s.m[a * 4] * v[0] + s.m[a * 4 + 1] * v[1] + s.m[a * 4 + 2] * v[2] + s.m[a * 4 + 3]);
                        total += ws[k];
                    }
                    if (total > 0)
                        for (int a = 0; a < 3; ++a) p[i * 3 + a] = q[a] / total;
                }
            }
        }
        std::ofstream f(out + "/" + name + ".obj");
        f << "o " << name << '\n';
        for (std::size_t i = 0; i < xs.size(); ++i) f << "v " << p[i * 3] << ' ' << p[i * 3 + 1] << ' ' << p[i * 3 + 2] << '\n';
        auto us = r->getVertexTextureCoordinateUs(m), vs = r->getVertexTextureCoordinateVs(m);
        for (std::size_t i = 0; i < us.size(); ++i) f << "vt " << us[i] << ' ' << vs[i] << '\n';
        auto nx = r->getVertexNormalXs(m), ny = r->getVertexNormalYs(m), nz = r->getVertexNormalZs(m);
        for (std::size_t i = 0; i < nx.size(); ++i) f << "vn " << nx[i] << ' ' << ny[i] << ' ' << nz[i] << '\n';
        auto lp = r->getVertexLayoutPositionIndices(m), lt = r->getVertexLayoutTextureCoordinateIndices(m), ln = r->getVertexLayoutNormalIndices(m);
        for (std::uint32_t fi = 0; fi < r->getFaceCount(m); ++fi) {
            auto ls = r->getFaceVertexLayoutIndices(m, fi);
            f << 'f';
            for (auto l : ls) f << ' ' << lp[l] + 1 << '/' << lt[l] + 1 << '/' << ln[l] + 1;
            f << '\n';
        }
        fprintf(stderr, "mesh %s: %zu vertices, %u faces\n", name.c_str(), xs.size(), r->getFaceCount(m));
    }
    return 0;
}

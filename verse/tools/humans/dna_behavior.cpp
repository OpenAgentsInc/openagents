// dna_behavior: a MetaHuman DNA file's rig behaviour (the tables RigLogic
// evaluates) as text, the input for MetaHuman Lab's face rig (planned: a Coax
// port of OpenRigLogic).
//
//   dna_behavior HEAD.dna OUT.txt [--lod N]
//
// Writes, one table per line block, each preceded by `name count`:
//   raw NAME...           raw control names
//   gui ...               GUI to raw conditional table rows (in out from to slope cut)
//   psd rows cols values  pose-space corrective controls (product of inputs)
//   jgroup lod...         each joint group: its LOD row counts, inputs, outputs,
//                         values (row major, outputs x inputs) and joints
//   bsc in out            blend shape channels: input control, output channel
//   bslod n...            blend shape channel count per LOD
//   rep j t r             joint translation and rotation representations
//   joint NAME parent tx ty tz rx ry rz   the neutral skeleton (cm, degrees)
//
// Links OpenRigLogic (MIT):
//   c++ -std=c++17 -O2 dna_behavior.cpp -I$ORL/include -L$ORL/build -lriglogic413_5_0 -o dna_behavior
// The DNA files are MetaHuman assets: private, never committed, never used to
// train AI models, and so is this tool's output.
#include <riglogic/RigLogic.h>

#include <cstdio>
#include <string>

template <class A>
static void ints(FILE* f, const char* name, A a) {
    fprintf(f, "%s %zu", name, a.size());
    for (auto v : a) fprintf(f, " %d", static_cast<int>(v));
    fprintf(f, "\n");
}
template <class A>
static void floats(FILE* f, const char* name, A a) {
    fprintf(f, "%s %zu", name, a.size());
    for (auto v : a) fprintf(f, " %.7g", static_cast<double>(v));
    fprintf(f, "\n");
}

int main(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: dna_behavior HEAD.dna OUT.txt\n");
        return 2;
    }
    auto stream = rl4::makeScoped<rl4::FileStream>(argv[1], rl4::FileStream::AccessMode::Read, rl4::FileStream::OpenMode::Binary);
    auto reader = rl4::makeScoped<rl4::BinaryStreamReader>(stream.get());
    reader->read();
    if (!rl4::Status::isOk()) {
        fprintf(stderr, "dna: %s\n", rl4::Status::get().message);
        return 1;
    }
    const auto* r = reader.get();
    FILE* f = fopen(argv[2], "w");
    if (!f) return 1;
    fprintf(f, "raw %d", r->getRawControlCount());
    for (std::uint16_t c = 0; c < r->getRawControlCount(); ++c) fprintf(f, " %s", r->getRawControlName(c).c_str());
    fprintf(f, "\n");
    fprintf(f, "psdcount %d\n", r->getPSDCount());
    ints(f, "psdrows", r->getPSDRowIndices());
    ints(f, "psdcols", r->getPSDColumnIndices());
    floats(f, "psdvalues", r->getPSDValues());
    ints(f, "guiin", r->getGUIToRawInputIndices());
    ints(f, "guiout", r->getGUIToRawOutputIndices());
    fprintf(f, "jointrows %d jointcols %d\n", r->getJointRowCount(), r->getJointColumnCount());
    fprintf(f, "jgroups %d\n", r->getJointGroupCount());
    for (std::uint16_t g = 0; g < r->getJointGroupCount(); ++g) {
        ints(f, "lods", r->getJointGroupLODs(g));
        ints(f, "in", r->getJointGroupInputIndices(g));
        ints(f, "out", r->getJointGroupOutputIndices(g));
        floats(f, "values", r->getJointGroupValues(g));
        ints(f, "joints", r->getJointGroupJointIndices(g));
    }
    ints(f, "bsclods", r->getBlendShapeChannelLODs());
    ints(f, "bscin", r->getBlendShapeChannelInputIndices());
    ints(f, "bscout", r->getBlendShapeChannelOutputIndices());
    fprintf(f, "bschannels %d", r->getBlendShapeChannelCount());
    for (std::uint16_t c = 0; c < r->getBlendShapeChannelCount(); ++c) fprintf(f, " %s", r->getBlendShapeChannelName(c).c_str());
    fprintf(f, "\n");
    const std::uint16_t joints = r->getJointCount();
    auto tx = r->getNeutralJointTranslationXs(), ty = r->getNeutralJointTranslationYs(), tz = r->getNeutralJointTranslationZs();
    auto rx = r->getNeutralJointRotationXs(), ry = r->getNeutralJointRotationYs(), rz = r->getNeutralJointRotationZs();
    fprintf(f, "joints %d\n", joints);
    for (std::uint16_t j = 0; j < joints; ++j) {
        const int p = r->getJointParentIndex(j) == j ? -1 : r->getJointParentIndex(j);
        fprintf(f, "joint %s %d %.7g %.7g %.7g %.7g %.7g %.7g %d %d\n", r->getJointName(j).c_str(), p, tx[j], ty[j], tz[j], rx[j], ry[j], rz[j],
                static_cast<int>(r->getJointTranslationRepresentation(j)), static_cast<int>(r->getJointRotationRepresentation(j)));
    }
    fprintf(f, "meshes %d\n", r->getMeshCount());
    for (std::uint16_t m = 0; m < r->getMeshCount(); ++m) {
        std::size_t deltas = 0;
        for (std::uint16_t t = 0; t < r->getBlendShapeTargetCount(m); ++t) deltas += r->getBlendShapeTargetVertexIndices(m, t).size();
        fprintf(f, "mesh %s vertices %d targets %d deltas %zu\n", r->getMeshName(m).c_str(), r->getVertexPositionCount(m), r->getBlendShapeTargetCount(m), deltas);
    }
    fclose(f);
    return 0;
}

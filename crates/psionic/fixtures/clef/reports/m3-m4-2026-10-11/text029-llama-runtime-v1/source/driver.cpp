// Diagnostic-only exact-token Clef trunk capture; no generation or GPU backend.
#include "llama.h"
#include "llama-ext.h"
#include "ggml.h"
#include "ggml-cpu.h"
#include "ggml-backend.h"
#include <cerrno>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <map>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>
#include <fcntl.h>
#include <unistd.h>

constexpr int N = 359, H = 5120, L = 64;
struct Capture {
    std::filesystem::path out;
    std::set<std::string> written;
    std::set<const ggml_tensor *> observed;
    std::map<std::string, int> operations;
    std::string error;
};
static void require(bool ok, const std::string & error) {
    if (!ok) throw std::runtime_error(error);
}
static bool capture_name(const std::string & name) {
    if (name == "result_norm") return true;
    for (int i = 0; i < L; ++i) if (name == "l_out-" + std::to_string(i)) return true;
    return false;
}
static void write_new(const std::filesystem::path & path, const void * data, size_t bytes) {
    int fd = open(path.c_str(), O_WRONLY | O_CREAT | O_EXCL, 0600);
    require(fd >= 0, "exclusive output open failed");
    const char * next = static_cast<const char *>(data);
    while (bytes) {
        ssize_t count = write(fd, next, bytes);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) { close(fd); throw std::runtime_error("output write failed"); }
        next += count; bytes -= static_cast<size_t>(count);
    }
    require(close(fd) == 0, "output close failed");
}
static bool callback(ggml_tensor * tensor, bool ask, void * user) {
    auto & c = *static_cast<Capture *>(user);
    // C callbacks must not unwind through upstream frames.
    try {
        if (!c.error.empty()) return false;
        std::string name = ggml_get_name(tensor);
        if (ask) {
            if (c.observed.insert(tensor).second) {
                std::string key = ggml_op_name(tensor->op);
                if (tensor->op == GGML_OP_MUL_MAT && tensor->src[0]) {
                    key += ":lhs="; key += ggml_type_name(tensor->src[0]->type);
                }
                ++c.operations[key];
            }
            return capture_name(name);
        }
        require(capture_name(name), "unexpected callback tensor");
        require(!c.written.count(name), "duplicate layer capture");
        require(tensor->type == GGML_TYPE_F32 && ggml_is_contiguous(tensor), "capture dtype/layout differs");
        require(tensor->ne[0] == H && tensor->ne[1] == N && tensor->ne[2] == 1 && tensor->ne[3] == 1,
                "capture dimensions differ");
        require(ggml_nbytes(tensor) == size_t(N) * H * sizeof(float), "capture byte count differs");
        std::vector<float> rows(size_t(N) * H);
        ggml_backend_tensor_get(tensor, rows.data(), 0, rows.size() * sizeof(float));
        for (float value : rows) require(std::isfinite(value), "nonfinite captured row");
        write_new(c.out / (name + ".f32"), rows.data(), rows.size() * sizeof(float));
        c.written.insert(name);
        std::fprintf(stderr, "captured %s %zu bytes\n", name.c_str(), rows.size() * sizeof(float));
        return true;
    } catch (const std::exception & error) { c.error = error.what(); return false; }
}
static bool abort_on_error(void * user) { return !static_cast<Capture *>(user)->error.empty(); }

int main(int argc, char ** argv) {
    llama_model * model = nullptr;
    llama_context * context = nullptr;
    llama_batch_ext * batch = nullptr;
    // Both callback pointers remain valid until context cleanup, including errors.
    Capture capture;
    bool initialized = false;
    int code = 1;
    try {
        require(argc == 4, "expected MODEL INPUT_TSV EMPTY_OUTPUT_DIRECTORY");
        require(std::getenv("CLEF_LLAMA_GUARDED") && std::string(std::getenv("CLEF_LLAMA_GUARDED")) == "1",
                "owned CPU guard required");
        capture.out = argv[3];
        require(std::filesystem::is_directory(capture.out) && std::filesystem::is_empty(capture.out), "output must be empty");
        std::ifstream in(argv[2]); require(in.good(), "input file missing");
        std::vector<int> tokens, orders;
        for (int i = 0; i < N; ++i) {
            int position, token, order;
            require(bool(in >> position >> token >> order), "input row missing");
            require(position == i && token >= 0 && token < 248320 && order >= 0 && order <= 4, "invalid exact input row");
            tokens.push_back(token); orders.push_back(order);
        }
        std::string extra; require(!(in >> extra), "extra input row");
        int questions = 0, options = 0;
        for (int i = 0; i < N; ++i) {
            if (orders[i] && (i == 0 || orders[i] != orders[i - 1])) {
                require(i == 0 || orders[i - 1] == 0, "span separation differs");
                if (orders[i] == 4) { require(questions > 0, "option before question"); ++options; }
                else ++questions;
            }
        }
        require(questions == 3 && options == 6, "expected three questions and six options");
        llama_backend_init(); initialized = true;
        // Static CPU-only registration; never search for loadable device plugins.
        require(ggml_backend_dev_count() == 1 && ggml_backend_reg_count() == 1, "unexpected backend count");
        require(ggml_backend_dev_type(ggml_backend_dev_get(0)) == GGML_BACKEND_DEVICE_TYPE_CPU, "non-CPU device");
        const auto * traits = ggml_get_type_traits_cpu(GGML_TYPE_Q4_K);
        require(traits && traits->vec_dot_type == GGML_TYPE_Q8_K, "Q4_K RHS type differs");
        auto mp = llama_model_default_params();
        ggml_backend_dev_t no_devices[] = {nullptr};
        mp.devices = no_devices; mp.n_gpu_layers = 0; mp.split_mode = LLAMA_SPLIT_MODE_NONE;
        mp.load_mode = LLAMA_LOAD_MODE_MMAP; mp.lazy_mode = LLAMA_LAZY_MODE_OFF;
        mp.use_extra_bufts = false; mp.no_host = false; mp.load_mtp = false;
        model = llama_model_load_from_file(argv[1], mp); require(model, "model load failed");
        require(llama_model_n_embd(model) == H && llama_model_n_layer(model) == L &&
                llama_vocab_n_tokens(llama_model_get_vocab(model)) == 248320, "model shape differs");
        auto cp = llama_context_default_params();
        cp.n_ctx = 512; cp.n_batch = 512; cp.n_ubatch = 512; cp.n_seq_max = 1;
        cp.n_threads = 2; cp.n_threads_batch = 2; cp.embeddings = true;
        cp.pooling_type = LLAMA_POOLING_TYPE_NONE; cp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_DISABLED;
        cp.offload_kqv = false; cp.op_offload = false;
        cp.cb_eval = callback; cp.cb_eval_user_data = &capture;
        cp.abort_callback = abort_on_error; cp.abort_callback_data = &capture;
        context = llama_init_from_model(model, cp); require(context, "context creation failed");
        require(llama_get_memory(context) == nullptr, "unexpected persistent cache");
        require(llama_n_threads(context) == 2 && llama_n_threads_batch(context) == 2, "thread count differs");
        batch = llama_batch_ext_init(context); require(batch, "batch creation failed");
        for (int i = 0; i < N; ++i) {
            int index = llama_batch_ext_add_token(batch, 0, tokens[i]); require(index == i, "batch index differs");
            llama_pos pos = i;
            require(llama_batch_ext_set_pos(batch, i, &pos) && llama_batch_ext_set_output_embd(batch, i, true) &&
                    llama_batch_ext_set_decision_order(batch, i, static_cast<llama_decision_order>(orders[i])), "batch setter failed");
        }
        int rc = llama_process(context, LLAMA_PROCESS_TYPE_ENCODE, batch);
        require(capture.error.empty(), capture.error); require(rc == 0, "encode failed");
        require(capture.written.size() == 65, "incomplete layer captures");
        for (int i = 0; i < L; ++i) require(capture.written.count("l_out-" + std::to_string(i)), "layer missing");
        require(capture.written.count("result_norm"), "final rows missing");
        require(capture.operations["GATED_DELTA_NET"] == 48, "expected 48 fused CPU Delta operators");
        require(capture.operations["FLASH_ATTN_EXT"] == 0, "unexpected Flash Attention operator");
        // The graph also computes a quantized embedded head. Keep it explicitly separate.
        std::string summary = "{\"status\":\"complete\",\"tokens\":359,\"hidden\":5120,\"layers\":64,\"captures\":65,"
            "\"threads\":2,\"gpu_executed\":false,\"cache\":false,\"Q4_K_rhs\":\"Q8_K\","
            "\"effective_delta\":\"fused_cpu_sequential_48_layers\",\"flash_attention\":false,"
            "\"embedded_head_excluded_from_comparison\":true,\"embedded_head_logits\":[";
        for (int i = 0; i < 6; ++i) {
            float * row = llama_get_embeddings_ith(context, i); require(row && std::isfinite(row[0]), "embedded head output invalid");
            char text[64]; std::snprintf(text, sizeof(text), "%s%.9g", i ? "," : "", double(row[0])); summary += text;
        }
        summary += "],\"observed_graph_operations\":{";
        bool first = true;
        for (const auto & item : capture.operations) {
            summary += (first ? "" : ",") + std::string("\"") + item.first + "\":" + std::to_string(item.second); first = false;
        }
        summary += "}}\n";
        write_new(capture.out / "driver.json", summary.data(), summary.size());
        code = 0;
    } catch (const std::exception & error) { std::fprintf(stderr, "diagnostic failed: %s\n", error.what()); }
    if (batch) llama_batch_ext_free(batch);
    if (context) llama_free(context);
    if (model) llama_model_free(model);
    if (initialized) llama_backend_free();
    return code;
}

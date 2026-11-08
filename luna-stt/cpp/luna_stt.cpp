#include "luna_stt.h"
#include "whisper.h"
#include "ggml-backend.h"   // <-- NOWE: nagłówek z API backendu
#include <cstring>
#include <string>
#include <vector>

struct LunaSTTContext {
    struct whisper_context* ctx;
};

LunaSTTContext* luna_stt_init(const char* model_path) {
    // --- NOWE: Rejestrujemy backend CPU (tylko raz na proces) ---
    static bool backend_registered = false;
    if (!backend_registered) {
        ggml_backend_register(ggml_backend_cpu_reg());
        backend_registered = true;
    }
    // -------------------------------------------------------------

    auto* luna_ctx = new LunaSTTContext();

    auto cparams = whisper_context_default_params();
    luna_ctx->ctx = whisper_init_from_file_with_params(model_path, cparams);

    if (!luna_ctx->ctx) {
        delete luna_ctx;
        return nullptr;
    }
    return luna_ctx;
}

char* luna_stt_transcribe(LunaSTTContext* ctx, const float* audio_data, int length) {
    if (!ctx || !ctx->ctx) return nullptr;

    whisper_full_params params = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
    params.print_realtime = false;
    params.print_progress = false;
    params.language = "pl";

    if (whisper_full(ctx->ctx, params, audio_data, length) != 0) {
        return nullptr;
    }

    std::string result;
    int n_segments = whisper_full_n_segments(ctx->ctx);
    for (int i = 0; i < n_segments; ++i) {
        result += whisper_full_get_segment_text(ctx->ctx, i);
    }

    char* out_str = (char*)malloc(result.length() + 1);
    strcpy(out_str, result.c_str());
    return out_str;
}

void luna_stt_free_string(char* str) {
    if (str) free(str);
}

void luna_stt_free(LunaSTTContext* ctx) {
    if (ctx) {
        if (ctx->ctx) whisper_free(ctx->ctx);
        delete ctx;
    }
}

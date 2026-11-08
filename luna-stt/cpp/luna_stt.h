#ifndef LUNA_STT_H
#define LUNA_STT_H

#ifdef __cplusplus
extern "C" {
#endif

// Opaque pointer do kontekstu whispera
typedef struct LunaSTTContext LunaSTTContext;

// Inicjalizacja
LunaSTTContext* luna_stt_init(const char* model_path);
// Transkrypcja (przyjmuje bufor float PCM 16kHz)
char* luna_stt_transcribe(LunaSTTContext* ctx, const float* audio_data, int length);
// Zwolnienie pamięci zaalokowanej w C++ (KLUCZOWE dla FFI)
void luna_stt_free_string(char* str);
// Sprzątanie
void luna_stt_free(LunaSTTContext* ctx);

#ifdef __cplusplus
}
#endif

#endif

// luna-stt/cpp/aneforge_stub.cpp
// Zaślepki dla funkcji Apple Neural Engine (ANE) na platformach innych niż Apple.
// Niektóre wersje whisper.cpp odwołują się do nich bezwarunkowo, co powoduje błędy linkowania.

#include <cstddef>

extern "C" {
    void * whisper_aneforge_init(...) { return nullptr; }
    void whisper_aneforge_free(...) {}
    void whisper_aneforge_encode(...) {}
}

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int};
use std::ptr::NonNull;

// Definiujemy surowe deklaracje z C++
#[repr(C)]
struct LunaSTTContextOpaque {
    _private: [u8; 0],
}

extern "C" {
    fn luna_stt_init(model_path: *const c_char) -> *mut LunaSTTContextOpaque;
    fn luna_stt_transcribe(
        ctx: *mut LunaSTTContextOpaque,
        audio_data: *const c_float,
        length: c_int,
    ) -> *mut c_char;
    fn luna_stt_free_string(str: *mut c_char);
    fn luna_stt_free(ctx: *mut LunaSTTContextOpaque);
}

// Bezpieczny wrapper
pub struct SttEngine {
    // NonNull gwarantuje, że wskaźnik nie jest nullem i mamy własność
    ctx: NonNull<LunaSTTContextOpaque>,
}

// Rust wymaga, aby typy współdzielone między wątkami były Send/Sync.
// Whisper nie jest w pełni thread-safe na jednym kontekście, więc Send, ale nie Sync.
unsafe impl Send for SttEngine {}

impl SttEngine {
    pub fn new(model_path: &str) -> Result<Self, String> {
        let c_path = CString::new(model_path).map_err(|e| e.to_string())?;
        let ctx_ptr = unsafe { luna_stt_init(c_path.as_ptr()) };
        
        let ctx = NonNull::new(ctx_ptr).ok_or_else(|| "Nie udało się załadować modelu STT".to_string())?;
        Ok(Self { ctx })
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String, String> {
        let ptr = unsafe {
            luna_stt_transcribe(
                self.ctx.as_ptr(),
                audio_data.as_ptr(),
                audio_data.len() as c_int,
            )
        };

        if ptr.is_null() {
            return Err("Transkrypcja nie powiodła się".to_string());
        }

        // Konwertujemy z powrotem na bezpiecznego Stringa w Rust
        let c_str = unsafe { CStr::from_ptr(ptr) };
        let result = c_str.to_string_lossy().into_owned();

        // Sprzątamy pamięć zaalokowaną w C++
        unsafe { luna_stt_free_string(ptr) };

        Ok(result)
    }
}

// Implementujemy Drop, aby zapobiec wyciekom pamięci
impl Drop for SttEngine {
    fn drop(&mut self) {
        unsafe { luna_stt_free(self.ctx.as_ptr()) };
    }
}

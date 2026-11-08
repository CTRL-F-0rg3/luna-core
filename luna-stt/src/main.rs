use luna_audio::{AudioCapture, AudioConfig};
use luna_plugins::PluginManager;
use luna_stt::SttEngine;
use std::sync::mpsc::{channel, sync_channel};
use std::thread;
use std::time::Duration;

fn main() {
    // 1. Wątek STT (własny model, własny wątek – nie blokuje nasłuchu)
    let (tx_audio, rx_audio) = sync_channel::<Vec<f32>>(8);
    let (tx_text, rx_text) = channel::<String>();

    thread::spawn(move || {
        let stt = match SttEngine::new("models/ggml-base.bin") {
            Ok(s) => s,
            Err(e) => { eprintln!("[STT] init: {e}"); return; }
        };
        eprintln!("[STT] model loaded, waiting for utterances...");
        while let Ok(samples) = rx_audio.recv() {
            eprintln!("[STT] transcribing {} samples...", samples.len());
            match stt.transcribe(&samples) {
                Ok(text) if !text.trim().is_empty() => {
                    let _ = tx_text.send(text.trim().to_string());
                }
                Ok(_) => {}
                Err(e) => eprintln!("[STT] {e}"),
            }
        }
    });

    // 2. Plugin manager
    let mut plugins = match PluginManager::new() {
        Ok(p) => p,
        Err(e) => { eprintln!("[plugins] {e}"); return; }
    };
    if let Err(e) = plugins.load_dir("plugins") {
        eprintln!("[plugins] load_dir: {e}");
    }
    println!("[main] loaded {} command(s)", plugins.commands.len());

    // 3. Mikrofon (z fallbackiem testowym)
    let tx_mic = tx_audio.clone();
    let _mic = match AudioCapture::start(AudioConfig::default(), move |samples| {
        let _ = tx_mic.try_send(samples);
    }) {
        Ok(m) => {
            println!("[Luna] Słucham... (Ctrl+C by zakończyć)");
            m
        }
        Err(e) => {
            eprintln!("[audio] {e}");
            eprintln!("[audio] sprawdź: arecord -l");
            eprintln!("[audio] fallback: test z buforem ciszy");
            let _ = tx_audio.send(vec![0.0; 16_000]);
            if let Ok(text) = rx_text.recv_timeout(Duration::from_secs(60)) {
                println!("[STT] {text}");
                match plugins.process(&text) {
                    Ok(Some(r)) => println!("[plugin] {r}"),
                    Ok(None) => println!("[plugin] (no handler)"),
                    Err(e) => eprintln!("[plugin] {e}"),
                }
            }
            return;
        }
    };

    // 4. Główna pętla: tekst → pluginy
    while let Ok(text) = rx_text.recv() {
        println!("[STT] {text}");
        match plugins.process(&text) {
            Ok(Some(r)) => println!("[plugin] {r}"),
            Ok(None) => println!("[plugin] (no handler)"),
            Err(e) => eprintln!("[plugin] {e}"),
        }
    }
}

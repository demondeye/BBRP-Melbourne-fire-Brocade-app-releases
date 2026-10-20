//! Voice control for the map.
//!
//! Two ways in (Settings → Voice):
//! - **Key only**: press the listen key (works while the game has focus) and say a postal.
//! - **Key or phrase**: as above, or say "Fire Com dispatch". The dispatcher answers
//!   "Go for Dispatch" and you say the postal.
//!
//! The map then flies to that postal, and the dispatcher reads it back with its
//! nearest road, plus the corner or intersection when the map shows one ("Copy,
//! postal five two four. The nearest road is Eclipse Boulevard. Intersection of
//! Eclipse Boulevard and Milton Road"). Nothing runs until a microphone is found.
//!
//! - Listening: the microphone picked in Settings (cpal), turned into words by the
//!   Vosk speech engine, limited to the wake phrase and numbers so it's accurate.
//!   It runs on this PC; the engine (~56 MB) is downloaded once, the first time.
//! - The listen key: polled with GetAsyncKeyState, so any key or mouse button works
//!   while another program (the game) has focus, and the key still reaches it.
//! - The dispatcher: natural-sounding recordings made with the Kokoro neural voices,
//!   built in (`assets/voice/*.bin`), with an optional police-scanner radio effect.

use crate::core::{lock, Shared, UiEvent};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq, Default, Debug)]
pub enum VoiceState {
    /// Voice control is off (or nobody is signed in).
    #[default]
    Off,
    /// No microphone found: voice control waits until one is.
    NoMic,
    /// Getting the speech engine (first time only): 0..1.
    Downloading(f32),
    /// Waiting for the key (and the phrase, if that's on).
    Waiting,
    /// Listening for a postal.
    Listening,
    /// The dispatcher is talking.
    Speaking,
    /// Something is stopping it.
    Problem(String),
}

#[derive(Clone, Default, Debug)]
pub struct VoiceStatus {
    pub state: VoiceState,
    /// The last thing heard when listening for a postal, and what it became.
    pub heard: Option<String>,
    /// Microphones on this PC (names), refreshed every few seconds.
    pub mics: Vec<String>,
    /// The microphone actually in use.
    pub mic_in_use: Option<String>,
}

// ── The listen key ────────────────────────────────────────────────────────

static KEY: AtomicU32 = AtomicU32::new(0x79);
static PRESSED: AtomicBool = AtomicBool::new(false);
static CAPTURE: AtomicBool = AtomicBool::new(false);
/// 0 = nothing yet, u32::MAX = cancelled (Esc), otherwise the key picked.
static CAPTURED: AtomicU32 = AtomicU32::new(0);
/// When the listen key was last pressed (ms since start), for the tick in Settings.
static LAST_PRESS_MS: AtomicU64 = AtomicU64::new(0);
/// Settings → Voice → Test microphone runs until this time (ms since start).
static TEST_UNTIL: AtomicU64 = AtomicU64::new(0);
/// Microphone loudness 0..1 (f32 bits), for the meter in Settings.
static LEVEL: AtomicU32 = AtomicU32::new(0);

fn now_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Settings: wait for the next key or mouse button and use it as the listen key.
pub fn start_capture() {
    CAPTURED.store(0, Ordering::SeqCst);
    CAPTURE.store(true, Ordering::SeqCst);
}
pub fn capturing() -> bool {
    CAPTURE.load(Ordering::SeqCst)
}
pub fn cancel_capture() {
    CAPTURE.store(false, Ordering::SeqCst);
}
/// The key picked after `start_capture` (None while still waiting or if cancelled).
pub fn take_captured() -> Option<u32> {
    match CAPTURED.swap(0, Ordering::SeqCst) {
        0 | u32::MAX => None,
        vk => Some(vk),
    }
}
/// The listen key was pressed in the last `ms` milliseconds.
pub fn key_pressed_recently(ms: u64) -> bool {
    let t = LAST_PRESS_MS.load(Ordering::SeqCst);
    t > 0 && now_ms().saturating_sub(t) < ms
}
/// Settings: turn the microphone test (level bar) on or off.
/// (The test stops by itself after 30 seconds.)
pub fn set_testing(on: bool) {
    TEST_UNTIL.store(if on { now_ms() + 30_000 } else { 0 }, Ordering::SeqCst);
}
pub fn testing() -> bool {
    now_ms() < TEST_UNTIL.load(Ordering::SeqCst)
}
/// How loud the microphone is right now, 0..1.
pub fn level() -> f32 {
    f32::from_bits(LEVEL.load(Ordering::Relaxed))
}

/// Watch the keyboard and mouse buttons (Windows): a quick poll of every key.
#[cfg(windows)]
fn start_keys() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    std::thread::spawn(|| {
        let mut was = [false; 256];
        loop {
            let capture = CAPTURE.load(Ordering::SeqCst);
            let key = KEY.load(Ordering::SeqCst) as usize;
            for vk in 1..255usize {
                // Skip the left/right mouse buttons and the generic Shift/Ctrl/Alt
                // (the left and right versions are reported on their own).
                if matches!(vk, 0x01 | 0x02 | 0x10 | 0x11 | 0x12) {
                    continue;
                }
                if !capture && vk != key {
                    continue;
                }
                let down = unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
                if down && !was[vk] {
                    if capture {
                        CAPTURED.store(if vk == 0x1B { u32::MAX } else { vk as u32 }, Ordering::SeqCst);
                        CAPTURE.store(false, Ordering::SeqCst);
                    } else if vk == key {
                        PRESSED.store(true, Ordering::SeqCst);
                        LAST_PRESS_MS.store(now_ms(), Ordering::SeqCst);
                    }
                }
                was[vk] = down;
            }
            std::thread::sleep(Duration::from_millis(12));
        }
    });
}
#[cfg(not(windows))]
fn start_keys() {}

/// A readable name for a key code ("F10", "Mouse button 4"…).
pub fn key_name(vk: u32) -> String {
    match vk {
        0x04 => return "Middle mouse button".into(),
        0x05 => return "Mouse button 4".into(),
        0x06 => return "Mouse button 5".into(),
        0x70..=0x87 => return format!("F{}", vk - 0x6F),
        _ => {}
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC};
        let sc = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
        // Keys that share a scan code with the number pad need the "extended" bit.
        let extended = matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x6F | 0xA3 | 0xA5 | 0x5B | 0x5C | 0x5D | 0x90);
        let lparam = ((sc as i32) << 16) | if extended { 1 << 24 } else { 0 };
        let mut buf = [0u16; 64];
        let n = unsafe { GetKeyNameTextW(lparam, buf.as_mut_ptr(), buf.len() as i32) };
        if n > 0 {
            return String::from_utf16_lossy(&buf[..n as usize]);
        }
    }
    format!("Key {vk}")
}

// ── The dispatcher's voice ───────────────────────────────────────────────

/// The voices: (id, name shown in Settings, female?).
pub const VOICES: [(&str, &str, bool); 10] = [
    ("af_heart", "Heart (US)", true),
    ("af_bella", "Bella (US)", true),
    ("af_nicole", "Nicole (US)", true),
    ("bf_emma", "Emma (UK)", true),
    ("bf_isabella", "Isabella (UK)", true),
    ("am_fenrir", "Fenrir (US)", false),
    ("am_michael", "Michael (US)", false),
    ("am_puck", "Puck (US)", false),
    ("bm_george", "George (UK)", false),
    ("bm_fable", "Fable (UK)", false),
];

/// A voice's recordings, packed: u32 count, then per clip u8 name length, name, u32 size, WAV bytes.
fn pack(voice: &str) -> &'static [u8] {
    macro_rules! packs {
        ($($v:literal),*) => {
            match voice {
                $($v => include_bytes!(concat!("../assets/voice/", $v, ".bin")) as &'static [u8],)*
                _ => include_bytes!("../assets/voice/af_heart.bin"),
            }
        };
    }
    packs!("af_heart", "af_bella", "af_nicole", "bf_emma", "bf_isabella", "am_fenrir", "am_michael", "am_puck", "bm_george", "bm_fable")
}

fn clip(voice: &str, name: &str) -> &'static [u8] {
    let b = pack(voice);
    let rd = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
    let mut at = 4;
    for _ in 0..rd(0) {
        let n = b[at] as usize;
        let clip_name = &b[at + 1..at + 1 + n];
        let size = rd(at + 1 + n);
        let data = &b[at + 5 + n..at + 5 + n + size];
        if clip_name == name.as_bytes() {
            return data;
        }
        at += 5 + n + size;
    }
    &[]
}

const RATE: u32 = 24_000;

/// A recording's samples (mono 24 kHz) as -1..1. Recordings are Ogg Vorbis
/// (most) or 16-bit WAV.
fn samples(voice: &str, name: &str) -> Vec<f32> {
    let b = clip(voice, name);
    if b.starts_with(b"OggS") {
        return match rodio::Decoder::new(std::io::Cursor::new(b)) {
            Ok(d) => d.map(|s: i16| s as f32 / 32768.0).collect(),
            Err(_) => Vec::new(),
        };
    }
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Vec::new();
    }
    // Walk the chunks to the audio data.
    let mut at = 12;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        if &b[at..at + 4] == b"data" {
            let data = &b[at + 8..(at + 8 + size).min(b.len())];
            return data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
        }
        at += 8 + size + (size & 1);
    }
    Vec::new()
}

/// Simple two-pole filter (RBJ biquad).
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}
impl Biquad {
    fn new(kind: char, freq: f32, q: f32) -> Self {
        let w = std::f32::consts::TAU * freq / RATE as f32;
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q);
        let (b0, b1, b2) = if kind == 'h' { ((1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0) } else { ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0) };
        let a0 = 1.0 + alpha;
        Biquad { b: [b0 / a0, b1 / a0, b2 / a0], a: [-2.0 * cs / a0, (1.0 - alpha) / a0], z: [0.0; 2] }
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// Police-scanner radio: narrow band, a little drive, hiss, and a squelch burst at each end.
fn radio(x: &[f32]) -> Vec<f32> {
    let mut seed = 0x2545F491u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let (mut hp, mut hp2, mut lp, mut lp2) = (Biquad::new('h', 350.0, 0.7), Biquad::new('h', 350.0, 0.7), Biquad::new('l', 3200.0, 0.7), Biquad::new('l', 3200.0, 0.7));
    let mut y: Vec<f32> = x.iter().map(|&s| lp2.run(lp.run(hp2.run(hp.run(s))))).collect();
    let peak = y.iter().fold(1e-6f32, |m, v| m.max(v.abs()));
    let drive = 2.2f32;
    for v in y.iter_mut() {
        *v = (*v / peak * drive).tanh() / drive.tanh() * 0.8;
    }
    let burst = |len: f32, noise: &mut dyn FnMut() -> f32| -> Vec<f32> {
        let n = (len * RATE as f32) as usize;
        let (mut h, mut l) = (Biquad::new('h', 900.0, 0.7), Biquad::new('l', 5000.0, 0.7));
        (0..n).map(|i| l.run(h.run(noise())) * 0.5 * (1.0 - i as f32 / n as f32).powf(1.5)).collect()
    };
    let mut out = burst(0.05, &mut noise);
    out.extend((0..(0.06 * RATE as f32) as usize).map(|_| 0.0));
    out.extend(y);
    out.extend((0..(0.05 * RATE as f32) as usize).map(|_| 0.0));
    out.extend(burst(0.12, &mut noise));
    for v in out.iter_mut() {
        *v += noise() * 0.015;
    }
    out
}

/// Say clips one after another (with a short gap) and wait until done.
fn play(voice: &str, radio_fx: bool, names: &[&str], gap_ms: u64) {
    let mut all: Vec<f32> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            all.extend(std::iter::repeat_n(0.0, (gap_ms as u32 * RATE / 1000) as usize));
        }
        if *n == "_pause" || *n == "_beat" {
            // A breath after the postal, and a beat before the road name.
            let secs = if *n == "_pause" { 0.25 } else { 0.08 };
            all.extend(std::iter::repeat_n(0.0, (RATE as f32 * secs) as usize));
            continue;
        }
        all.extend(samples(voice, n));
    }
    if all.is_empty() {
        return;
    }
    let all = if radio_fx { radio(&all) } else { all };
    let Ok((_stream, handle)) = rodio::OutputStream::try_default() else { return };
    let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
    sink.append(rodio::buffer::SamplesBuffer::new(1, RATE, all));
    sink.sleep_until_end();
}

/// A short two-tone radio chirp: "go ahead, I'm listening" (or a low one: "nothing").
fn chirp(up: bool) {
    let rate = 44_100u32;
    let mut s = Vec::new();
    let tones: [(f32, f32); 2] = if up { [(880.0, 0.07), (1320.0, 0.09)] } else { [(660.0, 0.08), (440.0, 0.12)] };
    for (f, len) in tones {
        let n = (len * rate as f32) as usize;
        for i in 0..n {
            let t = i as f32 / rate as f32;
            let env = (i.min(n - i) as f32 / (0.008 * rate as f32)).min(1.0);
            s.push((t * f * std::f32::consts::TAU).sin() * 0.25 * env);
        }
    }
    let Ok((_stream, handle)) = rodio::OutputStream::try_default() else { return };
    let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
    sink.append(rodio::buffer::SamplesBuffer::new(1, rate, s));
    sink.sleep_until_end();
}

/// Settings → Voice → Play.
pub fn play_sample(voice: &str, radio_fx: bool) {
    let v = voice.to_string();
    std::thread::spawn(move || play(&v, radio_fx, &["greet"], 0));
}

/// Say the postal back with its nearest road, and the corner or intersection
/// when the map shows one (same words as the box on the map):
/// "Copy, postal five two four. The nearest road is Eclipse Boulevard.
///  Intersection of Eclipse Boulevard and Milton Road."
fn read_back(voice: &str, radio_fx: bool, crossing: bool, code: &str) {
    let mut names: Vec<String> = vec!["copy".into()];
    names.extend(code.chars().filter(|c| c.is_ascii_digit()).map(|c| format!("d{c}")));
    if let Some(&(_, x, y)) = crate::postals::POSTALS.iter().find(|p| p.0 == code) {
        if let Some(place) = crate::streets::describe(x, y) {
            let clip_name = format!("s/{}", place.street);
            if !clip(voice, &clip_name).is_empty() {
                names.push("_pause".into());
                names.push("nearest".into());
                names.push("_beat".into());
                names.push(clip_name);
                if let Some(c) = place.crossing.as_ref().filter(|_| crossing) {
                    let kind = match c.kind {
                        crate::streets::Kind::Intersection => "k_intersection",
                        crate::streets::Kind::TIntersection => "k_t",
                        crate::streets::Kind::Corner => "k_corner",
                    };
                    let (a, b) = (format!("s/{}", c.streets[0]), format!("s/{}", c.streets[1]));
                    if !clip(voice, &a).is_empty() && !clip(voice, &b).is_empty() {
                        names.extend(["_pause".into(), kind.into(), "_beat".into(), a, "_beat".into(), "and".into(), "_beat".into(), b]);
                    }
                }
            }
        }
    }
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    play(voice, radio_fx, &names, 90);
}

// ── Understanding what was said ───────────────────────────────────────────

fn word_value(w: &str) -> Option<(u32, u8)> {
    // (value, kind): 0 = single digit, 1 = 10–19, 2 = tens
    Some(match w {
        "zero" | "oh" | "o" => (0, 0),
        "one" => (1, 0),
        "two" => (2, 0),
        "three" => (3, 0),
        "four" => (4, 0),
        "five" => (5, 0),
        "six" => (6, 0),
        "seven" => (7, 0),
        "eight" => (8, 0),
        "nine" => (9, 0),
        "ten" => (10, 1),
        "eleven" => (11, 1),
        "twelve" => (12, 1),
        "thirteen" => (13, 1),
        "fourteen" => (14, 1),
        "fifteen" => (15, 1),
        "sixteen" => (16, 1),
        "seventeen" => (17, 1),
        "eighteen" => (18, 1),
        "nineteen" => (19, 1),
        "twenty" => (20, 2),
        "thirty" => (30, 2),
        "forty" => (40, 2),
        "fifty" => (50, 2),
        "sixty" => (60, 2),
        "seventy" => (70, 2),
        "eighty" => (80, 2),
        "ninety" => (90, 2),
        _ => return None,
    })
}

/// Turn what was said into a postal number. Handles "three six one",
/// "three sixty one", "three hundred and sixty one", "ten twenty nine" and "1029".
pub fn parse_postal(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let digits: String = words.iter().filter(|w| w.chars().all(|c| c.is_ascii_digit())).copied().collect();
    if !digits.is_empty() {
        return Some(digits.trim_start_matches('0').to_string()).filter(|s| !s.is_empty());
    }
    let words: Vec<&str> = words.into_iter().filter(|w| !matches!(*w, "postal" | "go" | "to" | "code" | "and" | "number")).collect();
    if words.is_empty() {
        return None;
    }
    let out = if words.iter().any(|w| *w == "hundred" || *w == "thousand") {
        // Counted out: "three hundred sixty one", "three thousand and two".
        let (mut total, mut cur) = (0u32, 0u32);
        for w in &words {
            match *w {
                "hundred" => cur = cur.max(1) * 100,
                "thousand" => {
                    total += cur.max(1) * 1000;
                    cur = 0;
                }
                _ => cur += word_value(w)?.0,
            }
        }
        (total + cur).to_string()
    } else {
        // Read out in pieces: "three six one", "three sixty one", "ten twenty nine".
        let mut s = String::new();
        let mut i = 0;
        while i < words.len() {
            let (v, kind) = word_value(words[i])?;
            if kind == 2 {
                match words.get(i + 1).and_then(|w| word_value(w)) {
                    Some((d, 0)) if d > 0 => {
                        s += &(v + d).to_string();
                        i += 2;
                        continue;
                    }
                    _ => s += &v.to_string(),
                }
            } else {
                s += &v.to_string();
            }
            i += 1;
        }
        s
    };
    let out = out.trim_start_matches('0').to_string();
    (!out.is_empty() && out.len() <= 5).then_some(out)
}

fn postal_exists(code: &str) -> bool {
    crate::postals::POSTALS.iter().any(|p| p.0 == code)
}

fn is_cancel(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("cancel") || t.contains("never mind") || t.contains("disregard")
}


// ── Speech engine (Vosk) ──────────────────────────────────────────────────

const WAKE_PHRASES: [&str; 6] = ["fire com dispatch", "fire calm dispatch", "fire come dispatch", "fire comm dispatch", "fire comms dispatch", "fire con dispatch"];

fn wake_grammar() -> String {
    let mut g: Vec<&str> = WAKE_PHRASES.to_vec();
    g.push("[unk]");
    serde_json::to_string(&g).unwrap()
}

fn postal_grammar() -> String {
    let words = "zero oh one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty thirty forty fifty sixty seventy eighty ninety hundred thousand and postal go to cancel never mind disregard repeat that";
    let mut g: Vec<&str> = words.split(' ').collect();
    g.push("[unk]");
    serde_json::to_string(&g).unwrap()
}

/// The speech engine download: (url, sha256, size in bytes).
#[cfg(windows)]
const ENGINE: (&str, &str, u64) = ("https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip", "f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b", 14_882_445);
#[cfg(not(windows))]
const ENGINE: (&str, &str, u64) = ("https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip", "bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2", 7_185_065);
const MODEL: (&str, &str, u64) = ("https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip", "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498", 41_205_931);
#[cfg(windows)]
const LIB: &str = "libvosk.dll";
#[cfg(not(windows))]
const LIB: &str = "libvosk.so";

fn speech_dir() -> PathBuf {
    // MFB_SPEECH_DIR (for testing) points at a folder that already has the engine.
    std::env::var("MFB_SPEECH_DIR").map(PathBuf::from).unwrap_or_else(|_| crate::storage::data_dir().join("speech"))
}

fn installed(dir: &Path) -> bool {
    dir.join(LIB).exists() && dir.join("model").join("am").join("final.mdl").exists()
}

/// Download one zip (checking it), and unpack it into `into` without its top folder.
fn fetch(url: &str, sha: &str, size: u64, into: &Path, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    // MFB_SPEECH_BASE (for testing) fetches the same files from somewhere else.
    let url = match std::env::var("MFB_SPEECH_BASE") {
        Ok(base) => format!("{base}/{}", url.rsplit('/').next().unwrap_or(url)),
        Err(_) => url.to_string(),
    };
    let resp = ureq::get(&url).timeout(Duration::from_secs(60)).call().map_err(|_| "Couldn't download the speech engine. Check the internet connection — it's only needed once.".to_string())?;
    let mut reader = resp.into_reader();
    let mut data = Vec::with_capacity(size as usize);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|_| "The speech engine download was cut off. It will try again.".to_string())?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        progress(n as u64);
    }
    let digest = format!("{:x}", Sha256::digest(&data));
    if digest != sha {
        return Err("The speech engine download didn't check out. It will try again.".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| "The speech engine download couldn't be opened.".to_string())?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = f.enclosed_name() else { continue };
        // Drop the top folder ("vosk-win64-0.3.45/libvosk.dll" -> "libvosk.dll").
        let rel: PathBuf = path.components().skip(1).collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = into.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn install(dir: &Path, progress: &mut dyn FnMut(f32)) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let total = (ENGINE.2 + MODEL.2) as f32;
    let mut done = 0u64;
    let mut tick = |n: u64| {
        done += n;
        progress(done as f32 / total);
    };
    fetch(ENGINE.0, ENGINE.1, ENGINE.2, dir, &mut tick)?;
    fetch(MODEL.0, MODEL.1, MODEL.2, &dir.join("model"), &mut tick)?;
    Ok(())
}

type ModelNew = unsafe extern "C" fn(*const std::ffi::c_char) -> *mut std::ffi::c_void;
type RecNew = unsafe extern "C" fn(*mut std::ffi::c_void, f32, *const std::ffi::c_char) -> *mut std::ffi::c_void;
type Accept = unsafe extern "C" fn(*mut std::ffi::c_void, *const i16, i32) -> i32;
type Text = unsafe extern "C" fn(*mut std::ffi::c_void) -> *const std::ffi::c_char;
type Free = unsafe extern "C" fn(*mut std::ffi::c_void);
type LogLevel = unsafe extern "C" fn(i32);
type SetFlag = unsafe extern "C" fn(*mut std::ffi::c_void, i32);

struct Vosk {
    _lib: libloading::Library,
    model: *mut std::ffi::c_void,
    rec_new: RecNew,
    accept: Accept,
    result: Text,
    partial: Text,
    final_result: Text,
    rec_free: Free,
    set_words: Option<SetFlag>,
    set_partial_words: Option<SetFlag>,
}

impl Vosk {
    fn load(dir: &Path) -> Result<Vosk, String> {
        let lib_path = dir.join(LIB);
        #[cfg(windows)]
        let lib = unsafe {
            // Load its helper DLLs from the same folder.
            libloading::os::windows::Library::load_with_flags(&lib_path, 0x0000_0008).map(libloading::Library::from)
        };
        #[cfg(not(windows))]
        let lib = unsafe { libloading::Library::new(&lib_path) };
        let lib = lib.map_err(|e| format!("The speech engine couldn't start ({e})."))?;
        unsafe {
            let sym = |name: &[u8]| -> Result<*const (), String> {
                lib.get::<*const ()>(name).map(|s| *s).map_err(|_| "The speech engine is incomplete. Delete the \"speech\" folder in the app's data to get it again.".to_string())
            };
            let log: LogLevel = std::mem::transmute(sym(b"vosk_set_log_level\0")?);
            log(-1);
            let model_new: ModelNew = std::mem::transmute(sym(b"vosk_model_new\0")?);
            let rec_new: RecNew = std::mem::transmute(sym(b"vosk_recognizer_new_grm\0")?);
            let accept: Accept = std::mem::transmute(sym(b"vosk_recognizer_accept_waveform_s\0")?);
            let result: Text = std::mem::transmute(sym(b"vosk_recognizer_result\0")?);
            let partial: Text = std::mem::transmute(sym(b"vosk_recognizer_partial_result\0")?);
            let final_result: Text = std::mem::transmute(sym(b"vosk_recognizer_final_result\0")?);
            let rec_free: Free = std::mem::transmute(sym(b"vosk_recognizer_free\0")?);
            let set_words: Option<SetFlag> = sym(b"vosk_recognizer_set_words\0").ok().map(|p| std::mem::transmute(p));
            let set_partial_words: Option<SetFlag> = sym(b"vosk_recognizer_set_partial_words\0").ok().map(|p| std::mem::transmute(p));
            let model_path = std::ffi::CString::new(dir.join("model").to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            let model = model_new(model_path.as_ptr());
            if model.is_null() {
                return Err("The speech engine's language model couldn't be loaded. Delete the \"speech\" folder in the app's data to get it again.".into());
            }
            Ok(Vosk { _lib: lib, model, rec_new, accept, result, partial, final_result, rec_free, set_words, set_partial_words })
        }
    }

    fn recognizer(&self, grammar: &str) -> Rec<'_> {
        let g = std::ffi::CString::new(grammar).unwrap();
        let p = unsafe { (self.rec_new)(self.model, 16_000.0, g.as_ptr()) };
        // Word timings, so the wake phrase can be told apart from ordinary talk.
        unsafe {
            if let Some(f) = self.set_words {
                f(p, 1);
            }
            if let Some(f) = self.set_partial_words {
                f(p, 1);
            }
        }
        Rec { v: self, p }
    }
}

struct Rec<'a> {
    v: &'a Vosk,
    p: *mut std::ffi::c_void,
}

fn json_field(ptr: *const std::ffi::c_char, field: &str) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy();
    serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v.get(field).and_then(|t| t.as_str()).map(str::to_string)).unwrap_or_default()
}

/// The words in a result, with their start and end times (seconds).
fn json_words(ptr: *const std::ffi::c_char, field: &str) -> Vec<(String, f32, f32)> {
    if ptr.is_null() {
        return Vec::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else { return Vec::new() };
    v.get(field)
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|w| Some((w.get("word")?.as_str()?.to_string(), w.get("start")?.as_f64()? as f32, w.get("end")?.as_f64()? as f32)))
                .collect()
        })
        .unwrap_or_default()
}

impl Rec<'_> {
    /// Feed audio and check for the wake phrase as it's being said. (Every other
    /// command, like "repeat that", only counts after the wake phrase or the key,
    /// so it can't go off by accident while roleplaying.)
    fn feed_wake(&mut self, pcm: &[i16]) -> bool {
        let ended = self.feed(pcm);
        let raw = if ended { unsafe { (self.v.result)(self.p) } } else { unsafe { (self.v.partial)(self.p) } };
        let words = json_words(raw, if ended { "result" } else { "partial_result" });
        is_wake_words(&words)
    }
    /// Feed 16 kHz audio. True when a sentence has just finished (then call `text`).
    fn feed(&mut self, pcm: &[i16]) -> bool {
        unsafe { (self.v.accept)(self.p, pcm.as_ptr(), pcm.len() as i32) == 1 }
    }
    fn text(&mut self) -> String {
        json_field(unsafe { (self.v.result)(self.p) }, "text")
    }
    fn partial(&mut self) -> String {
        json_field(unsafe { (self.v.partial)(self.p) }, "partial")
    }
    fn finish(&mut self) -> String {
        json_field(unsafe { (self.v.final_result)(self.p) }, "text")
    }
}
impl Drop for Rec<'_> {
    fn drop(&mut self) {
        unsafe { (self.v.rec_free)(self.p) }
    }
}

/// Only "[unk]" means nothing it was listening for was said.
fn clean(text: &str) -> String {
    text.split_whitespace().filter(|w| *w != "[unk]").collect::<Vec<_>>().join(" ")
}

// ── The microphone ────────────────────────────────────────────────────────

/// Names of the microphones on this PC.
fn list_mics() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    host.input_devices().map(|ds| ds.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

struct Mic {
    _stream: cpal::Stream,
    rx: mpsc::Receiver<Vec<i16>>,
    failed: std::sync::Arc<AtomicBool>,
    name: String,
}

/// Open a microphone by name ("" = the Windows default). Audio arrives as 16 kHz mono.
fn open_mic(wanted: &str) -> Option<Mic> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = if wanted.is_empty() {
        host.default_input_device()
    } else {
        host.input_devices().ok().and_then(|mut ds| ds.find(|d| d.name().map(|n| n == wanted).unwrap_or(false))).or_else(|| host.default_input_device())
    }?;
    let name = device.name().unwrap_or_else(|_| "Microphone".into());
    let config = device.default_input_config().ok()?;
    let channels = config.channels() as usize;
    let rate = config.sample_rate().0 as f32;
    let (tx, rx) = mpsc::channel();
    let failed = std::sync::Arc::new(AtomicBool::new(false));
    let failed2 = failed.clone();
    // Down to mono, a gentle low-pass, then keep every (rate/16k)th sample.
    let step = 16_000.0 / rate;
    let mut phase = 0.0f32;
    let mut lp = 0.0f32;
    let alpha = 1.0 - (-std::f32::consts::TAU * 7_000.0 / rate).exp();
    let mut level = 0.0f32;
    let mut handle = move |mono: &mut dyn Iterator<Item = f32>| {
        let mut out = Vec::with_capacity(512);
        let mut peak = 0.0f32;
        for x in mono {
            lp += alpha * (x - lp);
            peak = peak.max(x.abs());
            phase += step;
            if phase >= 1.0 {
                phase -= 1.0;
                out.push((lp * 32767.0).clamp(-32768.0, 32767.0) as i16);
            }
        }
        level = (level * 0.8).max(peak);
        LEVEL.store(level.min(1.0).to_bits(), Ordering::Relaxed);
        let _ = tx.send(out);
    };
    let err = move |_| failed2.store(true, Ordering::SeqCst);
    let cfg: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(&cfg, move |d: &[f32], _: &_| handle(&mut d.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32)), err, None),
        cpal::SampleFormat::I16 => device.build_input_stream(&cfg, move |d: &[i16], _: &_| handle(&mut d.chunks(channels).map(|f| f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32)), err, None),
        cpal::SampleFormat::U16 => device.build_input_stream(&cfg, move |d: &[u16], _: &_| handle(&mut d.chunks(channels).map(|f| f.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum::<f32>() / channels as f32)), err, None),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(Mic { _stream: stream, rx, failed, name })
}

impl Mic {
    /// Throw away audio heard so far (e.g. the dispatcher's own voice).
    fn drain(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

// ── Running it ────────────────────────────────────────────────────────────

fn set_state(core: &Shared, ctx: &egui::Context, state: VoiceState) {
    let mut c = lock(core);
    if c.voice.state != state {
        c.voice.state = state;
        drop(c);
        ctx.request_repaint();
    }
}

/// Start watching the listen key and run the listener.
pub fn start(core: Shared, ctx: egui::Context) {
    start_keys();
    std::thread::spawn(move || engine(core, ctx));
}

/// What to do once something was heard as a postal. Some(true) = done.
fn handle_heard(core: &Shared, ctx: &egui::Context, voice: &str, radio_fx: bool, readback: bool, crossing: bool, text: &str) -> Option<bool> {
    if text.contains("repeat") {
        repeat_last(core, ctx, voice, radio_fx, crossing);
        return Some(true);
    }
    if is_cancel(text) {
        lock(core).voice.heard = Some(format!("“{text}” — cancelled"));
        ctx.request_repaint();
        play(voice, radio_fx, &["cancel"], 0);
        return Some(true);
    }
    match parse_postal(text) {
        Some(code) if postal_exists(&code) => {
            set_last_postal(&code);
            {
                let mut c = lock(core);
                c.voice.heard = Some(format!("“{text}” → postal {code}"));
                c.events.push(UiEvent::MapPostal(code.clone()));
            }
            ctx.request_repaint();
            if readback {
                read_back(voice, radio_fx, crossing, &code);
            }
            Some(true)
        }
        Some(code) => {
            lock(core).voice.heard = Some(format!("“{text}” → postal {code} isn't on the map"));
            ctx.request_repaint();
            play(voice, radio_fx, &["not_found"], 0);
            Some(false)
        }
        None => None,
    }
}

struct Snapshot {
    signed_in: bool,
    mode: String,
    voice: String,
    radio: bool,
    readback: bool,
    crossing: bool,
    key: u32,
    mic: String,
}

fn snapshot(core: &Shared) -> Snapshot {
    let c = lock(core);
    Snapshot {
        signed_in: c.user.is_some(),
        mode: c.settings.voice_mode.clone(),
        voice: c.settings.voice_name.clone(),
        radio: c.settings.voice_radio,
        readback: c.settings.voice_readback,
        crossing: c.settings.voice_crossing,
        key: c.settings.voice_key,
        mic: c.settings.voice_mic.clone(),
    }
}

/// Listen for one sentence. None = nothing said (6 s of silence), or the mic stopped.
fn listen_once(mic: &Mic, rec: &mut Rec) -> Option<String> {
    let start = Instant::now();
    let mut spoke = false;
    loop {
        match mic.rx.recv_timeout(Duration::from_millis(100)) {
            Ok(pcm) => {
                if rec.feed(&pcm) {
                    let t = clean(&rec.text());
                    if !t.is_empty() {
                        return Some(t);
                    }
                } else if !spoke && !clean(&rec.partial()).is_empty() {
                    spoke = true;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(_) => {}
        }
        if mic.failed.load(Ordering::SeqCst) {
            return None;
        }
        let waited = start.elapsed();
        if (!spoke && waited > Duration::from_secs(6)) || waited > Duration::from_secs(12) {
            let t = clean(&rec.finish());
            return (!t.is_empty()).then_some(t);
        }
    }
}

/// Listen for a postal (two tries) and go there.
fn take_postal(core: &Shared, ctx: &egui::Context, vosk: &Vosk, mic: &Mic, s: &Snapshot, by_phrase: bool) {
    let grammar = postal_grammar();
    for attempt in 0..2 {
        mic.drain();
        set_state(core, ctx, VoiceState::Listening);
        let mut rec = vosk.recognizer(&grammar);
        let heard = listen_once(mic, &mut rec);
        set_state(core, ctx, VoiceState::Speaking);
        match heard {
            Some(text) => match handle_heard(core, ctx, &s.voice, s.radio, s.readback, s.crossing, &text) {
                Some(true) => break,
                Some(false) => {}
                None => {
                    lock(core).voice.heard = Some(format!("“{text}” — not a postal"));
                    play(&s.voice, s.radio, &["say_again"], 0);
                }
            },
            None => {
                if attempt == 0 && by_phrase {
                    play(&s.voice, s.radio, &["say_again"], 0);
                } else {
                    if by_phrase {
                        play(&s.voice, s.radio, &["no_hear"], 0);
                    } else {
                        chirp(false);
                    }
                    break;
                }
            }
        }
    }
    mic.drain();
    PRESSED.store(false, Ordering::SeqCst);
}

/// Did they say the wake phrase? "Fire Com dispatch" (or how it may be heard:
/// "fire comm / calm / come / con dispatch"), said as its own command: at the start,
/// or after a short pause — not in the middle of a sentence.
fn is_wake_words(words: &[(String, f32, f32)]) -> bool {
    const MIDDLE: [&str; 6] = ["com", "comm", "comms", "calm", "come", "con"];
    for i in 0..words.len().saturating_sub(2) {
        if words[i].0 == "fire" && MIDDLE.contains(&words[i + 1].0.as_str()) && words[i + 2].0 == "dispatch" && (i == 0 || words[i].1 - words[i - 1].2 >= 0.3) {
            return true;
        }
    }
    false
}

/// The last postal the map went to (by voice, search or a click), for "repeat that".
static LAST_POSTAL: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The map: this postal is now selected.
pub fn set_last_postal(code: &str) {
    *LAST_POSTAL.lock().unwrap_or_else(|e| e.into_inner()) = Some(code.to_string());
}

/// Say the last postal again (and show it on the map).
fn repeat_last(core: &Shared, ctx: &egui::Context, voice: &str, radio_fx: bool, crossing: bool) {
    let last = LAST_POSTAL.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match last {
        Some(code) => {
            {
                let mut c = lock(core);
                c.voice.heard = Some(format!("“repeat that” → postal {code}"));
                c.events.push(UiEvent::MapPostal(code.clone()));
            }
            ctx.request_repaint();
            read_back(voice, radio_fx, crossing, &code);
        }
        None => play(voice, radio_fx, &["nothing_repeat"], 0),
    }
}

fn engine(core: Shared, ctx: egui::Context) {
    // The engine is loaded once and kept for as long as the app runs.
    let mut vosk: Option<&'static Vosk> = None;
    let mut mic: Option<Mic> = None;
    let mut mic_wanted = String::from("\u{0}");
    let mut last_list = Instant::now() - Duration::from_secs(60);
    let mut last_try = Instant::now() - Duration::from_secs(60);
    let wake = wake_grammar();
    // Listens for the phrase all the time (kept between passes so nothing is cut off).
    let mut wake_rec: Option<Rec<'static>> = None;
    loop {
        // Keep the list of microphones fresh (for Settings).
        if last_list.elapsed() > Duration::from_secs(3) {
            let mics = list_mics();
            let mut c = lock(&core);
            if c.voice.mics != mics {
                c.voice.mics = mics;
                ctx.request_repaint();
            }
            last_list = Instant::now();
        }
        let s = snapshot(&core);
        KEY.store(s.key, Ordering::SeqCst);
        let on = s.signed_in && s.mode != "off";
        let testing = testing();
        if !on && !testing {
            mic = None;
            wake_rec = None;
            LEVEL.store(0, Ordering::Relaxed);
            lock(&core).voice.mic_in_use = None;
            set_state(&core, &ctx, VoiceState::Off);
            PRESSED.store(false, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }

        // 1. A microphone. Nothing else happens until there is one.
        if mic.as_ref().is_some_and(|m| m.failed.load(Ordering::SeqCst)) || mic_wanted != s.mic {
            mic = None;
            wake_rec = None;
        }
        if mic.is_none() && (mic_wanted != s.mic || last_try.elapsed() > Duration::from_secs(2)) {
            mic_wanted = s.mic.clone();
            last_try = Instant::now();
            mic = open_mic(&s.mic);
            lock(&core).voice.mic_in_use = mic.as_ref().map(|m| m.name.clone());
        }
        let Some(m) = mic.as_ref() else {
            LEVEL.store(0, Ordering::Relaxed);
            set_state(&core, &ctx, VoiceState::NoMic);
            PRESSED.store(false, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        if !on {
            // Only the microphone test is running: keep the level bar moving.
            set_state(&core, &ctx, VoiceState::Off);
            wake_rec = None;
            std::thread::sleep(Duration::from_millis(100));
            m.drain();
            continue;
        }

        // 2. The speech engine (downloaded the first time).
        if vosk.is_none() {
            let dir = speech_dir();
            if !installed(&dir) {
                set_state(&core, &ctx, VoiceState::Downloading(0.0));
                let mut last = Instant::now();
                let r = install(&dir, &mut |p| {
                    if last.elapsed() > Duration::from_millis(200) {
                        set_state(&core, &ctx, VoiceState::Downloading(p));
                        last = Instant::now();
                    }
                });
                if let Err(e) = r {
                    set_state(&core, &ctx, VoiceState::Problem(e));
                    std::thread::sleep(Duration::from_secs(30));
                    continue;
                }
            }
            match Vosk::load(&dir) {
                Ok(v) => vosk = Some(Box::leak(Box::new(v))),
                Err(e) => {
                    set_state(&core, &ctx, VoiceState::Problem(e));
                    std::thread::sleep(Duration::from_secs(30));
                    continue;
                }
            }
        }
        let v: &'static Vosk = vosk.unwrap();

        // 3. Wait for the key (and the phrase).
        set_state(&core, &ctx, VoiceState::Waiting);
        if s.mode == "both" {
            if wake_rec.is_none() {
                m.drain();
                wake_rec = Some(v.recognizer(&wake));
            }
        } else {
            wake_rec = None;
        }
        let started = Instant::now();
        // Re-check settings and microphones every couple of seconds; the phrase
        // listener keeps running across these passes.
        while started.elapsed() < Duration::from_secs(2) {
            if PRESSED.swap(false, Ordering::SeqCst) {
                wake_rec = None;
                chirp(true);
                take_postal(&core, &ctx, v, m, &s, false);
                break;
            }
            let Ok(pcm) = m.rx.recv_timeout(Duration::from_millis(30)) else {
                if m.failed.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            };
            let Some(r) = wake_rec.as_mut() else { continue };
            // React as soon as the phrase is heard, not only when they stop talking.
            if r.feed_wake(&pcm) {
                wake_rec = None;
                set_state(&core, &ctx, VoiceState::Speaking);
                play(&s.voice, s.radio, &["greet"], 0);
                take_postal(&core, &ctx, v, m, &s, true);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_postal;
    #[test]
    fn numbers() {
        let p = |s: &str| parse_postal(s).unwrap_or_default();
        assert_eq!(p("three six one"), "361");
        assert_eq!(p("postal three sixty one"), "361");
        assert_eq!(p("three hundred and sixty one"), "361");
        assert_eq!(p("ten twenty nine"), "1029");
        assert_eq!(p("one oh two nine"), "1029");
        assert_eq!(p("thirty thirty one"), "3031");
        assert_eq!(p("three thousand and two"), "3002");
        assert_eq!(p("go to postal one two five"), "125");
        assert_eq!(p("twenty"), "20");
        assert_eq!(p("nine forty"), "940");
        assert_eq!(p("Postal 361."), "361");
        let w = |v: &[(&str, f32, f32)]| v.iter().map(|(a, b, c)| (a.to_string(), *b, *c)).collect::<Vec<_>>();
        assert!(super::is_wake_words(&w(&[("fire", 0.1, 0.3), ("com", 0.3, 0.5), ("dispatch", 0.5, 0.9)])));
        assert!(super::is_wake_words(&w(&[("[unk]", 0.0, 0.8), ("fire", 1.3, 1.5), ("calm", 1.5, 1.7), ("dispatch", 1.7, 2.0)])));
        assert!(!super::is_wake_words(&w(&[("[unk]", 0.0, 0.8), ("fire", 0.82, 1.0), ("con", 1.0, 1.2), ("dispatch", 1.2, 1.5)])));
        assert!(!super::is_wake_words(&w(&[("fire", 0.1, 0.3), ("dispatch", 0.5, 0.9)])));
    }

    /// Real speech through the real engine. Run with MFB_SPEECH_DIR (engine + model)
    /// and MFB_TEST_AUDIO (folder of 16 kHz mono 16-bit WAVs named after what they say).
    #[test]
    fn engine_hears() {
        let (Ok(_), Ok(audio)) = (std::env::var("MFB_SPEECH_DIR"), std::env::var("MFB_TEST_AUDIO")) else { return };
        let v = super::Vosk::load(&super::speech_dir()).expect("engine loads");
        // Each recording plus 1.5 s of quiet after it, like a live microphone.
        let read = |name: &str| -> Vec<i16> {
            let b = std::fs::read(format!("{audio}/{name}.wav")).unwrap();
            let mut v: Vec<i16> = b[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
            v.extend(std::iter::repeat_n(0i16, 24_000));
            v
        };
        let run = |grammar: &str, pcm: &[i16]| -> String {
            let mut r = v.recognizer(grammar);
            let mut out = String::new();
            for c in pcm.chunks(1600) {
                if r.feed(c) {
                    out = super::clean(&r.text());
                }
            }
            if out.is_empty() { super::clean(&r.finish()) } else { out }
        };
        let wake = super::wake_grammar();
        let post = super::postal_grammar();
        // Heard straight away, while still talking.
        let mut r = v.recognizer(&wake);
        assert!(read("fire com dispatch").chunks(1600).any(|c| r.feed_wake(c)));
        // Said in the middle of other talk, with no pause before it.
        let mut long = read("chatter");
        long.extend(read("fire com dispatch"));
        let mut r = v.recognizer(&wake);
        assert!(long.chunks(1600).any(|c| r.feed_wake(c)), "phrase after chatter and a pause");
        // In the middle of a sentence it doesn't count.
        let mut r = v.recognizer(&wake);
        assert!(!read("fire mid sentence").chunks(1600).any(|c| r.feed_wake(c)), "mid-sentence");
        // "Repeat that" on its own does nothing (roleplay talk)...
        for name in ["repeat that", "can you repeat that"] {
            let mut r = v.recognizer(&wake);
            assert!(!read(name).chunks(1600).any(|c| r.feed_wake(c)), "{name} on its own");
        }
        // ...it only works after the wake phrase or the key, when listening for a postal.
        assert_eq!(run(&post, &read("repeat that")), "repeat that");
        assert_eq!(run(&wake, &read("chatter")), "");
        for (name, code) in [("three six one", "361"), ("three sixty one", "361"), ("ten twenty nine", "1029"), ("postal one two five", "125")] {
            let t = run(&post, &read(name));
            assert_eq!(super::parse_postal(&t).as_deref(), Some(code), "{name}: heard {t}");
        }
        assert!(super::is_cancel(&run(&post, &read("cancel"))));
    }

    /// The first-time download. Run with MFB_TEST_INSTALL=<empty folder>.
    #[test]
    fn engine_downloads() {
        let Ok(dir) = std::env::var("MFB_TEST_INSTALL") else { return };
        let dir = std::path::PathBuf::from(dir);
        let mut last = 0.0;
        super::install(&dir, &mut |p| last = p).expect("installs");
        assert!(super::installed(&dir) && last > 0.99);
        super::Vosk::load(&dir).expect("loads after download");
    }

    /// Every postal's nearest road, and the roads of its corner or intersection,
    /// can be said in every voice.
    #[test]
    fn every_road_is_recorded() {
        let places: Vec<crate::streets::Place> = crate::postals::POSTALS.iter().filter_map(|&(_, x, y)| crate::streets::describe(x, y)).collect();
        let mut roads: Vec<String> = places.iter().map(|p| p.street.clone()).collect();
        roads.extend(places.iter().filter_map(|p| p.crossing.as_ref()).flat_map(|c| c.streets.clone()));
        roads.sort();
        roads.dedup();
        for (v, _, _) in super::VOICES {
            let missing: Vec<&String> = roads.iter().filter(|r| super::samples(v, &format!("s/{r}")).is_empty()).collect();
            assert!(missing.is_empty(), "{v} is missing {missing:?}");
        }
    }

    #[test]
    fn voices_and_radio() {
        for (v, _, _) in super::VOICES {
            for n in ["greet", "copy", "nothing_repeat", "nearest", "k_intersection", "k_t", "k_corner", "and", "say_again", "not_found", "cancel", "no_hear", "d0", "d9"] {
                assert!(!super::samples(v, n).is_empty(), "{v} {n}");
            }
        }
        let r = super::radio(&super::samples("am_fenrir", "greet"));
        assert!(r.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        if let Ok(path) = std::env::var("MFB_RENDER") {
            let pcm: Vec<u8> = r.iter().flat_map(|x| ((x * 32767.0) as i16).to_le_bytes()).collect();
            let mut w = Vec::new();
            w.extend(b"RIFF"); w.extend((36 + pcm.len() as u32).to_le_bytes()); w.extend(b"WAVEfmt ");
            w.extend(16u32.to_le_bytes()); w.extend(1u16.to_le_bytes()); w.extend(1u16.to_le_bytes());
            w.extend(24000u32.to_le_bytes()); w.extend(48000u32.to_le_bytes()); w.extend(2u16.to_le_bytes()); w.extend(16u16.to_le_bytes());
            w.extend(b"data"); w.extend((pcm.len() as u32).to_le_bytes()); w.extend(pcm);
            std::fs::write(path, w).unwrap();
        }
    }
}

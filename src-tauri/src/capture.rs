// Pictures of a Roblox window (phase 5): the live view on the phone and in the app's camera wall,
// and the prints. Windows' own window capture (Windows.Graphics.Capture, the one OBS uses): the
// window is read from what the compositor already drew, so nothing is sent to it, it never moves,
// and one behind others still shows. A minimized window draws nothing: it is told as minimized,
// never restored from here. Only a look: no key, click or focus ever goes to the window.
//
// A capture starts on a window's first request and ends by itself a few seconds after its last,
// so nothing is captured while nobody watches. All of it runs on one thread of its own, which
// owns the Direct3D device and the sessions; requests come in over a channel.
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindow};
use windows::core::Interface;

/// A capture with no request for this long ends.
const IDLE: Duration = Duration::from_secs(5);
/// How long the first request of a window waits for its first frame.
const FIRST_FRAME: Duration = Duration::from_millis(800);

/// A picture of a window, or why there is none.
pub enum Shot {
    Jpeg(Vec<u8>),
    Minimized,
    /// The window closed, or Windows refused to capture it.
    Gone,
}

/// Raw pixels, top-down, 3 bytes (RGB) each.
pub struct Rgb {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

struct Request {
    hwnd: isize,
    /// The picture is scaled down to this width (never up).
    width: u32,
    reply: Sender<Result<Rgb, Shot>>,
}

struct Session {
    _item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    size: SizeInt32,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    /// The last frame read, kept for a request that comes before the next one is drawn.
    last: Option<(u32, u32, Vec<u8>)>,
    used: Instant,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

static WORKER: OnceLock<Mutex<Sender<Request>>> = OnceLock::new();

fn worker() -> Sender<Request> {
    WORKER
        .get_or_init(|| {
            let (tx, rx) = channel();
            std::thread::spawn(move || run(rx));
            Mutex::new(tx)
        })
        .lock()
        .unwrap()
        .clone()
}

/// The window's picture now, scaled down to `width`, as raw pixels. Blocks: call it off the async
/// runtime.
pub fn pixels(hwnd: isize, width: u32) -> Result<Rgb, Shot> {
    let h = HWND(hwnd as *mut _);
    if !unsafe { IsWindow(Some(h)) }.as_bool() {
        return Err(Shot::Gone);
    }
    if unsafe { IsIconic(h) }.as_bool() {
        return Err(Shot::Minimized);
    }
    let (reply, answer) = channel();
    if worker().send(Request { hwnd, width, reply }).is_err() {
        return Err(Shot::Gone);
    }
    answer.recv_timeout(Duration::from_secs(5)).unwrap_or(Err(Shot::Gone))
}

/// The window's picture now as a JPEG. Blocks: call it off the async runtime.
pub fn jpeg(hwnd: isize, width: u32, quality: u8) -> Shot {
    match pixels(hwnd, width) {
        Ok(rgb) => encode(&rgb, quality).map(Shot::Jpeg).unwrap_or(Shot::Gone),
        Err(s) => s,
    }
}

pub fn encode(rgb: &Rgb, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(rgb.data.len() / 8);
    let e = jpeg_encoder::Encoder::new(&mut out, quality);
    e.encode(&rgb.data, rgb.width as u16, rgb.height as u16, jpeg_encoder::ColorType::Rgb).ok()?;
    Some(out)
}

struct Gpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
}

fn gpu() -> windows::core::Result<Gpu> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context))?;
    }
    let device = device.unwrap();
    let dxgi: IDXGIDevice = device.cast()?;
    let winrt: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?.cast()?;
    Ok(Gpu { device, context: context.unwrap(), winrt })
}

fn run(rx: Receiver<Request>) {
    // The capture objects are free-threaded; this thread only needs COM.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let mut gpu_ = None;
    let mut sessions: HashMap<isize, Session> = HashMap::new();
    loop {
        let req = rx.recv_timeout(Duration::from_secs(1));
        sessions.retain(|_, s| s.used.elapsed() < IDLE);
        let Ok(req) = req else { continue };
        if gpu_.is_none() {
            match gpu() {
                Ok(g) => gpu_ = Some(g),
                Err(e) => {
                    eprintln!("capture: no Direct3D device: {e}");
                    let _ = req.reply.send(Err(Shot::Gone));
                    continue;
                }
            }
        }
        let g = gpu_.as_ref().unwrap();
        let fresh = !sessions.contains_key(&req.hwnd);
        if fresh {
            match open(g, req.hwnd) {
                Ok(s) => {
                    sessions.insert(req.hwnd, s);
                }
                Err(e) => {
                    eprintln!("capture: window not captured: {e}");
                    let _ = req.reply.send(Err(Shot::Gone));
                    continue;
                }
            }
        }
        let s = sessions.get_mut(&req.hwnd).unwrap();
        s.used = Instant::now();
        let start = Instant::now();
        let mut got = read(g, s);
        // A capture just started has no frame yet: the first one comes in a moment.
        while fresh && got.is_none() && start.elapsed() < FIRST_FRAME {
            std::thread::sleep(Duration::from_millis(15));
            got = read(g, s);
        }
        if let Some(frame) = got {
            s.last = Some(frame);
        }
        let answer = match &s.last {
            Some((w, h, bgra)) => Ok(scale(bgra, *w, *h, req.width)),
            None => Err(Shot::Gone),
        };
        let _ = req.reply.send(answer);
    }
}

fn open(g: &Gpu, hwnd: isize) -> windows::core::Result<Session> {
    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(HWND(hwnd as *mut _)) }?;
    let size = item.Size()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&g.winrt, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size)?;
    let session = pool.CreateCaptureSession(&item)?;
    // Windows 11 lets the yellow frame and the cursor be left out; older ones refuse, harmlessly.
    let _ = session.SetIsBorderRequired(false);
    let _ = session.SetIsCursorCaptureEnabled(false);
    session.StartCapture()?;
    Ok(Session { _item: item, pool, session, size, staging: None, last: None, used: Instant::now() })
}

/// The newest frame drawn since the last read, as BGRA rows packed tight; None when none was.
fn read(g: &Gpu, s: &mut Session) -> Option<(u32, u32, Vec<u8>)> {
    let mut newest: Option<Direct3D11CaptureFrame> = None;
    while let Ok(f) = s.pool.TryGetNextFrame() {
        if let Some(old) = newest.replace(f) {
            let _ = old.Close();
        }
    }
    let frame = newest?;
    let result = copy(g, s, &frame);
    let content = frame.ContentSize().ok();
    let _ = frame.Close();
    // A window resized (the grid does it) gets buffers of its new size from the next frame on.
    if let Some(c) = content {
        if c.Width > 0 && c.Height > 0 && (c.Width != s.size.Width || c.Height != s.size.Height) {
            s.size = c;
            let _ = s.pool.Recreate(&g.winrt, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, c);
        }
    }
    result
}

fn copy(g: &Gpu, s: &mut Session, frame: &Direct3D11CaptureFrame) -> Option<(u32, u32, Vec<u8>)> {
    let surface = frame.Surface().ok()?;
    let access: IDirect3DDxgiInterfaceAccess = surface.cast().ok()?;
    let texture: ID3D11Texture2D = unsafe { access.GetInterface() }.ok()?;
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    let content = frame.ContentSize().ok()?;
    // The buffer may be bigger than the window for a frame after a resize: only the window's part.
    let w = (content.Width.max(1) as u32).min(desc.Width);
    let h = (content.Height.max(1) as u32).min(desc.Height);
    if s.staging.as_ref().is_none_or(|(_, sw, sh)| *sw != desc.Width || *sh != desc.Height) {
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut t = None;
        unsafe { g.device.CreateTexture2D(&staging_desc, None, Some(&mut t)) }.ok()?;
        s.staging = Some((t?, desc.Width, desc.Height));
    }
    let staging = &s.staging.as_ref()?.0;
    unsafe { g.context.CopyResource(staging, &texture) };
    let mut map = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { g.context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut map)) }.ok()?;
    let pitch = map.RowPitch as usize;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        let line = unsafe { std::slice::from_raw_parts((map.pData as *const u8).add(row * pitch), w as usize * 4) };
        out.extend_from_slice(line);
    }
    unsafe { g.context.Unmap(staging, 0) };
    Some((w, h, out))
}

/// BGRA to RGB, scaled down to `max_width` by averaging each output pixel's box of source pixels.
pub fn scale(bgra: &[u8], w: u32, h: u32, max_width: u32) -> Rgb {
    let (ow, oh) = if w <= max_width.max(1) {
        (w, h)
    } else {
        let ow = max_width.max(1);
        (ow, ((h as u64 * ow as u64) / w as u64).max(1) as u32)
    };
    let mut data = Vec::with_capacity((ow * oh * 3) as usize);
    for oy in 0..oh {
        let y0 = (oy as u64 * h as u64 / oh as u64) as usize;
        let y1 = (((oy + 1) as u64 * h as u64 / oh as u64) as usize).max(y0 + 1);
        for ox in 0..ow {
            let x0 = (ox as u64 * w as u64 / ow as u64) as usize;
            let x1 = (((ox + 1) as u64 * w as u64 / ow as u64) as usize).max(x0 + 1);
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for y in y0..y1 {
                let row = &bgra[(y * w as usize + x0) * 4..(y * w as usize + x1) * 4];
                for p in row.chunks_exact(4) {
                    b += p[0] as u32;
                    g += p[1] as u32;
                    r += p[2] as u32;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            data.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8]);
        }
    }
    Rgb { width: ow, height: oh, data }
}

/// Pictures side by side in a grid of equal cells, on black, each centred in its cell.
pub fn side_by_side(shots: &[Rgb]) -> Option<Rgb> {
    if shots.is_empty() {
        return None;
    }
    let n = shots.len() as u32;
    let cols = (n as f64).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let cw = shots.iter().map(|s| s.width).max()?;
    let ch = shots.iter().map(|s| s.height).max()?;
    let (width, height) = (cw * cols, ch * rows);
    let mut data = vec![0u8; (width * height * 3) as usize];
    for (i, s) in shots.iter().enumerate() {
        let (col, row) = (i as u32 % cols, i as u32 / cols);
        let x = col * cw + (cw - s.width) / 2;
        let y = row * ch + (ch - s.height) / 2;
        for line in 0..s.height {
            let from = (line * s.width * 3) as usize;
            let to = (((y + line) * width + x) * 3) as usize;
            data[to..to + (s.width * 3) as usize].copy_from_slice(&s.data[from..from + (s.width * 3) as usize]);
        }
    }
    Some(Rgb { width, height, data })
}

/// A live picture of the account's first window for the camera wall, as JPEG bytes. Fails with
/// "minimized", "gone" or "none" (no window), which the window turns into its own text.
#[tauri::command]
pub async fn capture_frame(app: tauri::AppHandle, account: String, width: u32) -> Result<tauri::ipc::Response, String> {
    use tauri::Manager;
    let hwnd = app.state::<crate::clients::Clients>().window(&account).ok_or("none")?;
    let shot = tauri::async_runtime::spawn_blocking(move || jpeg(hwnd, width.clamp(160, 1280), 75)).await.map_err(|_| "gone")?;
    match shot {
        Shot::Jpeg(bytes) => Ok(tauri::ipc::Response::new(bytes)),
        Shot::Minimized => Err("minimized".into()),
        Shot::Gone => Err("gone".into()),
    }
}

/// A print saved to Pictures\Quackblox: one account's window, or every one side by side when no
/// account is given. Says where it went.
#[tauri::command]
pub async fn capture_print(app: tauri::AppHandle, account: Option<String>) -> Result<String, String> {
    use tauri::Manager;
    let handle = app.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || match account {
        None => crate::phone::print_all(&handle),
        Some(key) => {
            let hwnd = handle.state::<crate::clients::Clients>().window(&key).ok_or_else(|| tr!("essa conta não tem Roblox aberto", "that account has no Roblox open"))?;
            match jpeg(hwnd, 1920, 90) {
                Shot::Jpeg(b) => Ok(b),
                Shot::Minimized => Err(tr!("a janela está minimizada: minimizada ela não desenha", "the window is minimized: it doesn't draw while minimized")),
                Shot::Gone => Err(tr!("não consegui ver essa janela", "couldn't see that window")),
            }
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    let dir = prints_folder(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| tr!("não consegui criar a pasta: {e}", "couldn't create the folder: {e}"))?;
    let now = crate::win::local_now();
    // Created only when the name is free: two prints in the same second never write over each other.
    let stamp = format!("{} {}", now.day, now.clock.replace(':', "-"));
    for n in 1..100 {
        let path = dir.join(if n == 1 { format!("Quackblox {stamp}.jpg") } else { format!("Quackblox {stamp} ({n}).jpg") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if let Err(e) = std::io::Write::write_all(&mut f, &bytes) {
                    // A half-written picture is not left in the folder.
                    drop(f);
                    let _ = std::fs::remove_file(&path);
                    return Err(tr!("não consegui salvar o print: {e}", "couldn't save the print: {e}"));
                }
                return Ok(path.display().to_string());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(tr!("não consegui salvar o print: {e}", "couldn't save the print: {e}")),
        }
    }
    Err(tr!("não consegui salvar o print: nome ocupado", "couldn't save the print: name taken"))
}

fn prints_folder(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    Ok(app.path().picture_dir().map_err(|e| e.to_string())?.join("Quackblox"))
}

/// Shows a print in Explorer, selected. Only a file in the prints' folder.
#[tauri::command]
pub fn capture_reveal(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let file = std::path::PathBuf::from(&path);
    let dir = prints_folder(&app)?;
    if file.parent() != Some(dir.as_path()) || !file.is_file() {
        return Err(tr!("esse arquivo não é um print do Quackblox", "that file isn't a Quackblox print"));
    }
    std::process::Command::new("explorer").arg(format!("/select,{}", file.display())).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_averages_and_keeps_the_shape() {
        // 4x2 BGRA: left half red, right half blue.
        let red = [0, 0, 255, 255];
        let blue = [255, 0, 0, 255];
        let mut px = vec![];
        for _ in 0..2 {
            px.extend_from_slice(&red);
            px.extend_from_slice(&red);
            px.extend_from_slice(&blue);
            px.extend_from_slice(&blue);
        }
        let out = scale(&px, 4, 2, 2);
        assert_eq!((out.width, out.height), (2, 1));
        assert_eq!(out.data, vec![255, 0, 0, 0, 0, 255]);
        let same = scale(&px, 4, 2, 100);
        assert_eq!((same.width, same.height), (4, 2));
    }

    #[test]
    fn side_by_side_makes_a_grid() {
        let a = Rgb { width: 2, height: 1, data: vec![9; 6] };
        let b = Rgb { width: 2, height: 1, data: vec![7; 6] };
        let c = Rgb { width: 2, height: 1, data: vec![5; 6] };
        let g = side_by_side(&[a, b, c]).unwrap();
        assert_eq!((g.width, g.height), (4, 2));
        assert_eq!(&g.data[0..3], &[9, 9, 9]);
        assert_eq!(&g.data[6..9], &[7, 7, 7]);
        assert_eq!(&g.data[12..15], &[5, 5, 5]);
        assert_eq!(&g.data[18..21], &[0, 0, 0]);
    }

    /// A real window's capture, by hand: QUACKBLOX_CAPTURE_HWND=<decimal handle> cargo test
    /// capture_a_window -- --ignored --nocapture. Writes the picture to the temp folder and says
    /// how long each one took.
    #[test]
    #[ignore]
    fn capture_a_window() {
        let hwnd: isize = std::env::var("QUACKBLOX_CAPTURE_HWND").unwrap().parse().unwrap();
        let width: u32 = std::env::var("QUACKBLOX_CAPTURE_WIDTH").ok().and_then(|w| w.parse().ok()).unwrap_or(480);
        for i in 0..10 {
            let t = Instant::now();
            let shot = jpeg(hwnd, width, 70);
            let took = t.elapsed();
            match shot {
                Shot::Jpeg(b) => {
                    println!("{i}: {} bytes in {took:?}", b.len());
                    std::fs::write(std::env::temp_dir().join("quackblox-capture.jpg"), b).unwrap();
                }
                Shot::Minimized => println!("{i}: minimized"),
                Shot::Gone => println!("{i}: gone"),
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Busy time (kernel + user, 100 ns units) of a process.
    fn busy(pid: u32) -> u64 {
        use windows::Win32::Foundation::{CloseHandle, FILETIME};
        use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        let Ok(h) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else { return 0 };
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let _ = unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) };
        let _ = unsafe { CloseHandle(h) };
        let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        t(k) + t(u)
    }

    /// The CPU the live view costs, by hand, in a release build with real clients open:
    /// QUACKBLOX_CAPTURE_HWNDS=<h1,h2> QUACKBLOX_CAPTURE_PIDS=<roblox pids,dwm pid> cargo test
    /// --release measure_capture_cpu -- --ignored --nocapture. For each rate (pictures a second per
    /// window, 0 first as the baseline), 20 s of capture at 640 px wide: the CPU of the capturer and
    /// of each pid, as a share of one core.
    #[test]
    #[ignore]
    fn measure_capture_cpu() {
        let hwnds: Vec<isize> = std::env::var("QUACKBLOX_CAPTURE_HWNDS").unwrap().split(',').map(|h| h.trim().parse().unwrap()).collect();
        let pids: Vec<u32> = std::env::var("QUACKBLOX_CAPTURE_PIDS").unwrap().split(',').map(|p| p.trim().parse().unwrap()).collect();
        let me = std::process::id();
        let secs = 20u64;
        for rate in [0u64, 2, 4, 8] {
            let before: Vec<u64> = std::iter::once(me).chain(pids.iter().copied()).map(busy).collect();
            let start = Instant::now();
            let mut shots = 0;
            while start.elapsed() < Duration::from_secs(secs) {
                let tick = Instant::now();
                if rate > 0 {
                    for &h in &hwnds {
                        if matches!(jpeg(h, 640, 70), Shot::Jpeg(_)) {
                            shots += 1;
                        }
                    }
                }
                let every = Duration::from_millis(if rate > 0 { 1000 / rate } else { 1000 });
                std::thread::sleep(every.saturating_sub(tick.elapsed()));
            }
            let wall = start.elapsed().as_nanos() as f64 / 100.0;
            let after: Vec<u64> = std::iter::once(me).chain(pids.iter().copied()).map(busy).collect();
            let pct: Vec<String> = before.iter().zip(&after).map(|(b, a)| format!("{:.1}%", (a - b) as f64 / wall * 100.0)).collect();
            println!("rate {rate}/s, {shots} pictures: capturer {}, pids {:?} -> {}", pct[0], pids, pct[1..].join(" "));
        }
    }
}

// Mute per account (phase 6): the sound of a client turned off or on in Windows' own volume mixer,
// the same switch as the speaker under each app there. Nothing is sent to Roblox. A client shows in
// the mixer only once it has made a sound, and Windows remembers an app's mute by its program, so
// the clients' watcher (clients.rs) sets every Quackblox client again at each look: muted for the
// accounts muted, sound on for the others.
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::core::Interface;

/// Once on each thread that calls `apply`.
pub fn init_thread() {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Sets the mute of every audio session of the given processes, on every speaker or headset on.
/// `want`: (pid, muted). Says how many sessions it changed.
pub fn apply(want: &[(u32, bool)]) -> usize {
    if want.is_empty() {
        return 0;
    }
    let Ok(devices) = (unsafe { CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) }) else { return 0 };
    let Ok(list) = (unsafe { devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }) else { return 0 };
    let mut changed = 0;
    for i in 0..unsafe { list.GetCount() }.unwrap_or(0) {
        let Ok(device) = (unsafe { list.Item(i) }) else { continue };
        let Ok(manager) = (unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) }) else { continue };
        let Ok(sessions) = (unsafe { manager.GetSessionEnumerator() }) else { continue };
        for s in 0..unsafe { sessions.GetCount() }.unwrap_or(0) {
            let Ok(control) = (unsafe { sessions.GetSession(s) }) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            let Ok(pid) = (unsafe { control2.GetProcessId() }) else { continue };
            let Some(&(_, muted)) = want.iter().find(|(p, _)| *p == pid) else { continue };
            let Ok(volume) = control.cast::<ISimpleAudioVolume>() else { continue };
            if unsafe { volume.GetMute() }.is_ok_and(|m| m.as_bool() != muted) && unsafe { volume.SetMute(muted, std::ptr::null()) }.is_ok() {
                changed += 1;
            }
        }
    }
    changed
}

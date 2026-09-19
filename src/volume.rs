use windows::core::Result;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

pub struct VolumeController;

impl VolumeController {
    fn get_endpoint_volume() -> Result<IAudioEndpointVolume> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            Ok(endpoint_volume)
        }
    }

    pub fn get_volume() -> std::result::Result<f32, String> {
        let endpoint = Self::get_endpoint_volume().map_err(|e| e.to_string())?;
        let level = unsafe { endpoint.GetMasterVolumeLevelScalar().map_err(|e| e.to_string())? };
        Ok(level)
    }

    pub fn set_volume(level: f32) -> std::result::Result<(), String> {
        let endpoint = Self::get_endpoint_volume().map_err(|e| e.to_string())?;
        let clamped = level.clamp(0.0, 1.0);
        unsafe {
            endpoint.SetMasterVolumeLevelScalar(clamped, std::ptr::null()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn is_muted() -> std::result::Result<bool, String> {
        let endpoint = Self::get_endpoint_volume().map_err(|e| e.to_string())?;
        let muted = unsafe { endpoint.GetMute().map_err(|e| e.to_string())? };
        Ok(muted.as_bool())
    }

    pub fn set_mute(mute: bool) -> std::result::Result<(), String> {
        let endpoint = Self::get_endpoint_volume().map_err(|e| e.to_string())?;
        unsafe {
            endpoint.SetMute(mute, std::ptr::null()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn toggle_mute() -> std::result::Result<bool, String> {
        let endpoint = Self::get_endpoint_volume().map_err(|e| e.to_string())?;
        unsafe {
            let current = endpoint.GetMute().map_err(|e| e.to_string())?;
            let new_mute = !current.as_bool();
            endpoint.SetMute(new_mute, std::ptr::null()).map_err(|e| e.to_string())?;
            Ok(new_mute)
        }
    }
}

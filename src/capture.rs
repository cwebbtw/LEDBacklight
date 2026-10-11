use anyhow::{Result, anyhow, ensure};
use windows::{
    Win32::Graphics::{
        Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
        Direct3D11::*,
        Dxgi::{Common::*, *},
    },
    core::Interface,
};

pub struct Monitor {
    pub name: String,
    pub origin: (i32, i32),
    pub width: i32,
    pub height: i32,
    adapter: IDXGIAdapter1,
    output: IDXGIOutput,
}
pub fn monitors() -> Result<Vec<Monitor>> {
    // SAFETY: COM interfaces own references; enumeration has no raw borrowed inputs.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut result = Vec::new();
        let mut a = 0;
        loop {
            let adapter = match factory.EnumAdapters1(a) {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(e.into()),
            };
            let mut o = 0;
            loop {
                let output = match adapter.EnumOutputs(o) {
                    Ok(o) => o,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(e) => return Err(e.into()),
                };
                let d = output.GetDesc()?;
                if d.AttachedToDesktop.as_bool() {
                    let len = d
                        .DeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(d.DeviceName.len());
                    result.push(Monitor {
                        origin: (d.DesktopCoordinates.left, d.DesktopCoordinates.top),
                        name: String::from_utf16_lossy(&d.DeviceName[..len]),
                        width: d.DesktopCoordinates.right - d.DesktopCoordinates.left,
                        height: d.DesktopCoordinates.bottom - d.DesktopCoordinates.top,
                        adapter: adapter.clone(),
                        output,
                    });
                }
                o += 1;
            }
            a += 1;
        }
        Ok(result)
    }
}

pub struct Frame {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}
pub struct Capture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: Option<ID3D11Texture2D>,
    dimensions: (u32, u32),
}
impl Capture {
    pub fn new(name: &str) -> Result<Self> {
        let monitor = monitors()?
            .into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| anyhow!("monitor {name} unavailable"))?;
        unsafe {
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                &monitor.adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device = device.ok_or_else(|| anyhow!("missing D3D device"))?;
            let duplication = monitor
                .output
                .cast::<IDXGIOutput1>()?
                .DuplicateOutput(&device)?;
            ensure!(
                duplication.GetDesc().Rotation == DXGI_MODE_ROTATION_IDENTITY,
                "rotated displays are not supported; use landscape orientation"
            );
            Ok(Self {
                device,
                context: context.ok_or_else(|| anyhow!("missing D3D context"))?,
                duplication,
                staging: None,
                dimensions: (0, 0),
            })
        }
    }
    pub fn next(&mut self) -> Result<Option<Frame>> {
        unsafe {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            match self
                .duplication
                .AcquireNextFrame(0, &mut info, &mut resource)
            {
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
                Err(e) => return Err(e.into()),
                Ok(()) => (),
            }
            // Always release an acquired frame, including all error paths below.
            let result = self.read(resource);
            let release = self.duplication.ReleaseFrame();
            release?;
            result.map(Some)
        }
    }
    unsafe fn read(&mut self, resource: Option<IDXGIResource>) -> Result<Frame> {
        // SAFETY: acquired resource lives through copy/map; mapped bytes are copied before Unmap.
        unsafe {
            let texture = resource
                .ok_or_else(|| anyhow!("missing frame resource"))?
                .cast::<ID3D11Texture2D>()?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            ensure!(
                desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM,
                "unsupported capture pixel format"
            );
            if self.dimensions != (desc.Width, desc.Height) {
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.MiscFlags = 0;
                let mut staging = None;
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut staging))?;
                self.staging = staging;
                self.dimensions = (desc.Width, desc.Height);
            }
            let staging = self
                .staging
                .as_ref()
                .ok_or_else(|| anyhow!("missing staging texture"))?;
            self.context.CopyResource(staging, &texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let len = (mapped.RowPitch as usize).checked_mul(desc.Height as usize);
            let data =
                len.map(|len| std::slice::from_raw_parts(mapped.pData.cast::<u8>(), len).to_vec());
            self.context.Unmap(staging, 0);
            Ok(Frame {
                data: data.ok_or_else(|| anyhow!("frame size overflow"))?,
                width: desc.Width as usize,
                height: desc.Height as usize,
                pitch: mapped.RowPitch as usize,
            })
        }
    }
}

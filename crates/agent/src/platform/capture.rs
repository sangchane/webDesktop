//! 화면 캡처 — DXGI Desktop Duplication, 기본 모니터(SPEC 이번 범위). 커서는 넣지 않는다(뷰어가 그린다 — 웹앱 input.rs).
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

/// BGRA 한 장(줄 사이 빈칸 없음).
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
}

pub struct Capturer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    output: IDXGIOutput1,
    dup: Option<IDXGIOutputDuplication>,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
}

/// 기본 모니터(바탕 화면 좌표 0,0 을 품은 출력)와 그 출력을 가진 어댑터.
/// 노트북은 그래픽 카드가 둘일 수 있어 기본 어댑터가 화면을 갖지 않을 수 있다.
fn primary_output() -> windows::core::Result<(IDXGIAdapter1, IDXGIOutput1)> {
    // SAFETY: COM 호출 — 결과는 windows 크레이트가 참조 수를 관리한다.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            let mut o = 0;
            while let Ok(output) = adapter.EnumOutputs(o) {
                let d = output.GetDesc()?;
                let r = d.DesktopCoordinates;
                if d.AttachedToDesktop.as_bool() && r.left == 0 && r.top == 0 {
                    return Ok((adapter, output.cast()?));
                }
                o += 1;
            }
            a += 1;
        }
    }
    Err(windows::core::Error::from(DXGI_ERROR_ACCESS_LOST))
}

impl Capturer {
    pub fn new() -> windows::core::Result<Self> {
        let (adapter, output) = primary_output()?;
        let mut device = None;
        let mut ctx = None;
        // SAFETY: 출력 포인터는 이 함수 안의 지역 변수.
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )?;
        }
        let (Some(device), Some(ctx)) = (device, ctx) else {
            return Err(windows::core::Error::from(DXGI_ERROR_ACCESS_LOST));
        };
        let mut c = Self {
            device,
            ctx,
            output,
            dup: None,
            staging: None,
        };
        c.dup = Some(c.duplicate()?);
        Ok(c)
    }

    fn duplicate(&self) -> windows::core::Result<IDXGIOutputDuplication> {
        // SAFETY: COM 호출.
        unsafe { self.output.DuplicateOutput(&self.device) }
    }

    /// 바뀐 화면이 있으면 한 장. 없거나(마우스만 움직임 포함) 잠시 못 읽으면 None.
    /// 바탕 화면이 바뀌면(UAC·잠금·해상도) 복제를 다시 만든다 — 보안 데스크톱은 첫 작업 2.
    pub fn next(&mut self) -> windows::core::Result<Option<Frame>> {
        if self.dup.is_none() {
            match self.duplicate() {
                Ok(d) => self.dup = Some(d),
                Err(_) => return Ok(None),
            }
        }
        let dup = self.dup.clone().expect("위에서 채움"); // COM 참조만 하나 늘린다
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut res: Option<IDXGIResource> = None;
        // SAFETY: 출력 포인터는 지역 변수, 얻은 프레임은 아래에서 ReleaseFrame.
        match unsafe { dup.AcquireNextFrame(0, &mut info, &mut res) } {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                self.dup = None;
                return Ok(None);
            }
            Err(e) => return Err(e),
        }
        let frame = if info.LastPresentTime == 0 {
            Ok(None) // 마우스만 바뀜
        } else {
            res.ok_or_else(|| windows::core::Error::from(DXGI_ERROR_ACCESS_LOST))
                .and_then(|r| r.cast::<ID3D11Texture2D>())
                .and_then(|tex| self.read(&tex).map(Some))
        };
        // SAFETY: 위에서 얻은 프레임을 돌려준다.
        unsafe {
            let _ = dup.ReleaseFrame();
        }
        frame
    }

    fn read(&mut self, tex: &ID3D11Texture2D) -> windows::core::Result<Frame> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: COM 호출, desc 는 지역 변수.
        unsafe { tex.GetDesc(&mut desc) };
        let (w, h) = (desc.Width, desc.Height);
        if self.staging.as_ref().is_none_or(|s| (s.1, s.2) != (w, h)) {
            let sd = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                MipLevels: 1,
                ArraySize: 1,
                ..desc
            };
            let mut t = None;
            // SAFETY: COM 호출.
            unsafe { self.device.CreateTexture2D(&sd, None, Some(&mut t))? };
            self.staging = t.map(|t| (t, w, h));
        }
        let staging = &self.staging.as_ref().expect("위에서 채움").0;
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        let row = w as usize * 4;
        let mut bgra = vec![0u8; row * h as usize];
        // SAFETY: Map 한 동안만 pData 를 읽고(줄 간격 RowPitch), 바로 Unmap.
        unsafe {
            self.ctx.CopyResource(staging, tex);
            self.ctx.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let src = m.pData as *const u8;
            for y in 0..h as usize {
                let line = std::slice::from_raw_parts(src.add(y * m.RowPitch as usize), row);
                bgra[y * row..(y + 1) * row].copy_from_slice(line);
            }
            self.ctx.Unmap(staging, 0);
        }
        Ok(Frame {
            width: w as usize,
            height: h as usize,
            bgra,
        })
    }
}

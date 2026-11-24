//! 하드웨어 H.264 — Media Foundation 인코더 MFT(NVENC·Quick Sync·AMF, 04 「인코딩」). 못 쓰면 Err → openh264(video.rs).
//! 하드웨어 MFT 는 비동기라 이벤트(입력 달라·출력 있음)를 받아 가며 한 장씩 넣고 뺀다. 저지연 모드라 한 장 넣으면 한 장 나온다.
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};
use windows::core::{Interface, Result};
use windows::Win32::Foundation::TRUE;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11Texture2D, ID3D11VideoContext, ID3D11VideoDevice, ID3D11VideoProcessor,
    ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView,
    D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_OPTIMAL_SPEED, D3D11_VPIV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VARIANT;

/// 한 장을 기다리는 상한 — 넘으면 이 인코더를 버리고 openh264 로(멈춘 드라이버에 세션이 묶이지 않게).
const WAIT: Duration = Duration::from_millis(500);

/// GPU 길 — 그래픽카드 안에서 BGRA → NV12(필요하면 줄이기)로 바꿔 인코더에 그대로 넘긴다(CPU 복사·변환 없음).
struct Gpu {
    vdev: ID3D11VideoDevice,
    vctx: ID3D11VideoContext,
    venum: ID3D11VideoProcessorEnumerator,
    proc: ID3D11VideoProcessor,
    nv12: ID3D11Texture2D,
    out_view: ID3D11VideoProcessorOutputView,
    /// 입력 사본은 캡처가 같은 것을 되쓰므로 뷰를 한 번 만들어 둔다(텍스처 주소로 구별).
    in_view: Option<(usize, ID3D11VideoProcessorInputView)>,
    _mgr: IMFDXGIDeviceManager,
}

pub struct HwH264 {
    gpu: Option<Gpu>,
    mft: IMFTransform,
    events: IMFMediaEventGenerator,
    codec: ICodecAPI,
    provides: bool,
    out_size: u32,
    need: u32,
    n: i64,
    frame_100ns: i64,
    pub name: String,
}

fn fail(m: &str) -> windows::core::Error {
    windows::core::Error::new(windows::core::HRESULT(-1), m)
}

impl HwH264 {
    /// `gpu` = (장치, 입력 폭, 입력 높이) 이면 GPU 길, None 이면 NV12 를 CPU 메모리로 받는 길. w·h 는 내보낼 크기.
    pub fn new(
        w: u32,
        h: u32,
        fps: u32,
        bps: u32,
        gpu: Option<(&ID3D11Device, u32, u32)>,
    ) -> Result<Self> {
        unsafe {
            // 이미 다른 방식으로 초기화된 실이면 실패해도 된다.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_LITE)?;
            let input = MFT_REGISTER_TYPE_INFO {
                guidMajorType: MFMediaType_Video,
                guidSubtype: MFVideoFormat_NV12,
            };
            let output = MFT_REGISTER_TYPE_INFO {
                guidMajorType: MFMediaType_Video,
                guidSubtype: MFVideoFormat_H264,
            };
            let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count = 0u32;
            MFTEnumEx(
                MFT_CATEGORY_VIDEO_ENCODER,
                MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
                Some(&input),
                Some(&output),
                &mut list,
                &mut count,
            )?;
            let acts: Vec<Option<IMFActivate>> =
                (0..count as usize).map(|i| list.add(i).read()).collect();
            CoTaskMemFree(Some(list as _));
            let act = acts
                .into_iter()
                .flatten()
                .next()
                .ok_or_else(|| fail("하드웨어 H.264 인코더 없음"))?;
            let name = {
                let mut p = windows::core::PWSTR::null();
                let mut len = 0;
                act.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut p, &mut len)
                    .map(|_| {
                        let s = p.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(p.0 as _));
                        s
                    })
                    .unwrap_or_default()
            };
            let mft: IMFTransform = act.ActivateObject()?;
            let attrs = mft.GetAttributes()?;
            attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
            let _ = attrs.SetUINT32(&MF_LOW_LATENCY, 1);
            let gpu = match gpu {
                Some((dev, iw, ih)) => {
                    if attrs.GetUINT32(&MF_SA_D3D11_AWARE).unwrap_or(0) == 0 {
                        return Err(fail("이 인코더는 D3D11 텍스처를 받지 않음"));
                    }
                    let mut token = 0;
                    let mut mgr = None;
                    MFCreateDXGIDeviceManager(&mut token, &mut mgr)?;
                    let mgr = mgr.ok_or_else(|| fail("장치 관리자 없음"))?;
                    mgr.ResetDevice(dev, token)?;
                    mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, mgr.as_raw() as usize)?;
                    Some(Gpu::new(dev, mgr, iw, ih, w, h, fps)?)
                }
                None => None,
            };
            let codec: ICodecAPI = mft.cast()?;
            codec.SetValue(&CODECAPI_AVLowLatencyMode, &VARIANT::from(true))?;
            // 드라이버마다 받는 값이 달라 나머지는 실패해도 계속(기본값으로).
            let _ = codec.SetValue(
                &CODECAPI_AVEncCommonRateControlMode,
                &VARIANT::from(eAVEncCommonRateControlMode_CBR.0 as u32),
            );
            let _ = codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &VARIANT::from(bps));
            let _ = codec.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &VARIANT::from(0u32));
            let _ = codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &VARIANT::from(fps * 60));

            let size = ((w as u64) << 32) | h as u64;
            let rate = ((fps as u64) << 32) | 1;
            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, bps)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
            out.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            // 웹 뷰어와 맞춘 constrained baseline(str0m 협상) — 안 받으면 baseline.
            out.SetUINT32(
                &MF_MT_MPEG2_PROFILE,
                eAVEncH264VProfile_ConstrainedBase.0 as u32,
            )?;
            if mft.SetOutputType(0, &out, 0).is_err() {
                out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
                mft.SetOutputType(0, &out, 0)?;
            }
            let inp = MFCreateMediaType()?;
            inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            inp.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            inp.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
            inp.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
            inp.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            mft.SetInputType(0, &inp, 0)?;
            let info = mft.GetOutputStreamInfo(0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            Ok(Self {
                gpu,
                events: mft.cast()?,
                mft,
                codec,
                provides: info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0,
                out_size: info.cbSize.max(1 << 20),
                need: 0,
                n: 0,
                frame_100ns: 10_000_000 / fps as i64,
                name,
            })
        }
    }

    /// 다음 이벤트 하나(기다림 상한까지). 입력 달라는 수를 센다.
    fn next_event(&mut self, until: Instant) -> Result<u32> {
        let start = Instant::now();
        loop {
            match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(ev) => {
                    let t = unsafe { ev.GetType()? };
                    if t == METransformNeedInput.0 as u32 {
                        self.need += 1;
                    }
                    return Ok(t);
                }
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => {
                    if Instant::now() > until {
                        return Err(fail("하드웨어 인코더 응답 없음"));
                    }
                    // 한 장이 보통 5~10ms — 그동안은 양보만 한다(Windows 의 1ms 잠은 실제로 15ms 까지 늘어난다).
                    if start.elapsed() < Duration::from_millis(20) {
                        std::thread::yield_now();
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn is_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    /// NV12 한 장(CPU 메모리) → Annex B. 출력이 없으면 빈 Vec.
    pub fn encode(&mut self, nv12: &[u8], keyframe: bool) -> Result<Vec<u8>> {
        let buf = unsafe {
            let buf = MFCreateMemoryBuffer(nv12.len() as u32)?;
            let mut p = std::ptr::null_mut();
            buf.Lock(&mut p, None, None)?;
            std::ptr::copy_nonoverlapping(nv12.as_ptr(), p, nv12.len());
            buf.Unlock()?;
            buf.SetCurrentLength(nv12.len() as u32)?;
            buf
        };
        self.submit(buf, keyframe)
    }

    /// 그래픽카드 안의 BGRA 한 장 → NV12(GPU) → Annex B. GPU 길로 만든 인코더만.
    pub fn encode_texture(&mut self, bgra: &ID3D11Texture2D, keyframe: bool) -> Result<Vec<u8>> {
        let g = self.gpu.as_mut().ok_or_else(|| fail("GPU 길 아님"))?;
        g.convert(bgra)?;
        let buf = unsafe {
            let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &g.nv12, 0, false)?;
            let len = buf.cast::<IMF2DBuffer>()?.GetContiguousLength()?;
            buf.SetCurrentLength(len)?;
            buf
        };
        self.submit(buf, keyframe)
    }

    fn submit(&mut self, buf: IMFMediaBuffer, keyframe: bool) -> Result<Vec<u8>> {
        let until = Instant::now() + WAIT;
        while self.need == 0 {
            self.next_event(until)?;
        }
        unsafe {
            if keyframe {
                let _ = self
                    .codec
                    .SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1u32));
            }
            let s = MFCreateSample()?;
            s.AddBuffer(&buf)?;
            s.SetSampleTime(self.n * self.frame_100ns)?;
            s.SetSampleDuration(self.frame_100ns)?;
            self.n += 1;
            self.mft.ProcessInput(0, &s, 0)?;
        }
        self.need -= 1;
        loop {
            if self.next_event(until)? == METransformHaveOutput.0 as u32 {
                return self.take_output();
            }
        }
    }

    fn take_output(&mut self) -> Result<Vec<u8>> {
        unsafe {
            let sample = if self.provides {
                None
            } else {
                let s = MFCreateSample()?;
                s.AddBuffer(&MFCreateMemoryBuffer(self.out_size)?)?;
                Some(s)
            };
            let mut out = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(sample),
                dwStatus: 0,
                pEvents: ManuallyDrop::new(None),
            }];
            let mut status = 0;
            let r = self.mft.ProcessOutput(0, &mut out, &mut status);
            let sample = ManuallyDrop::take(&mut out[0].pSample);
            drop(ManuallyDrop::take(&mut out[0].pEvents));
            r?;
            let Some(sample) = sample else {
                return Ok(vec![]);
            };
            let buf = sample.ConvertToContiguousBuffer()?;
            let mut p = std::ptr::null_mut();
            let mut len = 0;
            buf.Lock(&mut p, None, Some(&mut len))?;
            let v = std::slice::from_raw_parts(p, len as usize).to_vec();
            buf.Unlock()?;
            Ok(v)
        }
    }
}

impl Drop for HwH264 {
    fn drop(&mut self) {
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            let _ = MFShutdown();
        }
    }
}

impl Gpu {
    fn new(
        dev: &ID3D11Device,
        mgr: IMFDXGIDeviceManager,
        iw: u32,
        ih: u32,
        ow: u32,
        oh: u32,
        fps: u32,
    ) -> Result<Self> {
        unsafe {
            let vdev: ID3D11VideoDevice = dev.cast()?;
            let vctx: ID3D11VideoContext = dev.GetImmediateContext()?.cast()?;
            let rate = DXGI_RATIONAL {
                Numerator: fps,
                Denominator: 1,
            };
            let cd = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                InputFrameRate: rate,
                InputWidth: iw,
                InputHeight: ih,
                OutputFrameRate: rate,
                OutputWidth: ow,
                OutputHeight: oh,
                Usage: D3D11_VIDEO_USAGE_OPTIMAL_SPEED,
            };
            let venum = vdev.CreateVideoProcessorEnumerator(&cd)?;
            let proc = vdev.CreateVideoProcessor(&venum, 0)?;
            let td = D3D11_TEXTURE2D_DESC {
                Width: ow,
                Height: oh,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut nv12 = None;
            dev.CreateTexture2D(&td, None, Some(&mut nv12))?;
            let nv12 = nv12.ok_or_else(|| fail("NV12 텍스처 없음"))?;
            let od = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            let mut out_view = None;
            vdev.CreateVideoProcessorOutputView(&nv12, &venum, &od, Some(&mut out_view))?;
            Ok(Self {
                out_view: out_view.ok_or_else(|| fail("출력 뷰 없음"))?,
                vdev,
                vctx,
                venum,
                proc,
                nv12,
                in_view: None,
                _mgr: mgr,
            })
        }
    }

    /// BGRA → NV12(그리고 크기 맞추기) 한 번.
    fn convert(&mut self, bgra: &ID3D11Texture2D) -> Result<()> {
        unsafe {
            let key = bgra.as_raw() as usize;
            if self.in_view.as_ref().is_none_or(|v| v.0 != key) {
                let id = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                };
                let mut v = None;
                self.vdev
                    .CreateVideoProcessorInputView(bgra, &self.venum, &id, Some(&mut v))?;
                self.in_view = Some((key, v.ok_or_else(|| fail("입력 뷰 없음"))?));
            }
            let view = self.in_view.as_ref().expect("위에서 채움").1.clone();
            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: TRUE,
                pInputSurface: ManuallyDrop::new(Some(view)),
                ..Default::default()
            };
            let r = self.vctx.VideoProcessorBlt(
                &self.proc,
                &self.out_view,
                0,
                std::slice::from_ref(&stream),
            );
            ManuallyDrop::drop(&mut stream.pInputSurface);
            r
        }
    }
}

/// I420(openh264 YUVBuffer 꼴: Y 다음 U, V) → NV12(Y 다음 UV 교대).
pub fn i420_to_nv12(y: &[u8], u: &[u8], v: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(y.len() + u.len() * 2);
    out.extend_from_slice(y);
    for (a, b) in u.iter().zip(v) {
        out.push(*a);
        out.push(*b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv12_interleaves_chroma() {
        let mut out = vec![];
        i420_to_nv12(&[1, 2, 3, 4], &[5], &[6], &mut out);
        assert_eq!(out, [1, 2, 3, 4, 5, 6]);
    }
}

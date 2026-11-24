//! 캡처 한 장 → 줄이기 → H.264(openh264). 줄이기·크기 계산은 순수 함수로 떼어 시험한다.
#[cfg(windows)]
use crate::platform::encode::{i420_to_nv12, HwH264};
use openh264::encoder::{BitRate, Complexity, Encoder, EncoderConfig, FrameRate, Level, UsageType};
use openh264::formats::{BgraSliceU8, YUVBuffer, YUVSource};
use openh264::OpenH264API;
use tracing::{info, warn};
#[cfg(windows)]
use windows::core::Interface;
#[cfg(windows)]
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};

/// 03 상수 표.
pub const VIDEO_MAX_EDGE: usize = 2560;
pub const VIDEO_FPS: u32 = 30;
pub const VIDEO_BITRATE_KBPS: u32 = 4000;
pub const VIDEO_IDLE_RESEND_MS: u64 = 1000;
pub const VIDEO_ENC_THREADS: u16 = 4;
pub const INPUT_SETTLE_MS: u64 = 20;

/// 보낼 크기 — 긴 변이 VIDEO_MAX_EDGE 이하가 되게 정수배로 줄이고, H.264 4:2:0 이라 짝수로 자른다.
pub fn out_size(w: usize, h: usize) -> (usize, usize, usize) {
    let f = w.max(h).div_ceil(VIDEO_MAX_EDGE).max(1);
    ((w / f) & !1, (h / f) & !1, f)
}

/// f 배 줄이기(가장 가까운 점) + 짝수 자르기.
/// shortcut: 가장 가까운 점이라 5K 의 작은 글자가 거칠다 — 화질이 문제되면 상자 평균으로.
pub fn shrink(bgra: &[u8], w: usize, h: usize) -> (usize, usize, Vec<u8>) {
    let (ow, oh, f) = out_size(w, h);
    if (ow, oh) == (w, h) {
        return (w, h, bgra.to_vec());
    }
    let mut out = Vec::with_capacity(ow * oh * 4);
    for y in 0..oh {
        let row = &bgra[y * f * w * 4..];
        for x in 0..ow {
            out.extend_from_slice(&row[x * f * 4..x * f * 4 + 4]);
        }
    }
    (ow, oh, out)
}

pub struct Video {
    enc: Option<(Encoder, usize, usize)>,
    /// 하드웨어 인코더(먼저 씀). 한 번 실패하면 이 세션은 openh264 로만.
    #[cfg(windows)]
    hw: Option<(HwH264, usize, usize)>,
    hw_off: bool,
    nv12: Vec<u8>,
    /// GPU 길: 캡처 장치와 마지막 한 장(그래픽카드 안 사본, 줄이기 전 크기).
    #[cfg(windows)]
    device: Option<ID3D11Device>,
    #[cfg(windows)]
    tex: Option<(ID3D11Texture2D, usize, usize)>,
    /// GPU 길을 써도 되나(한 번 실패하면 이 세션은 CPU 길). 캡처가 이 값을 따른다.
    pub gpu_ok: bool,
    gpu_failed: bool,
    /// 마지막 화면(줄여 YUV 로 바꾼 것) — 화면이 안 바뀌어도 키프레임·유지 재전송에 쓴다.
    /// 바뀔 때 한 번만 바꿔 둔다 — 다시 보낼 때마다 1080p 를 다시 바꾸면 프레임마다 수십 ms 든다(지연 개선 2026-11-24).
    last: Option<(usize, usize, YUVBuffer)>,
    pub keyframe: bool,
}

impl Video {
    pub fn new() -> Self {
        Self {
            enc: None,
            #[cfg(windows)]
            hw: None,
            hw_off: false,
            nv12: vec![],
            #[cfg(windows)]
            device: None,
            #[cfg(windows)]
            tex: None,
            gpu_ok: true,
            gpu_failed: false,
            last: None,
            keyframe: true,
        }
    }

    /// 캡처 장치가 바뀌면(캡처를 다시 만듦) 그 장치에 묶인 GPU 인코더를 버린다.
    #[cfg(windows)]
    pub fn set_device(&mut self, d: ID3D11Device) {
        if self.device.as_ref().map(|x| x.as_raw()) != Some(d.as_raw()) {
            if self.hw.as_ref().is_some_and(|e| e.0.is_gpu()) {
                self.hw = None;
            }
            self.tex = None;
            self.device = Some(d);
        }
    }

    /// GPU 길의 한 장.
    #[cfg(windows)]
    pub fn set_texture(&mut self, tex: ID3D11Texture2D, w: usize, h: usize) {
        self.tex = Some((tex, w, h));
        self.last = None;
    }

    /// GPU 길이 방금 실패했으면 마지막 한 장을 돌려준다 — 캡처가 CPU 로 읽어 set_frame 하게.
    #[cfg(windows)]
    pub fn fallback_texture(&mut self) -> Option<ID3D11Texture2D> {
        if !std::mem::take(&mut self.gpu_failed) {
            return None;
        }
        self.tex.take().map(|t| t.0)
    }

    pub fn set_frame(&mut self, bgra: &[u8], w: usize, h: usize) {
        #[cfg(windows)]
        {
            self.tex = None;
        }
        // 줄일 일이 없으면 복사하지 않고 캡처 버퍼에서 바로 바꾼다.
        let (ow, oh, _) = out_size(w, h);
        let yuv = if (ow, oh) == (w, h) {
            YUVBuffer::from_bgra8_source(BgraSliceU8::new(bgra, (w, h)))
        } else {
            let (w, h, small) = shrink(bgra, w, h);
            YUVBuffer::from_bgra8_source(BgraSliceU8::new(&small, (w, h)))
        };
        self.last = Some((ow, oh, yuv));
    }

    /// 마지막 화면을 인코딩한다(Annex B). 크기가 바뀌면 인코더를 새로 만든다.
    pub fn encode(&mut self) -> Result<Option<Vec<u8>>, openh264::Error> {
        #[cfg(windows)]
        if let (Some((tex, w, h)), Some(dev)) = (&self.tex, &self.device) {
            return match encode_gpu(&mut self.hw, dev, &mut self.keyframe, tex, *w, *h) {
                Ok(bits) => Ok(bits),
                Err(e) => {
                    warn!(event = "video.gpu_failed", why = %e);
                    self.hw = None;
                    self.gpu_ok = false;
                    self.gpu_failed = true;
                    self.keyframe = true;
                    Ok(None)
                }
            };
        }
        let Some((w, h, yuv)) = &self.last else {
            return Ok(None);
        };
        #[cfg(windows)]
        if !self.hw_off {
            match encode_hw(
                &mut self.hw,
                &mut self.nv12,
                &mut self.keyframe,
                *w,
                *h,
                yuv,
            ) {
                Ok(bits) => return Ok(bits),
                Err(e) => {
                    warn!(event = "video.hw_failed", why = %e);
                    self.hw = None;
                    self.hw_off = true;
                    self.keyframe = true;
                }
            }
        }
        if self.enc.as_ref().is_none_or(|e| (e.1, e.2) != (*w, *h)) {
            let cfg = EncoderConfig::new()
                .bitrate(BitRate::from_bps(VIDEO_BITRATE_KBPS * 1000))
                .max_frame_rate(FrameRate::from_hz(VIDEO_FPS as f32))
                .usage_type(UsageType::ScreenContentRealTime)
                // 가장 빠른 쪽 + 여러 실로 나눠 한 장 인코딩 시간을 줄인다(지연이 화질보다 먼저).
                .complexity(Complexity::Low)
                .num_threads(VIDEO_ENC_THREADS)
                // 비트레이트가 넘쳐도 장을 버리지 않는다 — 버린 장은 다음 장까지(최대 1초) 화면이 늦는다.
                .skip_frames(false)
                .level(Level::Level_5_1); // 03 H264_MAX_LEVEL
            info!(event = "video.encoder", kind = "sw", w, h);
            self.enc = Some((
                Encoder::with_api_config(OpenH264API::from_source(), cfg)?,
                *w,
                *h,
            ));
            self.keyframe = true;
        }
        let enc = &mut self.enc.as_mut().expect("위에서 채움").0;
        if std::mem::take(&mut self.keyframe) {
            enc.force_intra_frame();
        }
        let bits = enc.encode(yuv)?.to_vec();
        Ok((!bits.is_empty()).then_some(bits))
    }
}

/// 하드웨어 인코더로 한 장. 크기가 바뀌면 새로 만든다.
#[cfg(windows)]
fn encode_hw(
    hw: &mut Option<(HwH264, usize, usize)>,
    nv12: &mut Vec<u8>,
    keyframe: &mut bool,
    w: usize,
    h: usize,
    yuv: &YUVBuffer,
) -> windows::core::Result<Option<Vec<u8>>> {
    if hw
        .as_ref()
        .is_none_or(|e| e.0.is_gpu() || (e.1, e.2) != (w, h))
    {
        *hw = None;
        let e = HwH264::new(
            w as u32,
            h as u32,
            VIDEO_FPS,
            VIDEO_BITRATE_KBPS * 1000,
            None,
        )?;
        info!(event = "video.encoder", kind = "hw", name = %e.name, w, h);
        *hw = Some((e, w, h));
        *keyframe = true;
    }
    i420_to_nv12(yuv.y(), yuv.u(), yuv.v(), nv12);
    let enc = &mut hw.as_mut().expect("위에서 채움").0;
    let bits = enc.encode(nv12, std::mem::take(keyframe))?;
    Ok((!bits.is_empty()).then_some(bits))
}

/// GPU 길로 한 장 — 그래픽카드 안의 BGRA(w×h)를 줄여 NV12 로 바꿔 바로 인코딩. 크기가 바뀌면 새로 만든다.
#[cfg(windows)]
fn encode_gpu(
    hw: &mut Option<(HwH264, usize, usize)>,
    dev: &ID3D11Device,
    keyframe: &mut bool,
    tex: &ID3D11Texture2D,
    w: usize,
    h: usize,
) -> windows::core::Result<Option<Vec<u8>>> {
    if hw
        .as_ref()
        .is_none_or(|e| !e.0.is_gpu() || (e.1, e.2) != (w, h))
    {
        *hw = None;
        let (ow, oh, _) = out_size(w, h);
        let gpu = Some((dev, w as u32, h as u32));
        let e = HwH264::new(
            ow as u32,
            oh as u32,
            VIDEO_FPS,
            VIDEO_BITRATE_KBPS * 1000,
            gpu,
        )?;
        info!(event = "video.encoder", kind = "hw_gpu", name = %e.name, w = ow, h = oh);
        *hw = Some((e, w, h));
        *keyframe = true;
    }
    let enc = &mut hw.as_mut().expect("위에서 채움").0;
    let bits = enc.encode_texture(tex, std::mem::take(keyframe))?;
    Ok((!bits.is_empty()).then_some(bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_rules() {
        assert_eq!(out_size(1920, 1080), (1920, 1080, 1));
        assert_eq!(out_size(2560, 1440), (2560, 1440, 1));
        assert_eq!(out_size(5120, 2880), (2560, 1440, 2)); // 5K → 긴 변 2560 (FR-005)
        assert_eq!(out_size(3840, 2160), (1920, 1080, 2));
        assert_eq!(out_size(1366, 767), (1366, 766, 1)); // 홀수는 짝수로
    }

    #[test]
    fn shrink_picks_every_fth_pixel() {
        let px = |x: usize, y: usize| [(x % 251) as u8, y as u8, 0, 255];
        let (w, h) = (5120, 4);
        let src: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| px(x, y)))
            .collect();
        let (ow, oh, out) = shrink(&src, w, h);
        assert_eq!((ow, oh), (2560, 2));
        assert_eq!(&out[0..4], &px(0, 0));
        assert_eq!(&out[4..8], &px(2, 0));
        assert_eq!(&out[ow * 4..ow * 4 + 4], &px(0, 2));
        // 줄일 필요가 없으면 그대로.
        let small = vec![9u8; 4 * 2 * 4];
        assert_eq!(shrink(&small, 4, 2), (4, 2, small.clone()));
    }

    #[test]
    fn encodes_a_frame() {
        let mut v = Video::new();
        let bgra = vec![128u8; 64 * 48 * 4];
        v.set_frame(&bgra, 64, 48);
        let bits = v.encode().unwrap().unwrap();
        assert_eq!(&bits[..4], &[0, 0, 0, 1]); // Annex B
    }
}

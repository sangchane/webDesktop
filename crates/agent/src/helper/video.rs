//! 캡처 한 장 → 줄이기 → H.264(openh264). 줄이기·크기 계산은 순수 함수로 떼어 시험한다.
use openh264::encoder::{BitRate, Encoder, EncoderConfig, FrameRate, Level, UsageType};
use openh264::formats::{BgraSliceU8, YUVBuffer};
use openh264::OpenH264API;

/// 03 상수 표.
pub const VIDEO_MAX_EDGE: usize = 2560;
pub const VIDEO_FPS: u32 = 30;
pub const VIDEO_BITRATE_KBPS: u32 = 4000;
pub const VIDEO_IDLE_RESEND_MS: u64 = 1000;

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
    /// 마지막 화면(줄인 뒤) — 화면이 안 바뀌어도 키프레임·유지 재전송에 쓴다.
    pub last: Option<(usize, usize, Vec<u8>)>,
    pub keyframe: bool,
}

impl Video {
    pub fn new() -> Self {
        Self {
            enc: None,
            last: None,
            keyframe: true,
        }
    }

    pub fn set_frame(&mut self, bgra: &[u8], w: usize, h: usize) {
        self.last = Some(shrink(bgra, w, h));
    }

    /// 마지막 화면을 인코딩한다(Annex B). 크기가 바뀌면 인코더를 새로 만든다.
    pub fn encode(&mut self) -> Result<Option<Vec<u8>>, openh264::Error> {
        let Some((w, h, bgra)) = &self.last else {
            return Ok(None);
        };
        if self.enc.as_ref().is_none_or(|e| (e.1, e.2) != (*w, *h)) {
            let cfg = EncoderConfig::new()
                .bitrate(BitRate::from_bps(VIDEO_BITRATE_KBPS * 1000))
                .max_frame_rate(FrameRate::from_hz(VIDEO_FPS as f32))
                .usage_type(UsageType::ScreenContentRealTime)
                .level(Level::Level_5_1); // 03 H264_MAX_LEVEL
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
        let yuv = YUVBuffer::from_bgra8_source(BgraSliceU8::new(bgra, (*w, *h)));
        let bits = enc.encode(&yuv)?.to_vec();
        Ok((!bits.is_empty()).then_some(bits))
    }
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

//! SC-003 입력 지연 (06 「SC-003 측정법」) — 뷰어 시계 하나.
//! 입력을 보낸 시각 → 도우미가 알려 준 반영 프레임의 RTP 타임스탬프(D-08) → 그 프레임이 그려진 시각.
//! 프레임이 먼저 그려지고 D-08 이 늦게 올 수도 있어 양쪽을 잠깐 기억한다.
use proto::Input;
use std::collections::VecDeque;

/// 기억하는 칸 수 — 60fps 로 약 4초.
const KEEP: usize = 256;

fn push(q: &mut VecDeque<(u32, f64)>, v: (u32, f64)) {
    if q.len() == KEEP {
        q.pop_front();
    }
    q.push_back(v);
}

fn take(q: &mut VecDeque<(u32, f64)>, k: u32) -> Option<f64> {
    let i = q.iter().position(|e| e.0 == k)?;
    q.remove(i).map(|e| e.1)
}

/// 재는 입력 — 화면이 바뀌는 누름·키·글자만(이동은 너무 잦다).
pub fn timed(i: &Input) -> Option<u32> {
    match i {
        Input::MouseButton {
            seq, down: true, ..
        }
        | Input::Key {
            seq, down: true, ..
        }
        | Input::Text { seq, .. } => Some(*seq),
        _ => None,
    }
}

#[derive(Default)]
pub struct Meter {
    sent: VecDeque<(u32, f64)>,
    /// (rtp_ts, 보낸 시각) — D-08 은 왔고 프레임은 아직.
    waiting: VecDeque<(u32, f64)>,
    /// (rtp_ts, 그린 시각) — 그려졌고 D-08 은 아직.
    drawn: VecDeque<(u32, f64)>,
    samples: Vec<f64>,
}

impl Meter {
    pub fn sent(&mut self, seq: u32, t: f64) {
        push(&mut self.sent, (seq, t));
    }

    pub fn applied(&mut self, seq: u32, rtp_ts: u32) {
        let Some(t0) = take(&mut self.sent, seq) else {
            return;
        };
        // 그린 프레임은 남겨 둔다 — 여러 입력이 같은 프레임에 반영될 수 있다.
        // 보낸 뒤에 그린 프레임만 — RTP 타임스탬프가 돌아 같은 값이 다시 나와도 옛 프레임과 짝짓지 않는다.
        match self.drawn.iter().find(|e| e.0 == rtp_ts && e.1 >= t0) {
            Some(&(_, t1)) => self.samples.push(t1 - t0),
            None => push(&mut self.waiting, (rtp_ts, t0)),
        }
    }

    pub fn drawn(&mut self, rtp_ts: u32, t: f64) {
        while let Some(t0) = take(&mut self.waiting, rtp_ts) {
            self.samples.push(t - t0);
        }
        push(&mut self.drawn, (rtp_ts, t));
    }

    /// 세션 전체의 가운데 값(ms).
    pub fn p50(&self) -> Option<u32> {
        let mut v = self.samples.clone();
        v.sort_by(f64::total_cmp);
        v.get(v.len().checked_sub(1)? / 2).map(|x| x.round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_in_either_order() {
        let mut m = Meter::default();
        assert_eq!(m.p50(), None);
        // D-08 먼저, 프레임 나중.
        m.sent(1, 1000.0);
        m.applied(1, 9000);
        m.drawn(9000, 1120.0);
        // 프레임 먼저, D-08 나중.
        m.sent(2, 2000.0);
        m.drawn(18000, 2090.0);
        m.applied(2, 18000);
        // 모르는 순번·남의 프레임은 무시.
        m.applied(99, 1);
        m.drawn(5, 3000.0);
        m.sent(3, 3000.0);
        m.applied(3, 27000);
        m.drawn(27000, 3200.0);
        assert_eq!(m.p50(), Some(120));
    }

    #[test]
    fn several_inputs_in_one_frame_all_count() {
        let mut m = Meter::default();
        m.sent(1, 1000.0);
        m.sent(2, 1050.0);
        m.applied(1, 9000);
        m.applied(2, 9000);
        m.drawn(9000, 1100.0);
        m.sent(3, 2000.0);
        m.applied(3, 9000); // 보내기 전에 그린 프레임과는 짝짓지 않는다.
        assert_eq!(m.samples, [100.0, 50.0]);
        m.sent(4, 3000.0);
        m.drawn(18000, 3040.0);
        m.applied(4, 18000);
        m.applied(5, 18000); // 모르는 순번
        assert_eq!(m.samples, [100.0, 50.0, 40.0]);
    }

    #[test]
    fn only_presses_are_timed() {
        let press = Input::MouseButton {
            seq: 4,
            button: proto::MouseButton::Left,
            down: true,
        };
        let release = Input::MouseButton {
            seq: 5,
            button: proto::MouseButton::Left,
            down: false,
        };
        assert_eq!(timed(&press), Some(4));
        assert_eq!(timed(&release), None);
        assert_eq!(
            timed(&Input::MouseMove {
                seq: 6,
                x: 0.0,
                y: 0.0
            }),
            None
        );
    }
}

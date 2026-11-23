//! 원격 화면 확대·이동 — 영상에 `translate(tx, ty) scale(s)`(기준점 왼쪽 위)를 건다. 좌표는 무대(stage) 안 px.
//! 순수 셈이라 native `cargo test -p web` 로 시험한다.

/// 03 상수 표.
pub const ZOOM_MAX: f64 = 6.0;
pub const PAN_EDGE_PCT: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    pub s: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            s: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

impl Zoom {
    pub fn zoomed(&self) -> bool {
        self.s > 1.001
    }

    pub fn css(&self) -> String {
        format!(
            "transform: translate({}px, {}px) scale({})",
            self.tx, self.ty, self.s
        )
    }

    /// 무대 밖 빈 곳이 보이지 않게 가둔다. 바뀌었으면 true.
    pub fn clamp(&mut self, (w, h): (f64, f64)) -> bool {
        let old = *self;
        self.s = self.s.clamp(1.0, ZOOM_MAX);
        self.tx = self.tx.clamp((w - w * self.s).min(0.0), 0.0);
        self.ty = self.ty.clamp((h - h * self.s).min(0.0), 0.0);
        *self != old
    }

    /// 집기: `center` 아래 내용을 손가락에 붙인 채 `factor` 배 하고 `pan` 만큼 옮긴다.
    pub fn pinch(&mut self, factor: f64, center: (f64, f64), pan: (f64, f64), size: (f64, f64)) {
        let s = (self.s * factor).clamp(1.0, ZOOM_MAX);
        let k = s / self.s;
        self.tx = center.0 - (center.0 - self.tx) * k + pan.0;
        self.ty = center.1 - (center.1 - self.ty) * k + pan.1;
        self.s = s;
        self.clamp(size);
    }

    /// 커서(무대 px)가 가장자리 PAN_EDGE_PCT 안에 들면 화면을 그만큼 민다.
    pub fn follow(&mut self, p: (f64, f64), (w, h): (f64, f64)) {
        let (mx, my) = (w * PAN_EDGE_PCT / 100.0, h * PAN_EDGE_PCT / 100.0);
        let push = |v: f64, m: f64, len: f64| {
            if v < m {
                m - v
            } else if v > len - m {
                len - m - v
            } else {
                0.0
            }
        };
        self.tx += push(p.0, mx, w);
        self.ty += push(p.1, my, h);
        self.clamp((w, h));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SIZE: (f64, f64) = (400.0, 300.0);

    #[test]
    fn pinch_keeps_point_under_fingers_and_clamps() {
        let mut z = Zoom::default();
        z.pinch(2.0, (200.0, 150.0), (0.0, 0.0), SIZE);
        assert_eq!(
            z,
            Zoom {
                s: 2.0,
                tx: -200.0,
                ty: -150.0
            }
        );
        // 가운데 아래 내용(200,150)은 그대로 가운데: -200 + 200*2 = 200.
        assert_eq!(z.tx + 200.0 * z.s, 200.0);
        // 넘치게 밀어도 가장자리에서 멈춘다.
        z.pinch(1.0, (0.0, 0.0), (1000.0, -1000.0), SIZE);
        assert_eq!((z.tx, z.ty), (0.0, -300.0));
        // 상한과 하한.
        z.pinch(100.0, (0.0, 0.0), (0.0, 0.0), SIZE);
        assert_eq!(z.s, ZOOM_MAX);
        z.pinch(0.0001, (0.0, 0.0), (0.0, 0.0), SIZE);
        assert_eq!(z, Zoom::default());
    }

    #[test]
    fn follow_moves_only_when_cursor_near_edge() {
        let mut z = Zoom {
            s: 2.0,
            tx: -200.0,
            ty: -150.0,
        };
        z.follow((200.0, 150.0), SIZE);
        assert_eq!((z.tx, z.ty), (-200.0, -150.0));
        // 오른쪽 가장자리 60px(15%) 안, 400-60-390 = -50 만큼 민다.
        z.follow((390.0, 150.0), SIZE);
        assert_eq!(z.tx, -250.0);
        // 확대 안 했으면 움직이지 않는다.
        let mut z = Zoom::default();
        z.follow((0.0, 0.0), SIZE);
        assert_eq!(z, Zoom::default());
    }

    #[test]
    fn smaller_stage_reclamps() {
        let mut z = Zoom {
            s: 2.0,
            tx: -400.0,
            ty: -300.0,
        };
        assert!(z.clamp((400.0, 100.0)));
        assert_eq!((z.tx, z.ty), (-400.0, -100.0));
        assert!(!z.clamp((400.0, 100.0)));
    }
}

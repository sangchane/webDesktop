//! 입력 변환기 — 브라우저 사건 → `proto::Input` (D-01·D-02·D-03, FR-006·007).
//! 브라우저를 모르는 순수 코드라 native `cargo test -p web` 로 시험한다.
use proto::{Input, MouseButton, INPUT_TEXT_MAX};

/// 03 상수 표 — 탭으로 보는 최대 시간·이동, 트랙패드 감도, 길게 누르기, 두 번 탭 간격.
pub const TAP_MAX_MS: f64 = 250.0;
pub const TAP_SLOP_PX: f64 = 10.0;
pub const TRACKPAD_GAIN: f64 = 1.5;
pub const LONG_PRESS_MS: f64 = 500.0;
pub const DOUBLE_TAP_MS: f64 = 300.0;

/// USB HID 키보드 페이지(0x07) 번호.
pub const HID_ENTER: u16 = 0x28;
pub const HID_ESC: u16 = 0x29;
pub const HID_BACKSPACE: u16 = 0x2A;
pub const HID_TAB: u16 = 0x2B;
pub const HID_DELETE: u16 = 0x4C;
pub const HID_CTRL: u16 = 0xE0;
pub const HID_SHIFT: u16 = 0xE1;
pub const HID_ALT: u16 = 0xE2;
pub const HID_WIN: u16 = 0xE3;

/// `KeyboardEvent.code` → HID. 한/영·한자 키(Lang1·2)는 없다 — 한글은 뷰어 IME 가 조합해 글자로 보낸다.
pub fn hid_usage(code: &str) -> Option<u16> {
    let b = code.as_bytes();
    if code.len() == 4 && code.starts_with("Key") && b[3].is_ascii_uppercase() {
        return Some(0x04 + (b[3] - b'A') as u16);
    }
    if code.len() == 6 && code.starts_with("Digit") && b[5].is_ascii_digit() {
        return Some(match b[5] {
            b'0' => 0x27,
            d => 0x1E + (d - b'1') as u16,
        });
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=12).contains(&n).then_some(0x3A + n - 1);
    }
    Some(match code {
        "Enter" | "NumpadEnter" => HID_ENTER,
        "Escape" => HID_ESC,
        "Backspace" => HID_BACKSPACE,
        "Tab" => HID_TAB,
        "Space" => 0x2C,
        "Minus" => 0x2D,
        "Equal" => 0x2E,
        "BracketLeft" => 0x2F,
        "BracketRight" => 0x30,
        "Backslash" => 0x31,
        "Semicolon" => 0x33,
        "Quote" => 0x34,
        "Backquote" => 0x35,
        "Comma" => 0x36,
        "Period" => 0x37,
        "Slash" => 0x38,
        "CapsLock" => 0x39,
        "PrintScreen" => 0x46,
        "ScrollLock" => 0x47,
        "Pause" => 0x48,
        "Insert" => 0x49,
        "Home" => 0x4A,
        "PageUp" => 0x4B,
        "Delete" => HID_DELETE,
        "End" => 0x4D,
        "PageDown" => 0x4E,
        "ArrowRight" => 0x4F,
        "ArrowLeft" => 0x50,
        "ArrowDown" => 0x51,
        "ArrowUp" => 0x52,
        "IntlBackslash" => 0x64,
        "ContextMenu" => 0x65,
        "ControlLeft" => HID_CTRL,
        "ShiftLeft" => HID_SHIFT,
        "AltLeft" => HID_ALT,
        "MetaLeft" => HID_WIN,
        "ControlRight" => 0xE4,
        "ShiftRight" => 0xE5,
        "AltRight" => 0xE6,
        "MetaRight" => 0xE7,
        _ => return None,
    })
}

/// `KeyboardEvent.key` 이름 → HID. 소프트 자판은 `code` 를 비워 보내기도 한다(Android 확실, iOS 확인 필요).
fn key_name_hid(key: &str) -> Option<u16> {
    Some(match key {
        "Enter" => HID_ENTER,
        "Escape" => HID_ESC,
        "Backspace" => HID_BACKSPACE,
        "Tab" => HID_TAB,
        "Delete" => HID_DELETE,
        "Home" => 0x4A,
        "PageUp" => 0x4B,
        "End" => 0x4D,
        "PageDown" => 0x4E,
        "ArrowRight" => 0x4F,
        "ArrowLeft" => 0x50,
        "ArrowDown" => 0x51,
        "ArrowUp" => 0x52,
        _ => return None,
    })
}

/// keydown·keyup 의 HID — `code` 가 비었으면 `key` 이름으로.
pub fn event_hid(key: &str, code: &str) -> Option<u16> {
    hid_usage(code).or_else(|| key_name_hid(key))
}

/// 영문·숫자 글자 → HID(수정키를 잡은 채 친 글자를 단축키로 보낼 때).
fn char_hid(c: char) -> Option<u16> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(0x04 + (c as u8 - b'a') as u16),
        '0' => Some(0x27),
        c @ '1'..='9' => Some(0x1E + (c as u8 - b'1') as u16),
        _ => None,
    }
}

/// keydown 을 키(D-02)로 보낼지. 글자 키는 숨은 입력칸의 글자(D-03)로 간다.
/// `buffer_empty`: 입력칸이 비었나 — 비지 않았으면 Backspace 는 입력칸 비교가 맡는다(두 번 지우지 않게).
pub fn key_to_send(
    key: &str,
    code: &str,
    modded: bool,
    composing: bool,
    buffer_empty: bool,
) -> Option<u16> {
    // 한/영·한자 키는 뷰어 IME 가 쓴다 — 오른쪽 Alt 자리(code AltRight)로 와도 보내지 않는다.
    if composing
        || matches!(
            key,
            "Process" | "Unidentified" | "Dead" | "HangulMode" | "HanjaMode" | "KanaMode"
        )
    {
        return None;
    }
    let hid = event_hid(key, code)?;
    if hid == HID_BACKSPACE && !buffer_empty && !modded {
        return None;
    }
    // "Enter"·"ArrowLeft"·"Shift" 처럼 이름이 긴 키는 글자를 만들지 않는다.
    (key.chars().count() > 1 || modded).then_some(hid)
}

/// 요소 안 좌표(px) → 영상 기준 0~1. `object-fit: contain` 의 검은 띠 위면 None.
pub fn to_unit(px: f64, py: f64, ew: f64, eh: f64, vw: f64, vh: f64) -> Option<(f32, f32)> {
    let (x0, y0, cw, ch) = content_rect(ew, eh, vw, vh)?;
    let (x, y) = ((px - x0) / cw, (py - y0) / ch);
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some((x as f32, y as f32))
}

/// 0~1 → 요소 안 좌표(px). 트랙패드 커서 표시용.
pub fn from_unit(u: (f64, f64), ew: f64, eh: f64, vw: f64, vh: f64) -> Option<(f64, f64)> {
    let (x0, y0, cw, ch) = content_rect(ew, eh, vw, vh)?;
    Some((x0 + u.0 * cw, y0 + u.1 * ch))
}

fn content_rect(ew: f64, eh: f64, vw: f64, vh: f64) -> Option<(f64, f64, f64, f64)> {
    if ew <= 0.0 || eh <= 0.0 || vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    let s = (ew / vw).min(eh / vh);
    let (cw, ch) = (vw * s, vh * s);
    Some(((ew - cw) / 2.0, (eh - ch) / 2.0, cw, ch))
}

fn centroid(pts: &[(f64, f64)]) -> (f64, f64) {
    let n = pts.len().max(1) as f64;
    let (x, y) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    (x / n, y / n)
}

/// 두 손가락 사이 거리(집기 판정). 한 손가락이면 0.
fn spread(pts: &[(f64, f64)]) -> f64 {
    match pts {
        [a, b, ..] => (a.0 - b.0).hypot(a.1 - b.1),
        _ => 0.0,
    }
}

/// 무대(stage) 안 영상 자리 — 영상 요소의 왼쪽 위·크기(확대 반영)와 영상 원본 크기. 손가락 좌표는 무대 px.
#[derive(Clone, Copy, Default, Debug)]
pub struct Geo {
    pub x0: f64,
    pub y0: f64,
    pub ew: f64,
    pub eh: f64,
    pub vw: f64,
    pub vh: f64,
}

impl Geo {
    pub fn unit(&self, p: (f64, f64)) -> Option<(f32, f32)> {
        to_unit(
            p.0 - self.x0,
            p.1 - self.y0,
            self.ew,
            self.eh,
            self.vw,
            self.vh,
        )
    }

    /// 0~1 → 무대 px(커서 그리기·따라가기).
    pub fn stage(&self, u: (f64, f64)) -> Option<(f64, f64)> {
        from_unit(u, self.ew, self.eh, self.vw, self.vh).map(|(x, y)| (x + self.x0, y + self.y0))
    }

    /// 화면에 보이는 영상 크기(px, 검은 띠 뺌).
    fn content(&self) -> (f64, f64) {
        content_rect(self.ew, self.eh, self.vw, self.vh).map_or((0.0, 0.0), |r| (r.2, r.3))
    }
}

/// 집기 한 걸음 — 화면 확대(`zoom::Zoom::pinch`)에 넘긴다. 호스트에는 아무것도 안 간다.
#[derive(Debug, PartialEq)]
pub struct Pinch {
    pub factor: f64,
    pub center: (f64, f64),
    pub pan: (f64, f64),
}

/// 두 손가락 손짓 — 처음 움직임으로 정하고 손을 뗄 때까지 유지한다(스크롤하다 확대되지 않게).
#[derive(Clone, Copy, Default, PartialEq)]
enum Two {
    #[default]
    Undecided,
    Scroll,
    Pinch,
}

/// 탭 하나 — (시각, 무대 px, 보낸 0~1 좌표).
type Tap = (f64, (f64, f64), (f32, f32));

/// 뷰어 하나의 입력 상태 — 순번·터치 손짓·입력칸 비교·잡아 둔 수정키.
#[derive(Default)]
pub struct Converter {
    seq: u32,
    /// 트랙패드 커서(0~1). 터치는 상대 이동이라 뷰어가 커서 위치를 갖고 있다.
    pub cursor: (f64, f64),
    /// 직접 터치(누른 곳 클릭) — 아니면 트랙패드.
    pub direct: bool,
    fingers: usize,
    fingers_max: usize,
    anchor: (f64, f64),
    spread: f64,
    /// 첫 손가락이 닿은 곳(무대 px).
    start: (f64, f64),
    started_ms: f64,
    past_slop: bool,
    two: Two,
    /// 트랙패드: 탭 바로 뒤 다시 닿음 → 움직이면 끌기.
    tap_then_touch: bool,
    /// 왼쪽 버튼을 누른 채 끄는 중.
    dragging: bool,
    /// 마지막 왼쪽 탭(시각, 무대 px, 보낸 좌표) — 두 번 탭을 같은 자리로(더블클릭).
    last_tap: Option<Tap>,
    /// 입력칸에서 이미 보낸 글.
    sent: String,
    latched: Vec<u16>,
}

impl Converter {
    /// 커서는 화면 가운데에서 시작한다.
    pub fn new() -> Self {
        Self {
            cursor: (0.5, 0.5),
            ..Default::default()
        }
    }

    fn next(&mut self) -> u32 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    fn key_ev(&mut self, hid_usage: u16, down: bool) -> Input {
        Input::Key {
            seq: self.next(),
            hid_usage,
            down,
        }
    }

    fn tap(&mut self, hid: u16) -> [Input; 2] {
        [self.key_ev(hid, true), self.key_ev(hid, false)]
    }

    /// 잡아 둔 수정키를 놓는다 — 다음 키 하나에만 걸린다(휴대폰 특수키 막대).
    fn release(&mut self) -> Vec<Input> {
        std::mem::take(&mut self.latched)
            .into_iter()
            .rev()
            .map(|h| self.key_ev(h, false))
            .collect()
    }

    pub fn latched(&self) -> &[u16] {
        &self.latched
    }

    pub fn mouse_move(&mut self, x: f32, y: f32) -> Input {
        self.cursor = (x as f64, y as f64);
        Input::MouseMove {
            seq: self.next(),
            x,
            y,
        }
    }

    pub fn mouse_button(&mut self, button: MouseButton, down: bool) -> Vec<Input> {
        let mut out = vec![Input::MouseButton {
            seq: self.next(),
            button,
            down,
        }];
        if !down {
            out.extend(self.release());
        }
        out
    }

    pub fn wheel(&mut self, dx: f32, dy: f32) -> Input {
        Input::Wheel {
            seq: self.next(),
            dx,
            dy,
        }
    }

    /// 실제 키보드의 키.
    pub fn key(&mut self, hid: u16, down: bool) -> Vec<Input> {
        let mut out = vec![self.key_ev(hid, down)];
        if !down && !(0xE0..=0xE7).contains(&hid) {
            out.extend(self.release());
        }
        out
    }

    /// 특수키 막대의 Esc·Tab·Del — 누르고 뗀다.
    pub fn tap_key(&mut self, hid: u16) -> Vec<Input> {
        let mut out = self.tap(hid).to_vec();
        out.extend(self.release());
        out
    }

    /// 특수키 막대의 Ctrl·Alt·Win — 한 번 누르면 잡고, 다시 누르면 놓는다.
    pub fn toggle_mod(&mut self, hid: u16) -> Vec<Input> {
        if let Some(i) = self.latched.iter().position(|&h| h == hid) {
            self.latched.remove(i);
            vec![self.key_ev(hid, false)]
        } else {
            self.latched.push(hid);
            vec![self.key_ev(hid, true)]
        }
    }

    /// 숨은 입력칸의 지금 값(조합이 끝난 때만 부른다). 보낸 글과 비교해 지운 만큼 Backspace, 더한 만큼 글자.
    /// 같은 값이 두 번 와도(조합 끝 사건 + input 사건) 두 번 보내지 않는다.
    pub fn text(&mut self, value: &str) -> Vec<Input> {
        let common = self
            .sent
            .chars()
            .zip(value.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let del = self.sent.chars().count() - common;
        let add: Vec<char> = value.chars().skip(common).collect();
        self.sent = value.to_owned();
        let mut out = Vec::new();
        for _ in 0..del {
            out.extend(self.tap(HID_BACKSPACE));
        }
        // Enter 가 키로 안 오고 줄바꿈 글자로 들어온 때(소프트 자판·조합 중 Enter) — Enter 키로 바꾼다.
        // 부른 쪽은 값에 줄바꿈이 있으면 입력칸을 비운다.
        if let Some(i) = add.iter().position(|&c| c == '\n') {
            let (before, after) = add.split_at(i);
            out.extend(self.chars(before));
            out.extend(self.tap(HID_ENTER));
            out.extend(self.chars(&after[1..]));
            return out;
        }
        out.extend(self.chars(&add));
        out
    }

    fn chars(&mut self, add: &[char]) -> Vec<Input> {
        let mut out = Vec::new();
        if add.is_empty() {
            return out;
        }
        if !self.latched.is_empty() {
            // Ctrl+C 처럼 — 글자 대신 키로.
            for c in add {
                match char_hid(*c) {
                    Some(h) => out.extend(self.tap(h)),
                    None => out.push(Input::Text {
                        seq: self.next(),
                        text: c.to_string(),
                    }),
                }
            }
            out.extend(self.release());
            return out;
        }
        for chunk in add.chunks(INPUT_TEXT_MAX).filter(|c| !c.contains(&'\n')) {
            out.push(Input::Text {
                seq: self.next(),
                text: chunk.iter().collect(),
            });
        }
        out
    }

    /// 입력칸을 비웠을 때(Enter 뒤·길어졌을 때).
    pub fn text_reset(&mut self) {
        self.sent.clear();
    }

    /// 손가락 수가 바뀔 때마다 부른다(touchstart, 손가락이 남은 touchend). 좌표는 무대 px.
    pub fn touch_start(&mut self, pts: &[(f64, f64)], t_ms: f64) {
        if self.fingers == 0 {
            self.started_ms = t_ms;
            self.past_slop = false;
            self.fingers_max = 0;
            self.two = Two::Undecided;
            self.start = centroid(pts);
            self.tap_then_touch = !self.direct
                && self
                    .last_tap
                    .is_some_and(|(t, ..)| t_ms - t <= DOUBLE_TAP_MS);
        }
        if pts.len() != self.fingers {
            // 손가락 수가 바뀌면 두 손가락 손짓을 다시 정한다(한 손가락 스크롤 뒤 집기).
            self.two = Two::Undecided;
        }
        self.fingers = pts.len();
        self.fingers_max = self.fingers_max.max(pts.len());
        self.anchor = centroid(pts);
        self.spread = spread(pts);
    }

    /// 트랙패드: 한 손가락 = 커서 이동(상대), 탭 뒤 끌기 = 왼쪽 끌기, 두 손가락 = 스크롤 또는 집기.
    /// 직접 터치: 한 손가락 = 스크롤, 길게 누른 뒤 끌기 = 왼쪽 끌기, 두 손가락 = 집기.
    pub fn touch_move(
        &mut self,
        pts: &[(f64, f64)],
        geo: &Geo,
        t_ms: f64,
    ) -> (Vec<Input>, Option<Pinch>) {
        if pts.len() != self.fingers {
            self.touch_start(pts, self.started_ms);
            return (vec![], None);
        }
        let c = centroid(pts);
        let sp = spread(pts);
        let (dx, dy) = (c.0 - self.anchor.0, c.1 - self.anchor.1);
        let mut out = vec![];
        // 두 손가락 손짓 끝에 남은 손가락은 아무것도 하지 않는다(집기 뒤 커서가 튀거나 스크롤되지 않게).
        if self.fingers == 1 && self.fingers_max >= 2 {
            self.anchor = c;
            return (out, None);
        }
        let grow = (sp - self.spread).abs();
        let still = dx.hypot(dy) <= TAP_SLOP_PX && grow <= TAP_SLOP_PX;
        if self.fingers == 2 && self.two == Two::Undecided {
            if still {
                return (out, None);
            }
            self.two = if self.direct || grow > dx.hypot(dy) {
                Two::Pinch
            } else {
                Two::Scroll
            };
        }
        if !self.past_slop {
            if still {
                return (out, None);
            }
            self.past_slop = true;
            if self.fingers == 1 {
                let at = if self.tap_then_touch {
                    Some((self.cursor.0 as f32, self.cursor.1 as f32))
                } else if self.direct && t_ms - self.started_ms >= LONG_PRESS_MS {
                    geo.unit(self.start)
                } else {
                    None
                };
                if let Some((x, y)) = at {
                    self.dragging = true;
                    out.push(self.mouse_move(x, y));
                    out.extend(self.mouse_button(MouseButton::Left, true));
                }
            }
        }
        let (prev, prev_sp) = (self.anchor, self.spread);
        self.anchor = c;
        self.spread = sp;
        match (self.fingers, self.direct) {
            (1, true) if self.dragging => {
                if let Some((x, y)) = geo.unit(c) {
                    out.push(self.mouse_move(x, y));
                }
            }
            // 손가락을 올리면 아래 내용이 올라온다(터치 화면 스크롤).
            (1, true) => out.push(self.wheel(-dx as f32, -dy as f32)),
            (1, false) => {
                let (vw, vh) = geo.content();
                if vw > 0.0 && vh > 0.0 {
                    let x = (self.cursor.0 + dx * TRACKPAD_GAIN / vw).clamp(0.0, 1.0);
                    let y = (self.cursor.1 + dy * TRACKPAD_GAIN / vh).clamp(0.0, 1.0);
                    out.push(self.mouse_move(x as f32, y as f32));
                }
            }
            (2, _) if self.two == Two::Pinch => {
                let factor = if prev_sp > 0.0 { sp / prev_sp } else { 1.0 };
                let pinch = Pinch {
                    factor,
                    center: prev,
                    pan: (dx, dy),
                };
                return (out, Some(pinch));
            }
            (2, _) => out.push(self.wheel(-dx as f32, -dy as f32)),
            _ => {}
        }
        (out, None)
    }

    /// touchcancel(OS 손짓·손바닥 거름) — 탭으로 보지 않고 끌기만 놓는다.
    pub fn touch_cancel(&mut self) -> Vec<Input> {
        self.fingers = 0;
        self.past_slop = true;
        self.release_all()
    }

    /// 누른 채인 끌기와 잡아 둔 수정키를 모두 놓는다(손짓 취소·화면을 떠날 때).
    pub fn release_all(&mut self) -> Vec<Input> {
        let mut out = vec![];
        if std::mem::take(&mut self.dragging) {
            out.push(Input::MouseButton {
                seq: self.next(),
                button: MouseButton::Left,
                down: false,
            });
        }
        out.extend(self.release());
        out
    }

    /// touchend. `left` = 아직 닿은 손가락. 모두 떨어졌을 때 탭·길게 누르기·끌기 끝을 판정한다.
    pub fn touch_end(&mut self, left: &[(f64, f64)], geo: &Geo, t_ms: f64) -> Vec<Input> {
        if !left.is_empty() {
            self.fingers = left.len();
            self.anchor = centroid(left);
            self.spread = spread(left);
            return vec![];
        }
        self.fingers = 0;
        if self.dragging {
            self.dragging = false;
            self.last_tap = None;
            return self.mouse_button(MouseButton::Left, false);
        }
        let held = t_ms - self.started_ms;
        let long = self.direct && self.fingers_max == 1 && held >= LONG_PRESS_MS;
        if self.past_slop || (held > TAP_MAX_MS && !long) {
            return vec![];
        }
        let button = match self.fingers_max {
            1 if long => MouseButton::Right,
            1 => MouseButton::Left,
            2 => MouseButton::Right,
            _ => return vec![],
        };
        let at = if self.direct {
            // 두 번째 탭이 첫 탭 가까이면 같은 자리로 — 폰 배율에선 손가락 3px 가 PC 15px 라 더블클릭이 안 된다.
            let again = self.last_tap.filter(|(t, p, _)| {
                button == MouseButton::Left
                    && t_ms - t <= DOUBLE_TAP_MS
                    && (p.0 - self.start.0).hypot(p.1 - self.start.1) <= TAP_SLOP_PX * 2.0
            });
            match again.map(|a| a.2).or_else(|| geo.unit(self.start)) {
                Some(u) => u,
                None => return vec![],
            }
        } else {
            // 호스트 커서가 다른 곳에 있을 수 있어 뷰어가 보여 준 커서 자리로 먼저 옮긴다.
            (self.cursor.0 as f32, self.cursor.1 as f32)
        };
        self.last_tap = (button == MouseButton::Left).then_some((t_ms, self.start, at));
        let mut out = vec![self.mouse_move(at.0, at.1)];
        out.extend(self.mouse_button(button, true));
        out.extend(self.mouse_button(button, false));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &[Input]) -> Vec<(u16, bool)> {
        v.iter()
            .filter_map(|i| match i {
                Input::Key {
                    hid_usage, down, ..
                } => Some((*hid_usage, *down)),
                _ => None,
            })
            .collect()
    }

    fn texts(v: &[Input]) -> Vec<String> {
        v.iter()
            .filter_map(|i| match i {
                Input::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hid_table_matches_usb_spec() {
        assert_eq!(hid_usage("KeyA"), Some(0x04));
        assert_eq!(hid_usage("KeyZ"), Some(0x1D));
        assert_eq!(hid_usage("Digit1"), Some(0x1E));
        assert_eq!(hid_usage("Digit0"), Some(0x27));
        assert_eq!(hid_usage("F1"), Some(0x3A));
        assert_eq!(hid_usage("F12"), Some(0x45));
        assert_eq!(hid_usage("F13"), None);
        assert_eq!(hid_usage("Lang1"), None);
        assert_eq!(hid_usage("MetaLeft"), Some(HID_WIN));
    }

    #[test]
    fn printable_keys_go_as_text_named_keys_as_keys() {
        assert_eq!(key_to_send("a", "KeyA", false, false, true), None);
        assert_eq!(key_to_send("c", "KeyC", true, false, true), Some(0x06));
        assert_eq!(
            key_to_send("Enter", "Enter", false, false, true),
            Some(HID_ENTER)
        );
        assert_eq!(key_to_send("Process", "KeyA", false, true, true), None);
        assert_eq!(
            key_to_send("ArrowLeft", "ArrowLeft", false, true, true),
            None
        );
        assert_eq!(key_to_send("HangulMode", "Lang1", false, false, true), None);
        // 입력칸에 글이 있으면 Backspace 는 입력칸 비교가 맡는다.
        assert_eq!(
            key_to_send("Backspace", "Backspace", false, false, false),
            None
        );
        assert_eq!(
            key_to_send("Backspace", "Backspace", false, false, true),
            Some(HID_BACKSPACE)
        );
    }

    #[test]
    fn letterbox_maps_to_unit_and_back() {
        // 1000x1000 요소에 16:9 영상 → 위아래 검은 띠.
        assert_eq!(to_unit(500.0, 10.0, 1000.0, 1000.0, 1920.0, 1080.0), None);
        let (x, y) = to_unit(0.0, 500.0, 1000.0, 1000.0, 1920.0, 1080.0).unwrap();
        assert!((x - 0.0).abs() < 1e-6 && (y - 0.5).abs() < 1e-6);
        let p = from_unit((1.0, 1.0), 1000.0, 1000.0, 1920.0, 1080.0).unwrap();
        assert!((p.0 - 1000.0).abs() < 1e-6 && (p.1 - 781.25).abs() < 1e-6);
        assert_eq!(to_unit(1.0, 1.0, 0.0, 10.0, 1920.0, 1080.0), None);
    }

    /// 영상이 무대를 꽉 채운 자리(검은 띠 없음).
    fn geo(w: f64, h: f64) -> Geo {
        Geo {
            ew: w,
            eh: h,
            vw: w,
            vh: h,
            ..Default::default()
        }
    }

    fn buttons(v: &[Input]) -> Vec<(MouseButton, bool)> {
        v.iter()
            .filter_map(|i| match i {
                Input::MouseButton { button, down, .. } => Some((*button, *down)),
                _ => None,
            })
            .collect()
    }

    fn last_move(v: &[Input]) -> (f32, f32) {
        v.iter()
            .rev()
            .find_map(|i| match i {
                Input::MouseMove { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .unwrap()
    }

    const G: Geo = Geo {
        x0: 0.0,
        y0: 0.0,
        ew: 400.0,
        eh: 300.0,
        vw: 400.0,
        vh: 300.0,
    };

    #[test]
    fn two_finger_tap_is_right_click() {
        let mut c = Converter::default();
        c.touch_start(&[(100.0, 100.0)], 0.0);
        c.touch_start(&[(100.0, 100.0), (140.0, 100.0)], 20.0);
        assert!(c.touch_end(&[(140.0, 100.0)], &G, 120.0).is_empty());
        let out = c.touch_end(&[], &G, 150.0);
        assert!(matches!(out[0], Input::MouseMove { .. }));
        assert_eq!(
            buttons(&out),
            [(MouseButton::Right, true), (MouseButton::Right, false)]
        );
    }

    #[test]
    fn one_finger_tap_clicks_but_drag_or_long_press_does_not() {
        let mut c = Converter::default();
        c.touch_start(&[(10.0, 10.0)], 0.0);
        assert!(
            c.touch_move(&[(14.0, 12.0)], &G, 50.0).0.is_empty(),
            "흔들림은 이동이 아님"
        );
        assert_eq!(c.touch_end(&[], &G, 100.0).len(), 3);

        c.touch_start(&[(10.0, 10.0)], 1000.0);
        assert_eq!(c.touch_move(&[(60.0, 10.0)], &G, 1050.0).0.len(), 1);
        assert!(c.touch_end(&[], &G, 1100.0).is_empty());

        c.touch_start(&[(10.0, 10.0)], 2000.0);
        assert!(c.touch_end(&[], &G, 2000.0 + TAP_MAX_MS + 1.0).is_empty());
    }

    #[test]
    fn trackpad_moves_cursor_relative_and_clamps() {
        let mut c = Converter::new();
        c.touch_start(&[(0.0, 0.0)], 0.0);
        let (out, _) = c.touch_move(&[(100.0, 0.0)], &geo(300.0, 200.0), 10.0);
        match out[..] {
            [Input::MouseMove { x, y, .. }] => {
                assert!((x - 1.0).abs() < 1e-6, "0.5 + 100*1.5/300 = 1.0");
                assert!((y - 0.5).abs() < 1e-6);
            }
            _ => panic!("{out:?}"),
        }
        c.touch_move(&[(400.0, 0.0)], &geo(300.0, 200.0), 20.0);
        assert!(c.cursor.0 <= 1.0);
        // 확대하면(영상이 두 배로 보이면) 같은 손가락 이동에 커서가 절반만 간다 — 정밀해진다.
        let mut c = Converter::new();
        c.touch_start(&[(0.0, 0.0)], 0.0);
        c.touch_move(&[(100.0, 0.0)], &geo(600.0, 400.0), 10.0);
        assert!((c.cursor.0 - 0.75).abs() < 1e-6);
    }

    #[test]
    fn two_fingers_pick_scroll_or_pinch_once() {
        let mut c = Converter::default();
        c.touch_start(&[(0.0, 100.0), (40.0, 100.0)], 0.0);
        let (out, p) = c.touch_move(&[(0.0, 50.0), (40.0, 50.0)], &G, 10.0);
        assert!(matches!(out[..], [Input::Wheel { dy, .. }] if dy == 50.0));
        assert!(p.is_none());
        // 스크롤로 정했으면 손가락이 벌어져도 확대하지 않는다.
        let (out, p) = c.touch_move(&[(0.0, 50.0), (80.0, 50.0)], &G, 20.0);
        assert!(p.is_none() && matches!(out[..], [Input::Wheel { .. }]));
        c.touch_end(&[], &G, 30.0);

        // 벌어지면 집기 — 호스트에는 아무것도 안 간다.
        c.touch_start(&[(100.0, 100.0), (140.0, 100.0)], 1000.0);
        let (out, p) = c.touch_move(&[(80.0, 100.0), (160.0, 100.0)], &G, 1010.0);
        assert!(out.is_empty());
        assert_eq!(
            p,
            Some(Pinch {
                factor: 2.0,
                center: (120.0, 100.0),
                pan: (0.0, 0.0)
            })
        );
        assert!(
            c.touch_end(&[], &G, 1100.0).is_empty(),
            "집기 뒤엔 클릭 없음"
        );
    }

    #[test]
    fn leftover_finger_after_pinch_is_ignored_and_cancel_is_not_a_tap() {
        let mut c = Converter {
            direct: true,
            ..Converter::new()
        };
        c.touch_start(&[(100.0, 100.0), (140.0, 100.0)], 0.0);
        c.touch_move(&[(80.0, 100.0), (160.0, 100.0)], &G, 10.0);
        c.touch_end(&[(160.0, 100.0)], &G, 20.0);
        assert_eq!(c.touch_move(&[(161.0, 150.0)], &G, 30.0), (vec![], None));
        assert!(c.touch_end(&[], &G, 40.0).is_empty());
        // 짧게 닿았다 취소 = 클릭 아님. 끌던 중 취소 = 왼쪽 버튼 놓기.
        c.touch_start(&[(100.0, 100.0)], 1000.0);
        assert!(c.touch_cancel().is_empty());
        c.touch_start(&[(100.0, 75.0)], 2000.0);
        c.touch_move(&[(200.0, 75.0)], &G, 2000.0 + LONG_PRESS_MS + 10.0);
        assert_eq!(buttons(&c.touch_cancel()), [(MouseButton::Left, false)]);
    }

    #[test]
    fn second_finger_after_scroll_can_pinch() {
        let mut c = Converter {
            direct: true,
            ..Converter::new()
        };
        c.touch_start(&[(100.0, 200.0)], 0.0);
        c.touch_move(&[(100.0, 150.0)], &G, 10.0);
        c.touch_start(&[(100.0, 150.0), (140.0, 150.0)], 20.0);
        let (_, p) = c.touch_move(&[(80.0, 150.0), (160.0, 150.0)], &G, 30.0);
        assert!(p.is_some_and(|p| p.factor == 2.0));
    }

    #[test]
    fn trackpad_tap_then_drag_holds_left_button() {
        let mut c = Converter::new();
        c.touch_start(&[(10.0, 10.0)], 0.0);
        c.touch_end(&[], &G, 80.0);
        c.touch_start(&[(10.0, 10.0)], 200.0);
        let (out, _) = c.touch_move(&[(50.0, 10.0)], &G, 260.0);
        assert_eq!(buttons(&out), [(MouseButton::Left, true)]);
        assert_eq!(
            buttons(&c.touch_end(&[], &G, 400.0)),
            [(MouseButton::Left, false)]
        );
    }

    #[test]
    fn direct_touch_clicks_where_touched() {
        let mut c = Converter {
            direct: true,
            ..Converter::new()
        };
        c.touch_start(&[(100.0, 75.0)], 0.0);
        let out = c.touch_end(&[], &G, 80.0);
        assert_eq!(last_move(&out), (0.25, 0.25));
        assert_eq!(
            buttons(&out),
            [(MouseButton::Left, true), (MouseButton::Left, false)]
        );
        // 두 번째 탭이 4px 비켜 닿아도 첫 탭 자리로 — 더블클릭.
        c.touch_start(&[(104.0, 78.0)], 200.0);
        assert_eq!(last_move(&c.touch_end(&[], &G, 260.0)), (0.25, 0.25));
        // 시간이 지나면 새 자리.
        c.touch_start(&[(104.0, 78.0)], 2000.0);
        assert_eq!(last_move(&c.touch_end(&[], &G, 2060.0)), (0.26, 0.26));
        // 검은 띠 위 탭은 보내지 않는다.
        let band = Geo {
            ew: 400.0,
            eh: 400.0,
            vw: 400.0,
            vh: 200.0,
            ..Default::default()
        };
        c.touch_start(&[(200.0, 20.0)], 5000.0);
        assert!(c.touch_end(&[], &band, 5050.0).is_empty());
    }

    #[test]
    fn direct_touch_long_press_right_clicks_or_drags_and_drag_scrolls() {
        let mut c = Converter {
            direct: true,
            ..Converter::new()
        };
        c.touch_start(&[(100.0, 75.0)], 0.0);
        let out = c.touch_end(&[], &G, LONG_PRESS_MS + 10.0);
        assert_eq!(
            buttons(&out),
            [(MouseButton::Right, true), (MouseButton::Right, false)]
        );

        c.touch_start(&[(100.0, 75.0)], 2000.0);
        let (out, _) = c.touch_move(&[(200.0, 75.0)], &G, 2000.0 + LONG_PRESS_MS + 10.0);
        assert_eq!(buttons(&out), [(MouseButton::Left, true)]);
        assert_eq!(last_move(&out), (0.5, 0.25));
        assert_eq!(
            buttons(&c.touch_end(&[], &G, 3000.0)),
            [(MouseButton::Left, false)]
        );

        // 바로 끌면 스크롤.
        c.touch_start(&[(100.0, 200.0)], 4000.0);
        let (out, _) = c.touch_move(&[(100.0, 100.0)], &G, 4050.0);
        assert!(matches!(out[..], [Input::Wheel { dy, .. }] if dy == 100.0));
    }

    #[test]
    fn composition_is_not_sent_twice() {
        let mut c = Converter::default();
        assert_eq!(texts(&c.text("안녕")), ["안녕"]);
        // 조합 끝 사건과 input 사건이 같은 값을 두 번 준다(Safari).
        assert!(c.text("안녕").is_empty());
        assert_eq!(texts(&c.text("안녕하세요")), ["하세요"]);
    }

    #[test]
    fn replaced_syllable_sends_backspace_then_text() {
        // iOS 한글 자판은 조합 사건 없이 글자를 바꿔 끼우기도 한다: ㅇ → 아 → 안.
        let mut c = Converter::default();
        c.text("ㅇ");
        let out = c.text("아");
        assert_eq!(keys(&out), [(HID_BACKSPACE, true), (HID_BACKSPACE, false)]);
        assert_eq!(texts(&out), ["아"]);
        let out = c.text("");
        assert_eq!(keys(&out).len(), 2);
        c.text_reset();
        assert_eq!(texts(&c.text("a")), ["a"]);
    }

    #[test]
    fn soft_keyboard_without_code_and_ime_toggle() {
        assert_eq!(
            key_to_send("Enter", "", false, false, false),
            Some(HID_ENTER)
        );
        assert_eq!(
            key_to_send("Backspace", "", false, false, true),
            Some(HID_BACKSPACE)
        );
        assert_eq!(
            key_to_send("HangulMode", "AltRight", false, false, true),
            None
        );
    }

    #[test]
    fn newline_in_text_becomes_enter_key() {
        let mut c = Converter::new();
        let out = c.text("ab\ncd");
        assert_eq!(texts(&out), ["ab", "cd"]);
        assert_eq!(keys(&out), [(HID_ENTER, true), (HID_ENTER, false)]);
    }

    #[test]
    fn latched_ctrl_turns_next_letter_into_shortcut_then_releases() {
        let mut c = Converter::default();
        assert_eq!(keys(&c.toggle_mod(HID_CTRL)), [(HID_CTRL, true)]);
        let out = c.text("c");
        assert_eq!(keys(&out), [(0x06, true), (0x06, false), (HID_CTRL, false)]);
        assert!(c.latched().is_empty());
        // 다시 누르면 놓기.
        c.toggle_mod(HID_ALT);
        assert_eq!(keys(&c.toggle_mod(HID_ALT)), [(HID_ALT, false)]);
        // Esc 막대 키도 잡은 수정키를 놓는다.
        c.toggle_mod(HID_WIN);
        assert_eq!(
            keys(&c.tap_key(HID_ESC)),
            [(HID_ESC, true), (HID_ESC, false), (HID_WIN, false)]
        );
    }

    #[test]
    fn seq_increases_across_kinds() {
        let mut c = Converter::default();
        let a = c.mouse_move(0.1, 0.1);
        let b = c.wheel(0.0, 1.0);
        let s = |i: &Input| match i {
            Input::MouseMove { seq, .. } | Input::Wheel { seq, .. } => *seq,
            _ => 0,
        };
        assert!(s(&b) > s(&a));
    }
}

//! 입력 주입(SendInput) — D-01·D-02·D-03. 키는 스캔 코드로 보내 호스트 자판 배치와 무관하게 같은 물리 키가 눌린다.
use proto::{Input, MouseButton};
use std::collections::HashSet;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, MOUSEINPUT, VK_PAUSE,
};

/// 키 하나를 Windows 로 보내는 방법.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Set 1 스캔 코드 + E0 확장 여부.
    Scan(u16, bool),
    /// 스캔 코드로 못 보내는 키(Pause 는 E1 열).
    Vk(u16),
}

/// USB HID 키보드 페이지(0x07) → Windows. 웹앱 `input::hid_usage` 가 보내는 키를 모두 덮는다.
pub fn key(hid: u16) -> Option<Key> {
    const LETTERS: [u16; 26] = [
        0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18,
        0x19, 0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C,
    ];
    let s = |c| Some(Key::Scan(c, false));
    let e = |c| Some(Key::Scan(c, true));
    match hid {
        0x04..=0x1D => s(LETTERS[(hid - 0x04) as usize]),
        0x1E..=0x27 => s(hid - 0x1E + 0x02), // 1~9, 0
        0x28 => s(0x1C),
        0x29 => s(0x01),
        0x2A => s(0x0E),
        0x2B => s(0x0F),
        0x2C => s(0x39),
        0x2D => s(0x0C),
        0x2E => s(0x0D),
        0x2F => s(0x1A),
        0x30 => s(0x1B),
        0x31 | 0x32 => s(0x2B),
        0x33 => s(0x27),
        0x34 => s(0x28),
        0x35 => s(0x29),
        0x36 => s(0x33),
        0x37 => s(0x34),
        0x38 => s(0x35),
        0x39 => s(0x3A),
        0x3A..=0x43 => s(hid - 0x3A + 0x3B), // F1~F10
        0x44 => s(0x57),
        0x45 => s(0x58),
        0x46 => e(0x37),
        0x47 => s(0x46),
        0x48 => Some(Key::Vk(VK_PAUSE)),
        0x49 => e(0x52),
        0x4A => e(0x47),
        0x4B => e(0x49),
        0x4C => e(0x53),
        0x4D => e(0x4F),
        0x4E => e(0x51),
        0x4F => e(0x4D),
        0x50 => e(0x4B),
        0x51 => e(0x50),
        0x52 => e(0x48),
        0x64 => s(0x56),
        0x65 => e(0x5D),
        0xE0 => s(0x1D),
        0xE1 => s(0x2A),
        0xE2 => s(0x38),
        0xE3 => e(0x5B),
        0xE4 => e(0x1D),
        0xE5 => s(0x36),
        0xE6 => e(0x38),
        0xE7 => e(0x5C),
        _ => None,
    }
}

/// 0~1 → SendInput 절대 좌표(0~65535, 기본 모니터 — 캡처하는 모니터와 같다).
pub fn abs(v: f32) -> i32 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as i32
}

/// 뷰어 휠(px, 아래·오른쪽이 +) → Windows 휠(한 칸 120, 위가 +). 브라우저 한 칸이 대개 100px.
pub fn wheel(dy_px: f32) -> i32 {
    (-dy_px * 1.2).round() as i32
}

fn mouse(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as _,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn kbd(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    // SAFETY: 슬라이스는 호출 동안 살아 있고 크기는 INPUT 하나 크기다.
    let n = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if n as usize != inputs.len() {
        // 보안 데스크톱(UAC·잠금)에서는 일반 프로세스의 주입이 막힌다 — 첫 작업 2.
        tracing::warn!(event = "input.blocked", sent = n, want = inputs.len());
    }
}

fn key_input(k: Key, down: bool) -> INPUT {
    let up = if down { 0 } else { KEYEVENTF_KEYUP };
    match k {
        Key::Scan(c, ext) => kbd(
            0,
            c,
            KEYEVENTF_SCANCODE | up | if ext { KEYEVENTF_EXTENDEDKEY } else { 0 },
        ),
        Key::Vk(v) => kbd(v, 0, up),
    }
}

fn button_flag(b: MouseButton, down: bool) -> u32 {
    match (b, down) {
        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    }
}

/// 눌린 키·버튼을 기억해 채널이 닫히면 모두 놓는다(DL-17 — 뷰어가 사라져도 호스트에 눌린 채 남지 않게).
#[derive(Default)]
pub struct Injector {
    keys: HashSet<u16>,
    buttons: Vec<MouseButton>,
}

impl Injector {
    pub fn apply(&mut self, i: &Input) {
        match i {
            Input::MouseMove { x, y, .. } => send(&[mouse(
                abs(*x),
                abs(*y),
                0,
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
            )]),
            Input::MouseButton { button, down, .. } => {
                self.buttons.retain(|b| b != button);
                if *down {
                    self.buttons.push(*button);
                }
                send(&[mouse(0, 0, 0, button_flag(*button, *down))]);
            }
            Input::Wheel { dx, dy, .. } => {
                let mut v = vec![];
                if *dy != 0.0 {
                    v.push(mouse(0, 0, wheel(*dy), MOUSEEVENTF_WHEEL));
                }
                if *dx != 0.0 {
                    v.push(mouse(0, 0, -wheel(*dx), MOUSEEVENTF_HWHEEL));
                }
                send(&v);
            }
            Input::Key {
                hid_usage, down, ..
            } => {
                let Some(k) = key(*hid_usage) else {
                    tracing::warn!(event = "input.unknown_key", hid = hid_usage);
                    return;
                };
                if *down {
                    self.keys.insert(*hid_usage);
                } else {
                    self.keys.remove(hid_usage);
                }
                send(&[key_input(k, *down)]);
            }
            Input::Text { text, .. } => {
                let v: Vec<INPUT> = text
                    .encode_utf16()
                    .flat_map(|u| {
                        [
                            kbd(0, u, KEYEVENTF_UNICODE),
                            kbd(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                        ]
                    })
                    .collect();
                send(&v);
            }
        }
    }

    pub fn release_all(&mut self) {
        if self.keys.is_empty() && self.buttons.is_empty() {
            return;
        }
        let mut v: Vec<INPUT> = self
            .keys
            .drain()
            .filter_map(key)
            .map(|k| key_input(k, false))
            .collect();
        v.extend(
            self.buttons
                .drain(..)
                .map(|b| mouse(0, 0, 0, button_flag(b, false))),
        );
        send(&v);
    }
}

/// 오류·panic 으로 세션이 끝나도 눌린 키·버튼을 놓는다.
impl Drop for Injector {
    fn drop(&mut self) {
        self.release_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_and_wheel() {
        assert_eq!(abs(0.0), 0);
        assert_eq!(abs(0.5), 32768);
        assert_eq!(abs(1.0), 65535);
        assert_eq!(abs(1.5), 65535);
        assert_eq!(wheel(100.0), -120); // 아래로 한 칸
        assert_eq!(wheel(-100.0), 120);
    }

    #[test]
    fn hid_table_covers_web_keys() {
        assert_eq!(key(0x04), Some(Key::Scan(0x1E, false))); // A
        assert_eq!(key(0x1D), Some(Key::Scan(0x2C, false))); // Z
        assert_eq!(key(0x1E), Some(Key::Scan(0x02, false))); // 1
        assert_eq!(key(0x27), Some(Key::Scan(0x0B, false))); // 0
        assert_eq!(key(0x3A), Some(Key::Scan(0x3B, false))); // F1
        assert_eq!(key(0x3E), Some(Key::Scan(0x3F, false))); // F5
        assert_eq!(key(0x45), Some(Key::Scan(0x58, false))); // F12
        assert_eq!(key(0x4C), Some(Key::Scan(0x53, true))); // Delete
        assert_eq!(key(0x52), Some(Key::Scan(0x48, true))); // 위
        assert_eq!(key(0xE3), Some(Key::Scan(0x5B, true))); // 왼쪽 Win
        assert_eq!(key(0x48), Some(Key::Vk(VK_PAUSE)));
        // 웹앱 hid_usage 가 내는 범위 전부(숫자패드·한/영은 안 보냄).
        for h in (0x04..=0x52).chain([0x64, 0x65]).chain(0xE0..=0xE7) {
            assert!(key(h).is_some(), "HID {h:#x}");
        }
        assert_eq!(key(0x90), None);
    }
}

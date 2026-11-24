//! 원격 화면 — 영상·입력 변환 연결·확대·특수키 막대·가상 키보드(FR-006·007). 연결과 계측은 `session`.
use crate::hangul::{self, Hangul};
use crate::input::{self, Converter, Geo};
use crate::session::{self, Phase, Ui};
use crate::vkbd::{Act, VKbd};
use crate::zoom::Zoom;
use crate::{AppState, Screen};
use leptos::{html, prelude::*};
use proto::{Device, Input, MouseButton, NetPath};
use std::cell::RefCell;
use web_sys::{FocusOptions, MouseEvent, TouchEvent, WheelEvent};

/// 입력칸을 비우는 길이 — 비교할 글을 짧게 둔다.
const TEXT_KEEP: usize = 64;
/// 03 상수 표 — 화면 짧은 변이 이 이상이면 태블릿(기본 직접 터치).
const TABLET_MIN_PX: f64 = 600.0;
const MODE_KEY: &str = "webdesktop.touch";

#[derive(Default)]
struct Kbd {
    conv: Converter,
    composing: bool,
    /// 키로 보낸 뒤 아직 안 뗀 키 — keyup 을 짝 맞춰 보낸다.
    down: Vec<u16>,
    /// 누른 채인 마우스 버튼 — 화면 밖에서 떼도 원격에 눌린 채 남지 않게.
    buttons: Vec<MouseButton>,
    /// 가상 키보드로 친 글(한글 조합 포함).
    han: Hangul,
}

impl Kbd {
    /// 글 비교를 새로 시작한다(원격 커서를 옮긴 뒤 자동 고침·조합이 엉뚱한 글자를 지우지 않게).
    fn reset_text(&mut self, kb: &web_sys::HtmlTextAreaElement) {
        if !self.composing {
            kb.set_value("");
            self.conv.text_reset();
            self.han.clear();
        }
    }

    /// 가상 키보드 글을 버리고 비교를 새로 시작한다(시스템 자판·실제 키보드로 넘어갈 때).
    fn leave_vk(&mut self) {
        if !self.han.buf.is_empty() {
            self.han.clear();
            self.conv.text_reset();
        }
    }

    /// 가상 키보드 키 하나.
    fn vkey(&mut self, a: Act) -> Vec<Input> {
        match a {
            // Ctrl 을 잡은 채 지우기 = Ctrl+Backspace 한 번(조합 중 글을 비교로 고치지 않는다).
            Act::Backspace if !self.conv.latched().is_empty() => {
                self.han.clear();
                self.conv.text_reset();
                self.conv.tap_key(input::HID_BACKSPACE)
            }
            Act::Commit => {
                self.han.commit();
                vec![]
            }
            // Ctrl·Alt·Win 을 잡았으면 글자 대신 그 자리 키로(Ctrl+C).
            Act::Char { hid, shift, .. } if !self.conv.latched().is_empty() => {
                self.han.clear();
                self.conv.text_reset();
                let mut out = vec![];
                if shift && !self.conv.latched().contains(&input::HID_SHIFT) {
                    out.extend(self.conv.toggle_mod(input::HID_SHIFT));
                }
                out.extend(self.conv.tap_key(hid));
                out
            }
            Act::Char { ch, .. } => {
                if hangul::is_jamo(ch) {
                    self.han.push_jamo(ch);
                } else {
                    self.han.push_char(ch);
                }
                let out = self.conv.text(&self.han.buf);
                if ch == ' ' && self.han.buf.chars().count() > TEXT_KEEP {
                    self.han.clear();
                    self.conv.text_reset();
                }
                out
            }
            Act::Backspace if self.han.backspace() => self.conv.text(&self.han.buf),
            Act::Backspace => self.conv.tap_key(input::HID_BACKSPACE),
            Act::Key(h) => {
                self.han.clear();
                self.conv.text_reset();
                self.conv.tap_key(h)
            }
        }
    }
}

thread_local! {
    static KBD: RefCell<Kbd> = RefCell::default();
}

fn with<R>(f: impl FnOnce(&mut Kbd) -> R) -> R {
    KBD.with(|k| f(&mut k.borrow_mut()))
}

fn send(v: Vec<Input>) {
    session::send(v);
}

/// `with` 안에서 만든 입력을 보내고 나머지 값을 돌려준다.
fn send_and<R>(f: impl FnOnce(&mut Kbd) -> (Vec<Input>, R)) -> R {
    let (v, r) = with(f);
    send(v);
    r
}

/// 저장한 조작 방식. 없으면 태블릿은 직접 터치, 폰은 트랙패드(research/touch-control.md).
fn direct_default() -> bool {
    let w = crate::api::window();
    let saved = w
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(MODE_KEY).ok().flatten());
    match saved.as_deref() {
        Some("direct") => true,
        Some("trackpad") => false,
        _ => {
            let px = |v: Result<wasm_bindgen::JsValue, _>| {
                v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
            };
            px(w.inner_width()).min(px(w.inner_height())) >= TABLET_MIN_PX
        }
    }
}

const BAR: [(&str, u16, bool); 6] = [
    ("Esc", input::HID_ESC, false),
    ("Tab", input::HID_TAB, false),
    ("Ctrl", input::HID_CTRL, true),
    ("Alt", input::HID_ALT, true),
    ("Win", input::HID_WIN, true),
    ("Del", input::HID_DELETE, false),
];

#[component]
pub fn Remote(device: Device, t0: f64) -> impl IntoView {
    let st = expect_context::<AppState>();
    let ui = Ui {
        phase: RwSignal::new(Phase::Connecting),
        path: RwSignal::new(None),
    };
    let video = NodeRef::<html::Video>::new();
    let stage = NodeRef::<html::Div>::new();
    let kb = NodeRef::<html::Textarea>::new();
    let cursor = RwSignal::new(None::<(f64, f64)>);
    let latched = RwSignal::new(Vec::<u16>::new());
    let zoom = RwSignal::new(Zoom::default());
    let direct = RwSignal::new(direct_default());
    let vk_open = RwSignal::new(false);
    // 터치 기기에서 첫 터치 전까지 손짓 안내를 보인다.
    let hint = RwSignal::new(crate::api::window().navigator().max_touch_points() > 0);
    KBD.with(|k| {
        let mut conv = Converter::new();
        conv.direct = direct.get_untracked();
        *k.borrow_mut() = Kbd {
            conv,
            ..Default::default()
        }
    });

    // iOS 자동 고침은 autocorrect 로만 꺼진다.
    kb.on_load(|k| {
        let _ = k.set_attribute("autocorrect", "off");
    });
    let id = device.id.clone();
    video.on_load(move |v| {
        v.set_muted(true);
        v.set_autoplay(true);
        let _ = v.set_attribute("playsinline", "");
        leptos::task::spawn_local(session::open(id, t0, v, ui));
    });
    // 화면을 떠날 때 끌던 버튼·잡은 수정키를 놓고 닫는다.
    // shortcut: 채널이 이미 끊겼으면 이 놓기가 안 간다 — 에이전트가 채널 닫힘에 모두 놓아야 한다(T9).
    on_cleanup(|| {
        send(with(|k| {
            let mut v = k.conv.release_all();
            for b in std::mem::take(&mut k.buttons) {
                v.extend(k.conv.mouse_button(b, false));
            }
            v
        }));
        session::close();
    });

    // 무대의 화면 위치·크기. 확대는 영상에만 걸려 무대는 그대로다.
    let stage_box = move || {
        let r = stage.get_untracked()?.get_bounding_client_rect();
        Some((r.left(), r.top(), r.width(), r.height()))
    };
    // 무대 안 영상 자리(확대 반영). 무대가 줄었으면(키보드·화면 돌림) 확대를 다시 가둔다.
    let geo = move || {
        let (_, _, w, h) = stage_box()?;
        let v = video.get_untracked()?;
        let mut z = zoom.get_untracked();
        if z.clamp((w, h)) {
            zoom.set(z);
        }
        Some(Geo {
            x0: z.tx,
            y0: z.ty,
            ew: w * z.s,
            eh: h * z.s,
            vw: v.video_width() as f64,
            vh: v.video_height() as f64,
        })
    };
    let local = move |cx: f64, cy: f64| stage_box().map(|(l, t, ..)| (cx - l, cy - t));
    let unit = move |cx: i32, cy: i32| geo()?.unit(local(cx as f64, cy as f64)?);
    let focus_kb = move || {
        if let Some(k) = kb.get() {
            let o = FocusOptions::new();
            o.set_prevent_scroll(true);
            let _ = k.focus_with_options(&o);
        }
    };
    // 뒤로·앞으로(3·4)는 보내지 않는다.
    let button = |e: &MouseEvent| match e.button() {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        _ => None,
    };
    let on_move = move |e: MouseEvent| {
        if let Some((x, y)) = unit(e.client_x(), e.client_y()) {
            send(vec![with(|k| k.conv.mouse_move(x, y))]);
        }
    };
    let on_down = move |e: MouseEvent| {
        e.prevent_default();
        focus_kb();
        if let (Some((x, y)), Some(b)) = (unit(e.client_x(), e.client_y()), button(&e)) {
            send(with(|k| {
                if let Some(kb) = kb.get() {
                    k.reset_text(&kb);
                }
                k.buttons.push(b);
                let mut v = vec![k.conv.mouse_move(x, y)];
                v.extend(k.conv.mouse_button(b, true));
                v
            }));
        }
    };
    let release = move |b: MouseButton| {
        send(with(|k| match k.buttons.iter().position(|&h| h == b) {
            Some(i) => {
                k.buttons.remove(i);
                k.conv.mouse_button(b, false)
            }
            None => vec![],
        }));
        latched.set(with(|k| k.conv.latched().to_vec()));
    };
    let on_up = move |e: MouseEvent| {
        if let Some(b) = button(&e) {
            release(b);
        }
    };
    let on_leave = move |_| {
        for b in with(|k| k.buttons.clone()) {
            release(b);
        }
    };
    let on_wheel = move |e: WheelEvent| {
        e.prevent_default();
        // deltaMode 1 = 줄, 2 = 쪽 → px.
        let k = match e.delta_mode() {
            1 => 16.0,
            2 => stage_box().map_or(800.0, |b| b.3),
            _ => 1.0,
        };
        send(vec![with(|c| {
            c.conv
                .wheel((e.delta_x() * k) as f32, (e.delta_y() * k) as f32)
        })]);
    };

    // 터치 — 트랙패드(커서는 뷰어가 그림) 또는 직접 터치. 좌표는 무대 px.
    let points = move |list: web_sys::TouchList| {
        (0..list.length())
            .filter_map(|i| list.item(i))
            .filter_map(|p| local(p.client_x() as f64, p.client_y() as f64))
            .collect::<Vec<_>>()
    };
    let show_cursor = move || {
        let c = with(|k| k.conv.cursor);
        cursor.set(
            (!direct.get_untracked())
                .then(geo)
                .flatten()
                .and_then(|g| g.stage(c)),
        );
    };
    // 트랙패드: 확대 중이면 커서를 따라 화면을 민다.
    let follow = move || {
        if direct.get_untracked() {
            return;
        }
        let c = with(|k| k.conv.cursor);
        if let (Some(p), Some((.., w, h))) = (geo().and_then(|g| g.stage(c)), stage_box()) {
            zoom.update(|z| z.follow(p, (w, h)));
        }
        show_cursor();
    };
    let on_tstart = move |e: TouchEvent| {
        e.prevent_default();
        hint.set(false);
        let pts = points(e.target_touches());
        with(|k| k.conv.touch_start(&pts, e.time_stamp()));
        show_cursor();
    };
    let on_tmove = move |e: TouchEvent| {
        e.prevent_default();
        let pts = points(e.target_touches());
        let g = geo().unwrap_or_default();
        let (out, pinch) = with(|k| k.conv.touch_move(&pts, &g, e.time_stamp()));
        let moved = out.iter().any(|i| matches!(i, Input::MouseMove { .. }));
        send(out);
        if let (Some(p), Some((.., w, h))) = (pinch, stage_box()) {
            zoom.update(|z| z.pinch(p.factor, p.center, p.pan, (w, h)));
        }
        // 커서가 움직였을 때만 따라간다 — 집기로 다른 곳을 보는 것을 되돌리지 않게.
        if moved {
            follow();
        } else {
            show_cursor();
        }
    };
    let on_tend = move |e: TouchEvent| {
        e.prevent_default();
        let pts = points(e.target_touches());
        let g = geo().unwrap_or_default();
        send(with(|k| {
            let out = k.conv.touch_end(&pts, &g, e.time_stamp());
            if let (true, Some(kb)) = (!out.is_empty(), kb.get()) {
                k.reset_text(&kb);
            }
            out
        }));
        latched.set(with(|k| k.conv.latched().to_vec()));
    };

    // 숨은 입력칸 — 실제 키보드와 휴대폰 자판(한글 조합 포함)을 받는다.
    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let Some(k) = kb.get() else { return };
        let modded = e.ctrl_key() || e.alt_key() || e.meta_key();
        let composing = with(|s| s.composing) || e.is_composing();
        let Some(hid) =
            input::key_to_send(&e.key(), &e.code(), modded, composing, k.value().is_empty())
        else {
            return;
        };
        e.prevent_default();
        send(with(|s| {
            s.leave_vk();
            // 길게 눌러 반복되는 keydown 은 한 번만 적는다(keyup 짝이 하나라서).
            if !e.repeat() || !s.down.contains(&hid) {
                s.down.push(hid);
            }
            s.conv.key(hid, true)
        }));
        if hid == input::HID_ENTER {
            k.set_value("");
            with(|s| s.conv.text_reset());
        }
    };
    let on_keyup = move |e: web_sys::KeyboardEvent| {
        let Some(hid) = input::event_hid(&e.key(), &e.code()) else {
            return;
        };
        send(with(|s| match s.down.iter().position(|&h| h == hid) {
            Some(i) => {
                s.down.remove(i);
                s.conv.key(hid, false)
            }
            None => vec![],
        }));
        latched.set(with(|k| k.conv.latched().to_vec()));
    };
    let sync_text = move || {
        let Some(k) = kb.get() else { return };
        let value = k.value();
        let shortcut = send_and(|s| {
            s.leave_vk();
            let shortcut = !s.conv.latched().is_empty();
            (s.conv.text(&value), shortcut)
        });
        // 잡은 수정키로 보낸 글자는 호스트 글이 아니다 — 비교를 새로 시작한다.
        if shortcut || value.contains('\n') || value.chars().count() > TEXT_KEEP {
            k.set_value("");
            with(|s| s.conv.text_reset());
        }
        latched.set(with(|s| s.conv.latched().to_vec()));
    };

    // 가상 키보드 — 시스템 자판 입력칸에 남은 글이 있으면 비교를 새로 시작한다.
    let on_vkey = Callback::new(move |a: Act| {
        send(with(|s| {
            if let Some(k) = kb.get_untracked().filter(|k| !k.value().is_empty()) {
                k.set_value("");
                s.conv.text_reset();
            }
            s.vkey(a)
        }));
        latched.set(with(|k| k.conv.latched().to_vec()));
    });
    // 키보드가 열리고 닫히면 무대 크기가 바뀐다 — 그려진 뒤 확대를 가두고 커서를 따라간다.
    let refit = move || {
        request_animation_frame(move || {
            if let Some((.., w, h)) = stage_box() {
                zoom.update(|z| {
                    z.clamp((w, h));
                });
            }
            follow();
        })
    };
    let toggle_vk = move || {
        let open = !vk_open.get_untracked();
        vk_open.set(open);
        if open {
            // 시스템 자판을 닫는다.
            if let Some(k) = kb.get_untracked() {
                let _ = k.blur();
            }
        }
        refit();
    };
    let set_mode = move |d: bool| {
        direct.set(d);
        with(|k| k.conv.direct = d);
        if let Ok(Some(s)) = crate::api::window().local_storage() {
            let _ = s.set_item(MODE_KEY, if d { "direct" } else { "trackpad" });
        }
        show_cursor();
    };

    let bar = BAR
        .iter()
        .map(|&(label, hid, is_mod)| {
            let act = move || {
                send(with(|k| {
                    if is_mod {
                        k.conv.toggle_mod(hid)
                    } else {
                        k.han.clear();
                        k.conv.text_reset();
                        k.conv.tap_key(hid)
                    }
                }));
                latched.set(with(|k| k.conv.latched().to_vec()));
            };
            // 누를 때 입력칸 포커스를 뺏지 않는다(휴대폰 자판이 닫히지 않게).
            view! {
                <button
                    class="key"
                    class:on=move || latched.get().contains(&hid)
                    aria-pressed=move || is_mod.then(|| latched.get().contains(&hid).to_string())
                    on:mousedown=move |e: MouseEvent| { e.prevent_default(); act(); }
                    on:touchstart=move |e: TouchEvent| { e.prevent_default(); act(); }
                >
                    {label}
                </button>
            }
        })
        .collect_view();

    // ?bench=N — 남은 횟수가 있으면 첫 화면(또는 실패) 3초 뒤 닫고 목록으로. 마지막 번은 열어 둔다.
    // 같은 열기(t0)일 때만 닫는다 — 끊김 뒤 다시 Live 가 와도 다음 열기를 닫지 않게.
    // shortcut: 「여는 중」에서 안 끝나면(잠긴 화면) 시험이 멈춘다 — SC-004 안내가 생기면 Ended 로 잡힌다.
    Effect::new(move |_| {
        let done = matches!(ui.phase.get(), Phase::Live | Phase::Ended(_));
        if done && crate::BENCH.with(|b| b.get()) > 0 {
            set_timeout(
                move || {
                    if matches!(st.screen.get_untracked(), Screen::Remote { t0: x, .. } if x == t0)
                    {
                        session::close();
                        st.screen.set(Screen::List);
                    }
                },
                std::time::Duration::from_secs(3),
            );
        }
    });

    let name = device.name.clone();
    let back = move |_| {
        session::close();
        st.screen.set(Screen::List);
    };
    let status = move || match ui.phase.get() {
        Phase::Live => None,
        Phase::Connecting => Some(
            view! { <div class="status"><p>{format!("「{name}」 화면을 여는 중…")}</p></div> }
                .into_any(),
        ),
        Phase::Lost => Some(
            view! { <div class="status"><p>"연결이 잠깐 끊겼습니다. 다시 잇는 중…"</p></div> }
                .into_any(),
        ),
        Phase::Ended(m) => Some(
            view! {
                <div class="status">
                    <p>{if m.is_empty() { "연결을 끝냈습니다" } else { m }}</p>
                    <button class="primary" on:click=back>"목록으로"</button>
                </div>
            }
            .into_any(),
        ),
    };
    let hint_text = move || {
        if direct.get() {
            "탭 = 그 자리 클릭 · 길게 = 오른쪽 클릭 · 길게 누른 뒤 끌기 = 끌기 · 끌기 = 스크롤 · 두 손가락 = 확대·이동"
        } else {
            "한 손가락 끌기 = 커서 이동 · 탭 = 클릭 · 탭 후 끌기 = 끌기 · 두 손가락 탭 = 오른쪽 클릭 · 두 손가락 끌기 = 스크롤 · 벌리기 = 확대"
        }
    };
    let mode_btn = move |d: bool, label: &'static str| {
        view! {
            <button class="item" class:on=move || direct.get() == d aria-pressed=move || (direct.get() == d).to_string()
                popovertarget="more" popovertargetaction="hide" on:click=move |_| set_mode(d)>
                {label}
            </button>
        }
    };

    view! {
        <main class="remote" class:kbd-open=vk_open>
            <div
                class="stage"
                node_ref=stage
                on:mousemove=on_move
                on:mousedown=on_down
                on:mouseup=on_up
                on:mouseleave=on_leave
                on:wheel=on_wheel
                on:contextmenu=|e: MouseEvent| e.prevent_default()
                on:touchstart=on_tstart
                on:touchmove=on_tmove
                on:touchend=on_tend
                on:touchcancel=move |e: TouchEvent| {
                    e.prevent_default();
                    send(with(|k| k.conv.touch_cancel()));
                    latched.set(with(|k| k.conv.latched().to_vec()));
                }
            >
                <video node_ref=video style=move || zoom.get().css()></video>
                {move || cursor.get().map(|(x, y)| view! {
                    <span class="cursor" style=format!("transform: translate({x}px, {y}px)")></span>
                })}
                {move || ui.path.get().map(|p| view! {
                    <span class="path" data-path=format!("{p:?}").to_lowercase()>
                        {match p { NetPath::Relay => "중계 연결", NetPath::Direct => "직접 연결" }}
                    </span>
                })}
                {move || (hint.get() && ui.phase.get() == Phase::Live).then(|| view! {
                    <p class="hint">{hint_text}</p>
                })}
                {status}
            </div>
            // 무대 밖에 둔다 — 무대의 터치 처리가 누름을 막지 않게.
            {move || zoom.get().zoomed().then(|| view! {
                <button class="zoom" on:click=move |_| { zoom.set(Zoom::default()); show_cursor(); }>
                    {format!("{:.1}배 · 원래대로", zoom.get().s)}
                </button>
            })}
            <textarea
                class="kb"
                node_ref=kb
                aria-label="원격 입력"
                autocapitalize="off"
                autocomplete="off"
                spellcheck="false"
                on:keydown=on_keydown
                on:keyup=on_keyup
                on:blur=move |_| {
                    // Alt+Tab 처럼 뗀 키가 다른 창으로 가면 원격에 눌린 채 남는다 — 포커스를 잃으면 모두 뗀다.
                    send(with(|s| std::mem::take(&mut s.down).into_iter().flat_map(|h| s.conv.key(h, false)).collect()));
                    latched.set(with(|k| k.conv.latched().to_vec()));
                }
                on:compositionstart=move |_| with(|s| s.composing = true)
                on:compositionend=move |_| { with(|s| s.composing = false); sync_text(); }
                on:input=move |_| if !with(|s| s.composing) { sync_text() }
            ></textarea>
            <nav class="keys" aria-label="특수키">
                <button class="key" class:on=vk_open aria-label="가상 키보드" aria-pressed=move || vk_open.get().to_string()
                    on:click=move |_| toggle_vk()>"자판"</button>
                {bar}
                <button class="key" aria-label="더 보기" popovertarget="more">"⋯"</button>
                <button class="key end" on:click=move |_| { session::close(); st.screen.set(Screen::List); }>"끊기"</button>
            </nav>
            {move || vk_open.get().then(|| view! { <VKbd on=on_vkey/> })}
            <div id="more" class="menu" popover="auto">
                <p class="muted">"조작 방식"</p>
                <div class="pair">{mode_btn(false, "트랙패드")}{mode_btn(true, "직접 터치")}</div>
                <button class="item" popovertarget="more" popovertargetaction="hide" on:click=move |_| {
                    if vk_open.get_untracked() { toggle_vk(); }
                    focus_kb();
                }>"폰 자판 열기(받아쓰기·밀어 쓰기)"</button>
            </div>
        </main>
    }
}

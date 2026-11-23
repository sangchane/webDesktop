//! 원격 화면 — 영상·입력 변환 연결·특수키 막대(FR-006·007). 연결과 계측은 `session`.
use crate::input::{self, Converter};
use crate::session::{self, Phase, Ui};
use crate::{AppState, Screen};
use leptos::{html, prelude::*};
use proto::{Device, Input, MouseButton, NetPath};
use std::cell::RefCell;
use web_sys::{FocusOptions, MouseEvent, TouchEvent, WheelEvent};

/// 입력칸을 비우는 길이 — 비교할 글을 짧게 둔다.
const TEXT_KEEP: usize = 64;

#[derive(Default)]
struct Kbd {
    conv: Converter,
    composing: bool,
    /// 키로 보낸 뒤 아직 안 뗀 키 — keyup 을 짝 맞춰 보낸다.
    down: Vec<u16>,
    /// 누른 채인 마우스 버튼 — 화면 밖에서 떼도 원격에 눌린 채 남지 않게.
    buttons: Vec<MouseButton>,
}

impl Kbd {
    /// 입력칸 비교를 새로 시작한다(원격 커서를 옮긴 뒤 자동 고침이 엉뚱한 글자를 지우지 않게).
    fn reset_text(&mut self, kb: &web_sys::HtmlTextAreaElement) {
        if !self.composing {
            kb.set_value("");
            self.conv.text_reset();
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
    let kb = NodeRef::<html::Textarea>::new();
    let cursor = RwSignal::new(None::<(f64, f64)>);
    let latched = RwSignal::new(Vec::<u16>::new());
    // 터치 기기에서 첫 터치 전까지 손짓 안내를 보인다.
    let hint = RwSignal::new(crate::api::window().navigator().max_touch_points() > 0);
    KBD.with(|k| {
        *k.borrow_mut() = Kbd {
            conv: Converter::new(),
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
    on_cleanup(session::close);

    // 영상 요소 안 좌표와 크기, 영상 원본 크기.
    let geo = move || {
        let v = video.get()?;
        let r = v.get_bounding_client_rect();
        Some((
            r.left(),
            r.top(),
            r.width(),
            r.height(),
            v.video_width() as f64,
            v.video_height() as f64,
        ))
    };
    let unit = move |cx: i32, cy: i32| {
        let (l, t, w, h, vw, vh) = geo()?;
        input::to_unit(cx as f64 - l, cy as f64 - t, w, h, vw, vh)
    };
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
            2 => geo().map_or(800.0, |g| g.3),
            _ => 1.0,
        };
        send(vec![with(|c| {
            c.conv
                .wheel((e.delta_x() * k) as f32, (e.delta_y() * k) as f32)
        })]);
    };

    // 터치 = 트랙패드. 커서는 뷰어가 그린다.
    let points = move |list: web_sys::TouchList| {
        let Some((l, t, ..)) = geo() else {
            return vec![];
        };
        (0..list.length())
            .filter_map(|i| list.item(i))
            .map(|p| (p.client_x() as f64 - l, p.client_y() as f64 - t))
            .collect::<Vec<_>>()
    };
    let content = move || {
        let (_, _, w, h, vw, vh) = geo()?;
        let a = input::from_unit((0.0, 0.0), w, h, vw, vh)?;
        let b = input::from_unit((1.0, 1.0), w, h, vw, vh)?;
        Some(((b.0 - a.0), (b.1 - a.1), w, h, vw, vh))
    };
    let show_cursor = move || {
        let c = with(|k| k.conv.cursor);
        cursor.set(content().and_then(|(_, _, w, h, vw, vh)| input::from_unit(c, w, h, vw, vh)));
    };
    let on_tstart = move |e: TouchEvent| {
        e.prevent_default();
        hint.set(false);
        let pts = points(e.touches());
        with(|k| k.conv.touch_start(&pts, e.time_stamp()));
        show_cursor();
    };
    let on_tmove = move |e: TouchEvent| {
        e.prevent_default();
        let pts = points(e.touches());
        let view = content().map_or((0.0, 0.0), |c| (c.0, c.1));
        send(with(|k| k.conv.touch_move(&pts, view)));
        show_cursor();
    };
    let on_tend = move |e: TouchEvent| {
        e.prevent_default();
        let pts = points(e.touches());
        send(with(|k| {
            let out = k.conv.touch_end(&pts, e.time_stamp());
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
        send(with(|s| s.conv.text(&value)));
        if value.contains('\n') || value.chars().count() > TEXT_KEEP {
            k.set_value("");
            with(|s| s.conv.text_reset());
        }
        latched.set(with(|s| s.conv.latched().to_vec()));
    };

    let bar = BAR
        .iter()
        .map(|&(label, hid, is_mod)| {
            let act = move || {
                send(with(|k| {
                    if is_mod {
                        k.conv.toggle_mod(hid)
                    } else {
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

    view! {
        <main class="remote">
            <div
                class="stage"
                on:mousemove=on_move
                on:mousedown=on_down
                on:mouseup=on_up
                on:mouseleave=on_leave
                on:wheel=on_wheel
                on:contextmenu=|e: MouseEvent| e.prevent_default()
                on:touchstart=on_tstart
                on:touchmove=on_tmove
                on:touchend=on_tend
                on:touchcancel=on_tend
            >
                <video node_ref=video></video>
                {move || cursor.get().map(|(x, y)| view! {
                    <span class="cursor" style=format!("transform: translate({x}px, {y}px)")></span>
                })}
                {move || ui.path.get().map(|p| view! {
                    <span class="path" data-path=format!("{p:?}").to_lowercase()>
                        {match p { NetPath::Relay => "중계 연결", NetPath::Direct => "직접 연결" }}
                    </span>
                })}
                {move || (hint.get() && ui.phase.get() == Phase::Live).then(|| view! {
                    <p class="hint">"한 손가락 끌기 = 커서 이동 · 탭 = 클릭 · 두 손가락 탭 = 오른쪽 클릭 · 두 손가락 끌기 = 스크롤"</p>
                })}
                {status}
            </div>
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
                {bar}
                <button class="key" aria-label="키보드" on:click=move |_| focus_kb()>"자판"</button>
                <button class="key end" on:click=move |_| { session::close(); st.screen.set(Screen::List); }>"끊기"</button>
            </nav>
        </main>
    }
}

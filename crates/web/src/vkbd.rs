//! 자체 가상 키보드 — 화면 아래에 붙는 PC 자판(글자·기호·Fn 3층, 한글 두벌식). 보내기는 `remote` 가 맡는다(`Act`).
//! 시스템 자판에 없는 Esc·F키·방향키를 주고, 한글은 웹앱이 조합해 호스트 IME 와 상관없이 같은 글을 보낸다.
use crate::input::{HID_BACKSPACE, HID_ENTER};
use leptos::prelude::*;
use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::PointerEvent;

#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// 글자. `hid` = 같은 자리 실제 키(Ctrl 등을 잡았을 때 단축키로), `shift` = Shift 를 눌러야 나오는 글자인가.
    Char {
        ch: char,
        hid: u16,
        shift: bool,
    },
    Key(u16),
    Backspace,
    /// 한글·영문을 바꿀 때 — 조합을 끝낸다(다음 자모가 앞 글자에 붙지 않게).
    Commit,
}

/// 키보드 안 상태 키가 바꾸는 것 — (층, Shift, 한글).
type Flip = fn(RwSignal<Layer>, RwSignal<bool>, RwSignal<bool>);

#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Letters,
    Symbols,
    Fn,
}

/// 두벌식 = QWERTY 자리. (영문, 한글, Shift 한글)
const LETTERS: [(&str, &str, &str); 3] = [
    ("qwertyuiop", "ㅂㅈㄷㄱㅅㅛㅕㅑㅐㅔ", "ㅃㅉㄸㄲㅆㅛㅕㅑㅒㅖ"),
    ("asdfghjkl", "ㅁㄴㅇㄹㅎㅗㅓㅏㅣ", "ㅁㄴㅇㄹㅎㅗㅓㅏㅣ"),
    ("zxcvbnm", "ㅋㅌㅊㅍㅠㅜㅡ", "ㅋㅌㅊㅍㅠㅜㅡ"),
];

/// (글자, HID, Shift 글자인가) — USB HID 키보드 페이지 0x07.
const SYMBOLS: [&[(char, u16, bool)]; 4] = [
    &[
        ('1', 0x1E, false),
        ('2', 0x1F, false),
        ('3', 0x20, false),
        ('4', 0x21, false),
        ('5', 0x22, false),
        ('6', 0x23, false),
        ('7', 0x24, false),
        ('8', 0x25, false),
        ('9', 0x26, false),
        ('0', 0x27, false),
    ],
    &[
        ('!', 0x1E, true),
        ('@', 0x1F, true),
        ('#', 0x20, true),
        ('$', 0x21, true),
        ('%', 0x22, true),
        ('^', 0x23, true),
        ('&', 0x24, true),
        ('*', 0x25, true),
        ('(', 0x26, true),
        (')', 0x27, true),
    ],
    &[
        ('-', 0x2D, false),
        ('_', 0x2D, true),
        ('=', 0x2E, false),
        ('+', 0x2E, true),
        ('[', 0x2F, false),
        (']', 0x30, false),
        ('{', 0x2F, true),
        ('}', 0x30, true),
        ('\\', 0x31, false),
        ('|', 0x31, true),
    ],
    &[
        (';', 0x33, false),
        (':', 0x33, true),
        ('\'', 0x34, false),
        ('"', 0x34, true),
        (',', 0x36, false),
        ('<', 0x36, true),
        ('>', 0x37, true),
        ('/', 0x38, false),
        ('?', 0x38, true),
    ],
];

/// (글자, 읽어 줄 이름, HID). 빈 이름 = 글자 그대로.
const FN_KEYS: [[(&str, &str, u16); 6]; 4] = [
    [
        ("F1", "", 0x3A),
        ("F2", "", 0x3B),
        ("F3", "", 0x3C),
        ("F4", "", 0x3D),
        ("F5", "", 0x3E),
        ("F6", "", 0x3F),
    ],
    [
        ("F7", "", 0x40),
        ("F8", "", 0x41),
        ("F9", "", 0x42),
        ("F10", "", 0x43),
        ("F11", "", 0x44),
        ("F12", "", 0x45),
    ],
    [
        ("Ins", "Insert", 0x49),
        ("Home", "", 0x4A),
        ("PgUp", "Page Up", 0x4B),
        ("PrtSc", "Print Screen", 0x46),
        ("↑", "위 화살표", 0x52),
        ("⌫", "지우기", HID_BACKSPACE),
    ],
    [
        ("Del", "Delete", 0x4C),
        ("End", "", 0x4D),
        ("PgDn", "Page Down", 0x4E),
        ("←", "왼쪽 화살표", 0x50),
        ("↓", "아래 화살표", 0x51),
        ("→", "오른쪽 화살표", 0x4F),
    ],
];

/// 길게 누르면 되풀이하는 키 — 지우기·방향키.
fn repeats(a: Act) -> bool {
    matches!(a, Act::Backspace | Act::Key(0x4F..=0x52))
}

/// 되풀이 첫 지연과 간격(ms) — iOS·Windows 자판 기본(약 0.5초 뒤 초당 약 15번)에 맞춤.
const REPEAT_TICK_MS: i32 = 60;
const REPEAT_AFTER_TICKS: u32 = 7;

/// 도는 되풀이 — (setInterval 번호, 그 콜백). 콜백은 멈출 때 버린다.
type Repeat = (i32, Closure<dyn FnMut()>);

thread_local! {
    static REPEAT: RefCell<Option<Repeat>> = const { RefCell::new(None) };
}

fn stop_repeat() {
    if let Some((id, _)) = REPEAT.with(|r| r.borrow_mut().take()) {
        crate::api::window().clear_interval_with_handle(id);
    }
}

fn start_repeat(on: Callback<Act>, a: Act) {
    stop_repeat();
    let mut n = 0u32;
    let tick = Closure::<dyn FnMut()>::new(move || {
        n += 1;
        if n > REPEAT_AFTER_TICKS {
            on.run(a);
        }
    });
    if let Ok(id) = crate::api::window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        REPEAT_TICK_MS,
    ) {
        REPEAT.with(|r| *r.borrow_mut() = Some((id, tick)));
    }
}

#[component]
pub fn VKbd(on: Callback<Act>) -> impl IntoView {
    let layer = RwSignal::new(Layer::Letters);
    let shift = RwSignal::new(false);
    let ko = RwSignal::new(false);
    on_cleanup(stop_repeat);

    // 누르는 순간 보낸다(떼기를 기다리지 않음). 포커스를 옮기지 않아 시스템 자판이 뜨지 않는다.
    let key =
        move |label: Signal<String>, aria: Option<&'static str>, class: &'static str, act: Act| {
            let down = move |e: PointerEvent| {
                e.prevent_default();
                on.run(act);
                if matches!(act, Act::Char { .. }) && layer.get_untracked() == Layer::Letters {
                    shift.set(false);
                }
                if repeats(act) {
                    start_repeat(on, act);
                }
            };
            view! {
                <button class=format!("vk {class}") aria-label=aria
                    on:pointerdown=down on:pointerup=move |_| stop_repeat()
                    on:pointercancel=move |_| stop_repeat() on:pointerleave=move |_| stop_repeat()>
                    {label}
                </button>
            }
        };
    // 층·Shift·한글 같은 키보드 안 상태 키.
    let toggle = move |label: Signal<String>, aria: &'static str, on_: Signal<bool>, f: Flip| {
        view! {
            <button class="vk mod" class:on=on_ aria-label=aria aria-pressed=move || on_.get().to_string()
                on:pointerdown=move |e: PointerEvent| { e.prevent_default(); f(layer, shift, ko); if aria == "한글" { on.run(Act::Commit); } }>
                {label}
            </button>
        }
    };
    let txt = |s: &'static str| Signal::stored(s.to_string());
    let off = Signal::stored(false);

    let letters = move || {
        let rows = LETTERS.iter().enumerate().map(|(r, &(en, k, ks))| {
            let keys = en.chars().zip(k.chars().zip(ks.chars())).map(|(e, (j, js))| {
                let hid = 0x04 + (e as u16 - 'a' as u16);
                let ch = move || match (ko.get(), shift.get()) {
                    (true, false) => j,
                    (true, true) => js,
                    (false, s) => if s { e.to_ascii_uppercase() } else { e },
                };
                let label = Signal::derive(move || ch().to_string());
                // 글자는 누를 때의 한글·Shift 상태로 정해야 해서 Act 를 그 자리에서 만든다.
                view! {
                    <button class="vk" on:pointerdown=move |ev: PointerEvent| {
                        ev.prevent_default();
                        let c = ch();
                        on.run(Act::Char { ch: c, hid, shift: c != if ko.get_untracked() { j } else { e } });
                        shift.set(false);
                    }>{label}</button>
                }
            }).collect_view();
            match r {
                2 => view! {
                    <div class="vrow">
                        {toggle(txt("Shift"), "Shift", shift.into(), |_, s, _| s.update(|v| *v = !*v))}
                        {keys}
                        {key(txt("⌫"), Some("지우기"), "mod", Act::Backspace)}
                    </div>
                }.into_any(),
                1 => view! { <div class="vrow inset">{keys}</div> }.into_any(),
                _ => view! { <div class="vrow">{keys}</div> }.into_any(),
            }
        }).collect_view();
        view! {
            {rows}
            <div class="vrow">
                {toggle(txt("!#1"), "기호", off, |l, _, _| l.set(Layer::Symbols))}
                {toggle(txt("Fn"), "기능키", off, |l, _, _| l.set(Layer::Fn))}
                {toggle(txt("한글"), "한글", ko.into(), |_, _, k| k.update(|v| *v = !*v))}
                {key(txt("space"), Some("띄어쓰기"), "space", Act::Char { ch: ' ', hid: 0x2C, shift: false })}
                {key(txt("."), None, "", Act::Char { ch: '.', hid: 0x37, shift: false })}
                {key(txt("Enter"), None, "mod wide", Act::Key(HID_ENTER))}
            </div>
        }
    };
    let abc = Signal::derive(move || if ko.get() { "가" } else { "ABC" }.to_string());
    let symbols = move || {
        let rows = SYMBOLS
            .iter()
            .enumerate()
            .map(|(r, row)| {
                let keys = row
                    .iter()
                    .map(|&(ch, hid, shift)| {
                        key(
                            Signal::stored(ch.to_string()),
                            None,
                            "",
                            Act::Char { ch, hid, shift },
                        )
                    })
                    .collect_view();
                let back = (r == 3).then(|| key(txt("⌫"), Some("지우기"), "mod", Act::Backspace));
                view! { <div class="vrow">{keys}{back}</div> }
            })
            .collect_view();
        view! {
            {rows}
            <div class="vrow">
                {toggle(abc, "글자", off, |l, _, _| l.set(Layer::Letters))}
                {toggle(txt("Fn"), "기능키", off, |l, _, _| l.set(Layer::Fn))}
                {key(txt("`"), None, "", Act::Char { ch: '`', hid: 0x35, shift: false })}
                {key(txt("space"), Some("띄어쓰기"), "space", Act::Char { ch: ' ', hid: 0x2C, shift: false })}
                {key(txt("~"), None, "", Act::Char { ch: '~', hid: 0x35, shift: true })}
                {key(txt("Enter"), None, "mod wide", Act::Key(HID_ENTER))}
            </div>
        }
    };
    let fns = move || {
        let rows = FN_KEYS
            .iter()
            .map(|row| {
                let keys = row
                    .iter()
                    .map(|&(label, aria, hid)| {
                        let act = if hid == HID_BACKSPACE {
                            Act::Backspace
                        } else {
                            Act::Key(hid)
                        };
                        key(txt(label), (!aria.is_empty()).then_some(aria), "mod", act)
                    })
                    .collect_view();
                view! { <div class="vrow">{keys}</div> }
            })
            .collect_view();
        view! {
            {rows}
            <div class="vrow">
                {toggle(abc, "글자", off, |l, _, _| l.set(Layer::Letters))}
                {toggle(txt("!#1"), "기호", off, |l, _, _| l.set(Layer::Symbols))}
                {key(txt("space"), Some("띄어쓰기"), "space", Act::Char { ch: ' ', hid: 0x2C, shift: false })}
                {key(txt("Enter"), None, "mod wide", Act::Key(HID_ENTER))}
            </div>
        }
    };

    view! {
        <div class="vkbd" role="group" aria-label="가상 키보드" on:touchstart=|_| ()>
            {move || match layer.get() {
                Layer::Letters => letters().into_any(),
                Layer::Symbols => symbols().into_any(),
                Layer::Fn => fns().into_any(),
            }}
        </div>
    }
}

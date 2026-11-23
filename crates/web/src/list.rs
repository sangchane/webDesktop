//! 기기 목록(E-02·M-01)과 기기 추가(E-03) 화면 — 03 「화면 스케치」.
use crate::api;
use crate::errors::{self, Fail};
use crate::{AppState, Load, Screen};
use leptos::leptos_dom::helpers::set_interval_with_handle;
use leptos::prelude::*;
use leptos::task::spawn_local;
use proto::{Device, EnrollCode, Os, Perm};
use std::time::Duration;

/// 지난 시간 → 「3일 전」.
pub fn ago(ms: f64) -> String {
    let m = (ms / 60_000.0).floor().max(0.0) as u64;
    match m {
        0 => "방금".into(),
        1..=59 => format!("{m}분 전"),
        60..=1439 => format!("{}시간 전", m / 60),
        _ => format!("{}일 전", m / 1440),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tone {
    On,
    Off,
    Warn,
}

fn perm_need(p: Perm) -> Option<&'static str> {
    match p {
        Perm::Ok => None,
        Perm::NeedScreen => Some("화면 기록"),
        Perm::NeedAccess => Some("손쉬운 사용"),
        Perm::NeedLocalnet => Some("로컬 네트워크"),
        Perm::NeedMulti => Some("여러 권한"),
    }
}

/// 목록 둘째 줄 — 지금 누르면 무엇이 되나. `seen_ago` = 마지막 접속 「3일 전」.
pub fn describe(d: &Device, seen_ago: Option<&str>) -> (String, Tone) {
    let seen = seen_ago.map_or(String::new(), |a| format!(" · 마지막 접속 {a}"));
    match (d.online, perm_need(d.perm)) {
        (true, Some(p)) => (format!("권한 필요 — {p}"), Tone::Warn),
        (true, None) if d.connectable => ("켜짐 · 지금 열 수 있음".into(), Tone::On),
        (true, None) => ("켜짐 · 준비 중".into(), Tone::Warn),
        (false, _) if d.offline_long => (format!("하루 넘게 꺼짐{seen}"), Tone::Warn),
        (false, _) => (format!("꺼짐{seen}"), Tone::Off),
    }
}

fn seen_ago(d: &Device) -> Option<String> {
    let t = js_sys::Date::parse(d.last_seen_at.as_deref()?);
    (!t.is_nan()).then(|| ago(js_sys::Date::now() - t))
}

fn os_name(o: Os) -> &'static str {
    match o {
        Os::Windows => "Windows",
        Os::Macos => "macOS",
    }
}

fn open(st: AppState, d: Device) {
    if d.online && d.connectable {
        st.notice.set(None);
        st.screen.set(Screen::Remote {
            device: d,
            t0: api::now_ms(),
        });
        return;
    }
    let msg = match (d.online, perm_need(d.perm)) {
        (true, Some(p)) => format!("「{}」에서 {p} 권한을 다시 허용하세요", d.name),
        (true, None) => format!("「{}」가 아직 준비 중입니다. 잠시 뒤 다시 누르세요", d.name),
        (false, _) => format!(
            "「{}」는 꺼져 있습니다{}",
            d.name,
            seen_ago(&d).map_or(String::new(), |a| format!(" · 마지막 접속 {a}"))
        ),
    };
    st.notice.set(Some(msg));
}

fn row(st: AppState, d: Device) -> impl IntoView {
    let (sub, tone) = describe(&d, seen_ago(&d).as_deref());
    let (name, os, online) = (d.name.clone(), os_name(d.os), d.online);
    let go = d.online && d.connectable;
    view! {
        <li>
            <button class="device" data-device=d.id.clone() on:click=move |_| open(st, d.clone())>
                <span class="dot" class:on=online aria-hidden="true"></span>
                <span class="name">{name}</span>
                {go.then(|| view! { <span class="go" aria-hidden="true">"열기 ›"</span> })}
                <span class="sub">
                    <span class="os">{os}</span>" · "
                    <span class:warn=tone == Tone::Warn class:on=tone == Tone::On>{sub}</span>
                </span>
            </button>
        </li>
    }
}

#[component]
pub fn List() -> impl IntoView {
    let st = expect_context::<AppState>();
    let body = move || {
        match st.load.get() {
        Load::Loading => view! {
            <ul class="devices" aria-busy="true">
                <li class="skeleton"></li><li class="skeleton"></li><li class="skeleton"></li>
            </ul>
        }
        .into_any(),
        Load::Failed(f) => view! {
            <div class="empty">
                <p>{errors::text(f)}</p>
                <button class="primary" on:click=|_| { let _ = api::window().location().reload(); }>"다시 시도"</button>
            </div>
        }
        .into_any(),
        Load::Ready if st.devices.with(Vec::is_empty) => view! {
            <div class="empty"><p>"아직 기기가 없습니다. ＋ 를 눌러 첫 PC 를 추가하세요"</p></div>
        }
        .into_any(),
        Load::Ready => view! {
            <ul class="devices">{move || st.devices.get().into_iter().map(|d| row(st, d)).collect_view()}</ul>
        }
        .into_any(),
    }
    };
    view! {
        <main class="page">
            <header class="bar">
                <h1>"내 기기"</h1>
                <button class="icon" aria-label="기기 추가" on:click=move |_| st.screen.set(Screen::Add)>"＋"</button>
            </header>
            {move || st.hub.get().and_then(Result::err).map(|f| view! {
                <p class="stale" role="status">{errors::text(f)}" · 상태가 늦을 수 있습니다"</p>
            })}
            {move || st.notice.get().map(|n| view! {
                <p class="notice" role="status">
                    <span>{n}</span>
                    <button class="link" on:click=move |_| st.notice.set(None)>"닫기"</button>
                </p>
            })}
            {body}
        </main>
    }
}

/// 「ABCD2345」 → 「ABCD 2345」.
fn spaced(code: &str) -> String {
    let (a, b) = code.split_at(code.len() / 2);
    format!("{a} {b}")
}

#[component]
pub fn Add() -> impl IntoView {
    let st = expect_context::<AppState>();
    let code = RwSignal::new(None::<Result<EnrollCode, Fail>>);
    let now = RwSignal::new(js_sys::Date::now());
    let copied = RwSignal::new(false);
    let fetch = move || {
        code.set(None);
        copied.set(false);
        spawn_local(async move {
            code.set(Some(
                api::post::<EnrollCode>("/api/enroll-codes", &()).await,
            ));
        });
    };
    fetch();
    if let Ok(h) =
        set_interval_with_handle(move || now.set(js_sys::Date::now()), Duration::from_secs(1))
    {
        on_cleanup(move || h.clear());
    }
    let left = move |c: &EnrollCode| {
        let ms = js_sys::Date::parse(&c.expires_at) - now.get();
        if ms.is_nan() || ms < 1000.0 {
            return None;
        }
        // 허브와 폰 시계가 조금 달라도 수명보다 길게 보이지 않게.
        let s = ((ms / 1000.0).floor() as u64).min(proto::REG_CODE_TTL_MIN * 60);
        Some(format!("{}분 {}초 뒤 만료", s / 60, s % 60))
    };
    let body = move || match code.get() {
        None => view! { <p class="muted" aria-busy="true">"코드를 받는 중…"</p> }.into_any(),
        Some(Err(f)) => view! {
            <div class="empty">
                <p>{errors::text(f)}</p>
                <button class="primary" on:click=move |_| fetch()>"다시 받기"</button>
            </div>
        }
        .into_any(),
        Some(Ok(c)) => {
            let link = c.install_links.windows.clone();
            let copy = move |_| {
                let p = api::window().navigator().clipboard().write_text(&link);
                spawn_local(async move {
                    copied.set(wasm_bindgen_futures::JsFuture::from(p).await.is_ok());
                });
            };
            let c2 = c.clone();
            view! {
                <ol class="steps">
                    <li>
                        <p>"추가할 PC 에서 Windows 설치 파일을 받으세요."</p>
                        <div class="actions">
                            <a class="primary" href=c.install_links.windows.clone()>"설치 파일 받기"</a>
                            <button class="secondary" on:click=copy>{move || if copied.get() { "복사했습니다" } else { "링크 복사" }}</button>
                        </div>
                        <p class="muted">"폰에서 보고 있다면 링크를 복사해 그 PC 로 보내세요(메일·메신저). Mac 설치 파일은 아직 없습니다."</p>
                        <code class="link-text">{c.install_links.windows.clone()}</code>
                    </li>
                    <li><p>"받은 파일을 실행하세요."</p></li>
                    <li>
                        <p>"설치 프로그램이 코드를 물으면 이것을 넣으세요."</p>
                        <p class="code" data-code=c.code.clone()>{spaced(&c.code)}</p>
                        <p class="muted">
                            {move || left(&c2).unwrap_or_else(|| "만료됨. 새 코드를 받으세요".into())}
                        </p>
                        <button class="secondary" on:click=move |_| fetch()>"새 코드"</button>
                    </li>
                </ol>
                <p class="muted">"이 화면을 열어 두면 PC 가 붙는 즉시 목록으로 돌아갑니다."</p>
            }
            .into_any()
        }
    };
    view! {
        <main class="page">
            <header class="bar">
                <button class="link" on:click=move |_| st.screen.set(Screen::List)>"‹ 목록"</button>
                <h1>"기기 추가"</h1>
            </header>
            {body}
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(online: bool, perm: Perm, connectable: bool, offline_long: bool) -> Device {
        Device {
            id: "d1".into(),
            name: "집 PC".into(),
            os: Os::Windows,
            online,
            last_seen_at: None,
            perm,
            connectable,
            offline_long,
        }
    }

    #[test]
    fn ago_steps() {
        assert_eq!(ago(30_000.0), "방금");
        assert_eq!(ago(5.0 * 60_000.0), "5분 전");
        assert_eq!(ago(3.0 * 3_600_000.0), "3시간 전");
        assert_eq!(ago(3.0 * 86_400_000.0), "3일 전");
        assert_eq!(ago(-5.0), "방금");
    }

    #[test]
    fn describe_says_what_a_tap_does() {
        assert_eq!(
            describe(&dev(true, Perm::Ok, true, false), None),
            ("켜짐 · 지금 열 수 있음".into(), Tone::On)
        );
        assert_eq!(
            describe(&dev(true, Perm::NeedScreen, false, false), None).1,
            Tone::Warn
        );
        assert_eq!(
            describe(&dev(false, Perm::Ok, false, false), Some("3일 전")).0,
            "꺼짐 · 마지막 접속 3일 전"
        );
        assert_eq!(
            describe(&dev(false, Perm::Ok, false, true), None).1,
            Tone::Warn
        );
    }

    #[test]
    fn code_is_split_in_half() {
        assert_eq!(spaced("ABCD2345"), "ABCD 2345");
    }
}

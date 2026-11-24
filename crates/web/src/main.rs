//! 웹앱 — 화면·입력 변환·브라우저 WebRTC (04 「코드 구조」). 메시지는 `proto` 를 그대로 쓴다.
mod api;
mod errors;
mod hangul;
mod hub;
mod input;
mod list;
mod measure;
mod remote;
mod session;
mod vkbd;
mod zoom;

use errors::Fail;
use leptos::prelude::*;
use leptos::task::spawn_local;
use proto::{Device, WsMsg};
use std::cell::Cell;
use std::rc::Rc;

thread_local! {
    /// 목록 받기가 도는 중인가, 도는 중에 또 받아야 할 일이 생겼나.
    static REFRESH: Cell<(bool, bool)> = const { Cell::new((false, false)) };
    /// `?bench=N` 의 남은 열기 횟수.
    pub static BENCH: Cell<u32> = Cell::new(api::bench());
}

#[derive(Clone, PartialEq)]
pub enum Screen {
    List,
    Add,
    /// t0 = 기기를 누른 시각(첫 프레임까지 잰다).
    Remote {
        device: Device,
        t0: f64,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum Load {
    Loading,
    Ready,
    Failed(Fail),
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub screen: RwSignal<Screen>,
    pub devices: RwSignal<Vec<Device>>,
    pub load: RwSignal<Load>,
    /// 목록 위 한 줄 알림.
    pub notice: RwSignal<Option<String>>,
    /// 허브 WebSocket — None = 아직, Err = 끊김.
    pub hub: RwSignal<Option<Result<(), Fail>>>,
}

fn main() {
    leptos::mount::mount_to_body(App);
}

/// 목록을 다시 받는다. 기기 추가 화면에서 새 기기가 생기면 목록으로 돌아간다.
/// 한 번에 하나만 — 도는 중에 상태가 바뀌면 끝난 뒤 한 번 더 받는다(늦은 응답이 새 상태를 덮지 않게).
fn refresh(st: AppState) {
    if REFRESH.with(|r| r.replace((true, r.get().0)).0) {
        return;
    }
    spawn_local(async move {
        match api::get::<Vec<Device>>("/api/devices").await {
            Ok(v) => {
                let added = st.screen.with_untracked(|s| *s == Screen::Add).then(|| {
                    let old = st.devices.get_untracked();
                    v.iter()
                        .find(|d| !old.iter().any(|o| o.id == d.id))
                        .map(|d| d.name.clone())
                });
                st.devices.set(v);
                st.load.set(Load::Ready);
                if let Some(Some(name)) = added {
                    st.notice.set(Some(format!("「{name}」를 추가했습니다")));
                    st.screen.set(Screen::List);
                }
            }
            Err(f) => st.load.set(Load::Failed(f)),
        }
        if REFRESH.with(|r| r.replace((false, false)).1) {
            refresh(st);
        }
    });
}

fn on_msg(st: AppState, m: WsMsg) {
    match m {
        WsMsg::DeviceStatus {
            device_id,
            online,
            perm,
            connectable,
            last_seen_at,
        } => {
            let known = st
                .devices
                .with_untracked(|v| v.iter().any(|d| d.id == device_id));
            if !known || REFRESH.with(|r| r.get().0) {
                return refresh(st);
            }
            st.devices.update(|v| {
                for d in v.iter_mut().filter(|d| d.id == device_id) {
                    d.online = online;
                    d.perm = perm;
                    d.connectable = connectable;
                    d.last_seen_at = last_seen_at.clone();
                }
            });
        }
        other => session::on_hub(other),
    }
}

#[component]
fn App() -> impl IntoView {
    let st = AppState {
        screen: RwSignal::new(Screen::List),
        devices: RwSignal::new(vec![]),
        load: RwSignal::new(Load::Loading),
        notice: RwSignal::new(None),
        hub: RwSignal::new(None),
    };
    provide_context(st);
    session::close_on_pagehide();
    refresh(st);
    hub::start(
        Rc::new(move |m| on_msg(st, m)),
        Rc::new(move |s| st.hub.set(Some(s))),
    );
    // ?bench=N — 목록에 오면 열 수 있는 첫 기기를 저절로 연다. 닫고 돌아오는 것은 remote.rs.
    Effect::new(move |_| {
        if st.screen.get() != Screen::List || st.load.get() != Load::Ready {
            return;
        }
        let Some(device) = st
            .devices
            .with(|v| v.iter().find(|d| d.connectable).cloned())
        else {
            return;
        };
        if BENCH.with(|b| b.get()) == 0 {
            return;
        }
        BENCH.with(|b| b.set(b.get() - 1));
        st.screen.set(Screen::Remote {
            device,
            t0: api::now_ms(),
        });
    });
    move || match st.screen.get() {
        Screen::List => view! { <list::List/> }.into_any(),
        Screen::Add => view! { <list::Add/> }.into_any(),
        Screen::Remote { device, t0 } => view! { <remote::Remote device t0/> }.into_any(),
    }
}

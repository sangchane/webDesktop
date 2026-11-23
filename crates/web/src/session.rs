//! 원격 세션 — E-07 로 세션을 열고, 뷰어가 offer 를 만든다(영상은 받기만, DataChannel `input`·`control` 은 뷰어가 연다).
//! 신호는 허브 WS(M-03·M-04). 계측(M-10): 누른 때부터 첫 프레임, 경로(선택된 후보 쌍), 재연결 시간, 입력 지연 p50.
use crate::api::{self, window};
use crate::errors::{self, Fail};
use crate::hub;
use crate::measure::{self, Meter};
use js_sys::{Function, Reflect, JSON};
use leptos::prelude::*;
use proto::{parse, Control, EndReason, ErrorType, Input, NetPath, SessionReq, SessionResp, WsMsg};
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    HtmlVideoElement, MessageEvent, RtcDataChannel, RtcDataChannelState, RtcIceConnectionState,
    RtcPeerConnection, RtcPeerConnectionIceEvent, RtcTrackEvent,
};

/// 계측을 허브에 다시 보내는 주기(06 — 1분 동안 모은 p50).
const REPORT_EVERY_MS: i32 = 60_000;

#[derive(Clone, PartialEq, Debug)]
pub enum Phase {
    Connecting,
    Live,
    /// 연결이 잠깐 끊겨 브라우저가 다시 잇는 중.
    Lost,
    /// 끝남 — 보일 문구(내가 닫았으면 빈 글).
    Ended(&'static str),
}

#[derive(Clone, Copy)]
pub struct Ui {
    pub phase: RwSignal<Phase>,
    pub path: RwSignal<Option<NetPath>>,
}

struct Live {
    session_id: String,
    pc: RtcPeerConnection,
    input: RtcDataChannel,
    ui: Ui,
    t0: f64,
    first_frame_ms: Cell<Option<u32>>,
    path: Cell<Option<NetPath>>,
    reconnect_ms: RefCell<Vec<u32>>,
    lost_at: Cell<Option<f64>>,
    meter: RefCell<Meter>,
    timer: Cell<i32>,
    /// answer 를 넣기 전에 온 후보 — 넣은 뒤 차례로 붙인다.
    early: RefCell<Option<Vec<String>>>,
}

thread_local! {
    static LIVE: RefCell<Option<Rc<Live>>> = const { RefCell::new(None) };
    /// 여는 중에 화면을 나가면 늘어난다 — 늦게 끝난 열기를 버리게.
    static GEN: Cell<u32> = const { Cell::new(0) };
    /// 응답을 기다리는 세션 요청 수.
    static INFLIGHT: Cell<u32> = const { Cell::new(0) };
}

fn current() -> Option<Rc<Live>> {
    LIVE.with(|l| l.borrow().clone())
}

fn js(v: &serde_json::Value) -> JsValue {
    JSON::parse(&v.to_string()).unwrap_or(JsValue::NULL)
}

fn internal<T>(_: T) -> Fail {
    Fail::Hub(ErrorType::Internal)
}

/// 탭을 닫거나 새로 고치면 세션을 닫는다 — 같은 뷰어의 다음 탭이 같은 세션을 받아 앞 탭과 엉키지 않게(05 「뷰어 식별」).
pub fn close_on_pagehide() {
    let f = Closure::<dyn Fn()>::new(close);
    let _ =
        window().add_event_listener_with_callback("pagehide", f.into_js_value().unchecked_ref());
}

/// 기기를 누른 시각 `t0`(performance.now) 부터 잰다.
pub async fn open(device_id: String, t0: f64, video: HtmlVideoElement, ui: Ui) {
    let gen = GEN.with(Cell::get);
    if let Err(f) = try_open(device_id, t0, video, ui, gen).await {
        // 그사이 화면을 나갔으면 이 실패는 지금 세션과 상관없다.
        if GEN.with(Cell::get) != gen {
            return;
        }
        teardown(Some(errors::text(f)));
        ui.phase.set(Phase::Ended(errors::text(f)));
    }
}

async fn try_open(
    device_id: String,
    t0: f64,
    video: HtmlVideoElement,
    ui: Ui,
    gen: u32,
) -> Result<(), Fail> {
    INFLIGHT.with(|n| n.set(n.get() + 1));
    let s: Result<SessionResp, Fail> = api::post(
        "/api/sessions",
        &SessionReq {
            device_id,
            viewer_id: api::viewer_id(),
        },
    )
    .await;
    INFLIGHT.with(|n| n.set(n.get() - 1));
    let s = s?;
    if GEN.with(Cell::get) != gen {
        // 같은 뷰어의 새 열기는 이 세션을 다시 받는다(200 재사용) — 그쪽이 있으면 닫지 않는다.
        if INFLIGHT.with(Cell::get) == 0 && current().is_none() {
            hub::send(&WsMsg::SessionEnd {
                session_id: s.session_id,
                reason: EndReason::ViewerClosed,
            });
        }
        return Ok(());
    }
    let policy = if api::relay_only() { "relay" } else { "all" };
    let cfg = js(&json!({ "iceServers": s.ice_servers, "iceTransportPolicy": policy }));
    let pc = RtcPeerConnection::new_with_configuration(cfg.unchecked_ref()).map_err(internal)?;
    pc.add_transceiver_with_str_and_init(
        "video",
        js(&json!({"direction": "recvonly"})).unchecked_ref(),
    );
    let input = pc.create_data_channel("input");
    let control = pc.create_data_channel("control");
    let live = Rc::new(Live {
        session_id: s.session_id.clone(),
        pc: pc.clone(),
        input,
        ui,
        t0,
        first_frame_ms: Cell::new(None),
        path: Cell::new(None),
        reconnect_ms: RefCell::default(),
        lost_at: Cell::new(None),
        meter: RefCell::default(),
        timer: Cell::new(0),
        early: RefCell::new(Some(vec![])),
    });
    LIVE.with(|l| *l.borrow_mut() = Some(live.clone()));

    // D-08 — 입력이 반영된 프레임의 RTP 타임스탬프.
    let on_control = Closure::<dyn Fn(MessageEvent)>::new(|e: MessageEvent| {
        let (Some(live), Some(t)) = (current(), e.data().as_string()) else {
            return;
        };
        if let Ok(Control::InputApplied { seq, rtp_ts }) = parse::<Control>(&t) {
            live.meter.borrow_mut().applied(seq, rtp_ts);
        }
    });
    control.set_onmessage(Some(on_control.into_js_value().unchecked_ref()));

    let sid = s.session_id.clone();
    let on_cand =
        Closure::<dyn Fn(RtcPeerConnectionIceEvent)>::new(move |e: RtcPeerConnectionIceEvent| {
            if let Some(c) = e
                .candidate()
                .map(|c| c.candidate())
                .filter(|c| !c.is_empty())
            {
                hub::send(&WsMsg::SignalCandidate {
                    session_id: sid.clone(),
                    candidate: c,
                });
            }
        });
    pc.set_onicecandidate(Some(on_cand.into_js_value().unchecked_ref()));

    let v = video.clone();
    let on_track = Closure::<dyn Fn(RtcTrackEvent)>::new(move |e: RtcTrackEvent| {
        // 보낸 쪽이 stream 을 안 붙였을 수도 있어 트랙으로 직접 만든다.
        if let Ok(stream) = web_sys::MediaStream::new_with_tracks(&js_sys::Array::of1(&e.track())) {
            v.set_src_object(Some(&stream));
            let _ = v.play();
        }
    });
    pc.set_ontrack(Some(on_track.into_js_value().unchecked_ref()));

    let on_ice = Closure::<dyn Fn()>::new(on_ice_state);
    pc.set_oniceconnectionstatechange(Some(on_ice.into_js_value().unchecked_ref()));

    watch_frames(&video);

    let report = Closure::<dyn Fn()>::new(|| {
        if let Some(l) = current() {
            report(&l);
        }
    });
    let id = window()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            report.into_js_value().unchecked_ref(),
            REPORT_EVERY_MS,
        )
        .unwrap_or(0);
    live.timer.set(id);

    let offer = JsFuture::from(pc.create_offer()).await.map_err(internal)?;
    let sdp = Reflect::get(&offer, &"sdp".into())
        .ok()
        .and_then(|v| v.as_string())
        .ok_or(Fail::Hub(ErrorType::Internal))?;
    JsFuture::from(pc.set_local_description(offer.unchecked_ref()))
        .await
        .map_err(internal)?;
    if !hub::send(&WsMsg::SignalOffer {
        session_id: s.session_id,
        sdp,
    }) {
        return Err(Fail::Network);
    }
    Ok(())
}

fn on_ice_state() {
    let Some(l) = current() else { return };
    match l.pc.ice_connection_state() {
        RtcIceConnectionState::Disconnected => {
            l.lost_at.set(Some(api::now_ms()));
            l.ui.phase.set(Phase::Lost);
        }
        RtcIceConnectionState::Connected | RtcIceConnectionState::Completed => {
            if let Some(t) = l.lost_at.take() {
                l.reconnect_ms.borrow_mut().push((api::now_ms() - t) as u32);
            }
            if l.first_frame_ms.get().is_some() {
                l.ui.phase.set(Phase::Live);
            }
            let l2 = l.clone();
            spawn_local(async move {
                let p = read_path(&l2.pc).await;
                if current().is_none_or(|c| !Rc::ptr_eq(&c, &l2)) {
                    return;
                }
                l2.path.set(p);
                l2.ui.path.set(p);
                report(&l2);
            });
        }
        RtcIceConnectionState::Failed => {
            // shortcut: ICE 재시작·새 세션 1회(FR-010)는 아직 없다 — 에이전트가 재시작을 받을 수 있게 된 뒤(T9) 넣는다.
            let msg = errors::ended(EndReason::Failed).unwrap_or_default();
            hub::send(&WsMsg::SessionEnd {
                session_id: l.session_id.clone(),
                reason: EndReason::Failed,
            });
            teardown(Some(msg));
        }
        _ => {}
    }
}

/// 그려진 프레임마다 — 첫 프레임 시각과 RTP 타임스탬프(SC-003). `requestVideoFrameCallback` 이 없으면 첫 프레임만 잰다.
fn watch_frames(video: &HtmlVideoElement) {
    let Some(rvfc) = Reflect::get(video, &"requestVideoFrameCallback".into())
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
    else {
        let on_play = Closure::<dyn Fn()>::new(|| {
            if let Some(l) = current() {
                first_frame(&l, api::now_ms());
            }
        });
        video.set_onplaying(Some(on_play.into_js_value().unchecked_ref()));
        return;
    };
    type Cb = Closure<dyn FnMut(f64, JsValue)>;
    let cb: Rc<RefCell<Option<Cb>>> = Rc::default();
    let (again, v, first) = (cb.clone(), video.clone(), rvfc.clone());
    // shortcut: 스스로를 다시 거는 처리기라 세션마다 하나씩 남는다(작음), 세션을 수백 번 열고 닫는 탭이면 약한 참조로 바꾼다.
    *cb.borrow_mut() = Some(Closure::new(move |now: f64, meta: JsValue| {
        let Some(l) = current() else { return };
        let num = |k: &str| Reflect::get(&meta, &k.into()).ok().and_then(|x| x.as_f64());
        let shown = num("expectedDisplayTime").unwrap_or(now);
        first_frame(&l, shown);
        if let Some(rtp) = num("rtpTimestamp") {
            l.meter.borrow_mut().drawn(rtp as u32, shown);
        }
        if let Some(c) = again.borrow().as_ref() {
            let _ = rvfc.call1(&v, c.as_ref());
        }
    }));
    if let Some(c) = cb.borrow().as_ref() {
        let _ = first.call1(video, c.as_ref());
    };
}

fn first_frame(l: &Rc<Live>, t: f64) {
    if l.first_frame_ms.get().is_some() {
        return;
    }
    l.first_frame_ms.set(Some((t - l.t0).max(0.0) as u32));
    l.ui.phase.set(Phase::Live);
    report(l);
}

/// 선택된 후보 쌍의 종류 — 어느 쪽이든 relay 면 중계.
async fn read_path(pc: &RtcPeerConnection) -> Option<NetPath> {
    let report = JsFuture::from(pc.get_stats()).await.ok()?;
    let values: Function = Reflect::get(&report, &"values".into())
        .ok()?
        .dyn_into()
        .ok()?;
    let all: Vec<JsValue> = js_sys::try_iter(&values.call0(&report).ok()?)
        .ok()??
        .filter_map(Result::ok)
        .collect();
    let s = |o: &JsValue, k: &str| Reflect::get(o, &k.into()).ok().and_then(|v| v.as_string());
    let b = |o: &JsValue, k: &str| {
        Reflect::get(o, &k.into()).ok().and_then(|v| v.as_bool()) == Some(true)
    };
    let of_type = |t: &'static str| {
        all.iter()
            .filter(move |x| s(x, "type").as_deref() == Some(t))
    };
    // Chrome·Safari 는 transport 가 고른 쌍을, Firefox 는 쌍의 selected 를 준다.
    let chosen = of_type("transport").find_map(|t| s(t, "selectedCandidatePairId"));
    let pair = of_type("candidate-pair").find(|p| match &chosen {
        Some(id) => s(p, "id").as_ref() == Some(id),
        None => {
            b(p, "selected") || (b(p, "nominated") && s(p, "state").as_deref() == Some("succeeded"))
        }
    })?;
    let relay = |key: &str| {
        let id = s(pair, key);
        all.iter().any(|c| {
            id.is_some() && s(c, "id") == id && s(c, "candidateType").as_deref() == Some("relay")
        })
    };
    Some(if relay("localCandidateId") || relay("remoteCandidateId") {
        NetPath::Relay
    } else {
        NetPath::Direct
    })
}

/// M-10 — 허브는 같은 세션의 앞 값을 덮어쓴다.
fn report(l: &Live) {
    hub::send(&WsMsg::ViewerStats {
        session_id: l.session_id.clone(),
        click_to_first_frame_ms: l.first_frame_ms.get(),
        latency_p50_ms: l.meter.borrow().p50(),
        reconnect_ms: l.reconnect_ms.borrow().clone(),
        restarts: 0,
        new_session_used: false,
        path: l.path.get(),
    });
}

/// 허브가 보낸 신호·종료(M-03·M-04)와 오류.
pub fn on_hub(msg: WsMsg) {
    let Some(l) = current() else { return };
    match msg {
        WsMsg::SignalAnswer { session_id, sdp } if session_id == l.session_id => {
            let desc = js(&json!({"type": "answer", "sdp": sdp}));
            let p = l.pc.set_remote_description(desc.unchecked_ref());
            spawn_local(async move {
                if JsFuture::from(p).await.is_err() {
                    return end_if(&session_id, errors::text(Fail::Hub(ErrorType::BadRequest)));
                }
                for c in l.early.borrow_mut().take().unwrap_or_default() {
                    add_candidate(&l.pc, c);
                }
            });
        }
        WsMsg::SignalCandidate {
            session_id,
            candidate,
        } if session_id == l.session_id => match l.early.borrow_mut().as_mut() {
            Some(v) => v.push(candidate),
            None => add_candidate(&l.pc, candidate),
        },
        WsMsg::SessionEnd { session_id, reason } if session_id == l.session_id => {
            teardown(Some(errors::ended(reason).unwrap_or_default()));
        }
        // 허브 오류에는 세션 id 가 없다 — 첫 연결 전에만 이 세션의 실패로 본다(붙은 뒤엔 WebRTC 가 허브 없이 이어진다).
        WsMsg::Error {
            kind: kind @ (ErrorType::DeviceOffline | ErrorType::NotFound),
            ..
        } if l.ui.phase.get_untracked() == Phase::Connecting => {
            teardown(Some(errors::text(Fail::Hub(kind))))
        }
        _ => {}
    }
}

fn add_candidate(pc: &RtcPeerConnection, candidate: String) {
    // BUNDLE 이라 모든 후보가 첫 m 줄에 붙는다.
    let c = js(&json!({"candidate": candidate, "sdpMLineIndex": 0}));
    let p = pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(c.unchecked_ref()));
    spawn_local(async move {
        let _ = JsFuture::from(p).await;
    });
}

/// 지금 세션이 `sid` 일 때만 정리한다(늦게 끝난 비동기 일이 다음 세션을 닫지 않게).
fn end_if(sid: &str, msg: &'static str) {
    if current().is_some_and(|l| l.session_id == sid) {
        teardown(Some(msg));
    }
}

/// 입력(D-01~D-03)을 보낸다. 재는 입력은 보낸 시각을 남긴다.
pub fn send(inputs: Vec<Input>) {
    let Some(l) = current() else { return };
    if l.input.ready_state() != RtcDataChannelState::Open {
        return;
    }
    let now = api::now_ms();
    for i in inputs {
        if let Some(seq) = measure::timed(&i) {
            l.meter.borrow_mut().sent(seq, now);
        }
        if let Ok(s) = serde_json::to_string(&i) {
            let _ = l.input.send_with_str(&s);
        }
    }
}

/// 내가 닫는다(연결 끊기·화면 나감).
pub fn close() {
    GEN.with(|g| g.set(g.get().wrapping_add(1)));
    if let Some(l) = current() {
        hub::send(&WsMsg::SessionEnd {
            session_id: l.session_id.clone(),
            reason: EndReason::ViewerClosed,
        });
    }
    teardown(None);
}

/// 연결을 정리한다. `msg` 가 있으면 화면에 끝난 이유를 보인다.
fn teardown(msg: Option<&'static str>) {
    let Some(l) = LIVE.with(|x| x.borrow_mut().take()) else {
        return;
    };
    report(&l);
    window().clear_interval_with_handle(l.timer.get());
    l.pc.close();
    if let Some(m) = msg {
        l.ui.phase.set(Phase::Ended(m));
    }
}

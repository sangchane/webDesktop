//! Claude Code 권한 요청 카드(FR-023) — 어느 화면에서든 아래에 뜬다. 허용·거부를 허브로 보낸다.
use crate::{api, hub, AppState};
use leptos::leptos_dom::helpers::{set_interval_with_handle, set_timeout};
use leptos::prelude::*;
use proto::WsMsg;
use std::time::Duration;

/// 03 ASK_ARM_MS — 카드가 바뀐 뒤 단추를 잠그는 시간. 두 번 누르기의 두 번째 탭이 새로 올라온 다른 요청을 허용하지 않게.
const ARM_MS: u64 = 700;

#[derive(Clone, PartialEq, Debug)]
pub struct Ask {
    pub id: String,
    pub device_id: String,
    pub project: String,
    pub tool: String,
    pub detail: String,
    /// 명령이 잘렸다 — 다 못 봤으니 허용 단추를 막는다.
    pub truncated: bool,
    /// 에이전트가 기다리는 끝 시각(이 브라우저 시계).
    pub until_ms: f64,
}

/// 허브 글 중 권한 요청이면 목록에 반영하고 true.
pub fn on_hub(st: AppState, m: &WsMsg) -> bool {
    match m {
        WsMsg::AskNew {
            ask_id,
            device_id,
            project,
            tool,
            detail,
            truncated,
            wait_sec,
        } => {
            let a = Ask {
                id: ask_id.clone(),
                device_id: device_id.clone(),
                project: project.clone(),
                tool: tool.clone(),
                detail: detail.clone(),
                truncated: *truncated,
                until_ms: api::now_ms() + *wait_sec as f64 * 1000.0,
            };
            // ask.sync·재접속으로 같은 요청이 또 온다 — 하나만.
            st.asks.update(|v| {
                if !v.iter().any(|x| x.id == a.id) {
                    v.push(a)
                }
            });
            true
        }
        WsMsg::AskDone { ask_id, .. } => {
            st.asks.update(|v| v.retain(|x| &x.id != ask_id));
            true
        }
        _ => false,
    }
}

fn answer(st: AppState, a: &Ask, allow: bool) {
    let sent = hub::send(&WsMsg::AskAnswer {
        ask_id: a.id.clone(),
        device_id: a.device_id.clone(),
        allow,
    });
    if sent {
        st.asks.update(|v| v.retain(|x| x.id != a.id));
    } else {
        st.notice.set(Some(
            "허브와 끊겨 답을 보내지 못했습니다. 잠시 뒤 다시 누르세요".into(),
        ));
    }
}

#[component]
pub fn Asks() -> impl IntoView {
    let st = expect_context::<AppState>();
    let now = RwSignal::new(api::now_ms());
    // 남은 초 표시와 지난 카드 치우기 — 에이전트가 시간 초과면 ask.done 을 보내지만 끊긴 동안은 못 받는다.
    if let Ok(h) = set_interval_with_handle(
        move || {
            let t = api::now_ms();
            now.set(t);
            if st
                .asks
                .with_untracked(|v| v.iter().any(|a| a.until_ms <= t))
            {
                st.asks.update(|v| v.retain(|a| a.until_ms > t));
            }
        },
        Duration::from_secs(1),
    ) {
        on_cleanup(move || h.clear());
    }
    // 맨 앞 카드가 바뀔 때만 다시 그린다 — 뒤에 요청이 붙거나 기기 상태가 와도 보고 있는 카드(스크롤·잠금)는 그대로.
    let first = Memo::new(move |_| st.asks.with(|v| v.first().cloned()));
    let more = move || st.asks.with(|v| v.len().saturating_sub(1));
    move || first.get().map(|a| card(st, a, now, more))
}

fn card(
    st: AppState,
    a: Ask,
    now: RwSignal<f64>,
    more: impl Fn() -> usize + Send + Sync + 'static,
) -> impl IntoView {
    let armed = RwSignal::new(false);
    set_timeout(
        move || {
            let _ = armed.try_set(true);
        },
        Duration::from_millis(ARM_MS),
    );
    let pc = st
        .devices
        .with_untracked(|d| {
            d.iter()
                .find(|d| d.id == a.device_id)
                .map(|d| d.name.clone())
        })
        .unwrap_or_else(|| "PC".into());
    // 1초 시계는 카드보다 먼저 찍혔을 수 있다 — 지금 시각과 큰 쪽으로.
    let until = a.until_ms;
    let left = move || {
        ((until - now.get().max(api::now_ms())) / 1000.0)
            .ceil()
            .max(0.0) as u32
    };
    let truncated = a.truncated;
    let (deny, allow) = (a.clone(), a.clone());
    let place = if a.project.is_empty() {
        pc
    } else {
        format!("{pc} · {}", a.project)
    };
    view! {
        <section class="ask" role="alertdialog" aria-labelledby="ask-title" aria-describedby="ask-detail">
            <p class="muted">{place}</p>
            <p id="ask-title" class="ask-title">
                "Claude Code 가 "<b>{a.tool.clone()}</b>" 을(를) 쓰려고 합니다"
                <span class="muted">" · "{left}"초"</span>
            </p>
            <pre id="ask-detail" class="ask-detail">{a.detail.clone()}</pre>
            {truncated.then(|| view! { <p class="warn">"명령이 길어 다 보이지 않습니다. 허용은 PC 에서 하세요"</p> })}
            <div class="ask-actions">
                <button class="secondary" data-ask="deny" disabled=move || !armed.get()
                    on:click=move |_| answer(st, &deny, false)>"거부"</button>
                <button class="primary" data-ask="allow" disabled=move || !armed.get() || truncated
                    on:click=move |_| answer(st, &allow, true)>"허용"</button>
            </div>
            {move || (more() > 0).then(|| view! { <p class="muted">"기다리는 요청 "{more()}"개 더"</p> })}
        </section>
    }
}

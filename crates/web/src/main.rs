//! 웹앱 — 화면·입력 변환·브라우저 WebRTC (04 「코드 구조」). 메시지는 `proto` 를 그대로 쓴다.
use leptos::prelude::*;

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    view! { <p>"WebDesktop — 준비 중 (proto " {proto::PROTO_MAJOR} ")"</p> }
}

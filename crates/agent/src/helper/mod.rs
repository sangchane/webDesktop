//! 도우미 계층 — 세션 하나: str0m 피어, 캡처 → 인코딩 → 전송, 입력 주입 (04 「코드 구조」 helper).
//! 허브에 직접 붙지 않는다. 신호는 상주(resident)가 넘겨주고(Cmd), 보낼 신호는 out 으로 돌려준다.
mod stun;
mod video;

use crate::platform::{capture::Capturer, inject::Injector};
use proto::{Control, EndReason, IceServer, Input, WsMsg};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use str0m::change::SdpOffer;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{MediaKind, MediaTime, Mid, Pt};
use str0m::net::{Protocol, Receive};
use str0m::rtp::Extension;
use str0m::{Candidate, Event, IceConnectionState, Output, Rtc};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};
use video::{Video, INPUT_SETTLE_MS, VIDEO_FPS, VIDEO_IDLE_RESEND_MS};

/// 상주 → 도우미.
pub enum Cmd {
    Offer(String),
    Candidate(String),
    /// 허브가 세션을 닫음(뷰어가 닫음·이어받기) — 알리지 않고 끝낸다.
    End,
}

/// 03 상수 표 — 처음 연결까지, 또는 끊긴 채로 이만큼 지나면 세션을 끝낸다(캡처·인코딩·화면 복제를 놓는다).
pub const SESSION_DEAD_SEC: u64 = 30;

/// 세션 실을 띄운다. 끝나면(허브가 닫은 경우 빼고) session.end 를 out 으로 보낸다.
/// out 의 세대 번호(gen)로 상주가 같은 세션의 앞 도우미가 늦게 보낸 글을 가려 버린다.
pub fn start(
    session_id: String,
    ice: Vec<IceServer>,
    gen: u64,
    out: UnboundedSender<(u64, WsMsg)>,
) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let send = |m: WsMsg| {
            let _ = out.send((gen, m));
        };
        // panic 해도 뷰어·허브가 세션을 산 것으로 두지 않게(키는 Injector 의 Drop 이 놓는다).
        let reason = match catch_unwind(AssertUnwindSafe(|| run(&session_id, &ice, rx, &send))) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                warn!(event = "session.failed", session_id, why = %e);
                Some(EndReason::Failed)
            }
            Err(_) => {
                warn!(event = "session.failed", session_id, why = "panic");
                Some(EndReason::Failed)
            }
        };
        info!(event = "session.ended", session_id, reason = ?reason);
        if let Some(reason) = reason {
            send(WsMsg::SessionEnd { session_id, reason });
        }
    });
    tx
}

/// 밖으로 나가는 길의 내 주소(패킷은 보내지 않는다 — 경로만 고른다).
fn local_ip() -> std::io::Result<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.connect("192.0.2.1:9")?; // TEST-NET-1(RFC 5737)
    Ok(s.local_addr()?.ip())
}

/// ice_servers 의 첫 stun: 주소(IPv4).
fn stun_server(ice: &[IceServer]) -> Option<SocketAddr> {
    ice.iter()
        .flat_map(|s| &s.urls)
        .filter_map(|u| u.strip_prefix("stun:"))
        .filter_map(|hp| hp.split('?').next())
        .find_map(|hp| hp.to_socket_addrs().ok()?.find(SocketAddr::is_ipv4))
}

/// 브라우저가 고른 H.264 중 openh264 가 내는 꼴(constrained baseline, packetization-mode 1)을 우선.
fn pick_pt(rtc: &mut Rtc, mid: Mid) -> Option<Pt> {
    let w = rtc.writer(mid)?;
    let h264: Vec<_> = w
        .payload_params()
        .filter(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
        .map(|p| (p.pt(), p.spec().format.profile_level_id))
        .collect();
    h264.iter()
        .find(|(_, prof)| prof.is_some_and(|v| v >> 8 == 0x42e0))
        .or(h264.first())
        .map(|(pt, _)| *pt)
}

fn is_timed(i: &Input) -> Option<u32> {
    // 웹앱 measure::timed 와 같은 기준 — 누름·글자만 잰다.
    match i {
        Input::MouseButton {
            seq, down: true, ..
        }
        | Input::Key {
            seq, down: true, ..
        }
        | Input::Text { seq, .. } => Some(*seq),
        _ => None,
    }
}

#[derive(Default)]
struct Chans {
    input: Option<ChannelId>,
    control: Option<ChannelId>,
}

fn run(
    session_id: &str,
    ice: &[IceServer],
    rx: Receiver<Cmd>,
    out: &dyn Fn(WsMsg),
) -> Result<Option<EndReason>, Box<dyn std::error::Error>> {
    let sock = UdpSocket::bind((local_ip()?, 0))?;
    let local = sock.local_addr()?;
    let mut rtc = Rtc::builder()
        .clear_codecs()
        .enable_h264(true)
        // 뷰어 jitter 버퍼를 최소로(playout-delay 0/0) — 뷰어 offer 에 있으면 번호는 str0m 이 맞춘다.
        .set_extension(12, Extension::PlayoutDelay)
        .build(Instant::now());
    rtc.add_local_candidate(Candidate::host(local, "udp")?);
    match stun_server(ice).and_then(|s| stun::query(&sock, s)) {
        Some(mapped) if mapped != local => {
            rtc.add_local_candidate(Candidate::server_reflexive(mapped, local, "udp")?);
            info!(event = "session.srflx", session_id, ok = true);
        }
        _ => info!(event = "session.srflx", session_id, ok = false),
    }
    // 연결 뒤에 만든다 — 바로 앞 세션의 도우미가 아직 복제를 쥐고 있으면 다음 프레임에 다시.
    let mut cap: Option<Capturer> = None;
    let mut cap_retry = Instant::now();
    let mut cap_warned = false;
    let mut video = Video::new();
    let mut inj = Injector::default();
    let mut chans = Chans::default();
    let mut mid: Option<Mid> = None;
    let mut pt: Option<Pt> = None;
    let mut connected = false;
    let mut answered = false;
    let mut early: Vec<String> = vec![];
    // (seq, 주입 시각) — 주입 뒤 한 프레임 간격이 지난 캡처에 붙인다(그 전 캡처엔 주입 전 화면이 쌓여 있다).
    let mut pending: Vec<(u32, Instant)> = vec![];
    let t0 = Instant::now();
    let dead = Duration::from_secs(SESSION_DEAD_SEC);
    // 연결 전도 「끊김」으로 센다 — offer 가 안 오거나 ICE 가 한 번도 안 붙어도 끝나게.
    let mut down_since = Some(t0);
    let every = Duration::from_millis(1000 / VIDEO_FPS as u64);
    let idle = Duration::from_millis(VIDEO_IDLE_RESEND_MS);
    let settle = Duration::from_millis(INPUT_SETTLE_MS);
    let mut next_frame = Instant::now();
    let mut last_sent: Option<Instant> = None;
    // UDP 최대 크기 — 작은 버퍼는 Windows 에서 큰 데이터그램 하나에 오류(WSAEMSGSIZE)를 낸다.
    let mut buf = vec![0u8; 65536];

    loop {
        // 1. 상주가 넘긴 신호.
        loop {
            match rx.try_recv() {
                Ok(Cmd::Offer(sdp)) => {
                    // 첫 offer, 또는 ICE 재시작 뒤의 새 offer.
                    let answer = rtc
                        .sdp_api()
                        .accept_offer(SdpOffer::from_sdp_string(&sdp)?)?;
                    out(WsMsg::SignalAnswer {
                        session_id: session_id.to_owned(),
                        sdp: answer.to_sdp_string(),
                    });
                    answered = true;
                    for c in early.drain(..) {
                        if let Ok(c) = Candidate::from_sdp_string(&c) {
                            rtc.add_remote_candidate(c);
                        }
                    }
                }
                Ok(Cmd::Candidate(c)) if answered => match Candidate::from_sdp_string(&c) {
                    Ok(c) => rtc.add_remote_candidate(c),
                    Err(e) => warn!(event = "session.bad_candidate", why = %e),
                },
                Ok(Cmd::Candidate(c)) => early.push(c),
                // 눌린 키·버튼은 inj 의 Drop 이 놓는다.
                Ok(Cmd::End) | Err(TryRecvError::Disconnected) => return Ok(None),
                Err(TryRecvError::Empty) => break,
            }
        }

        // 2. str0m 이 내보낼 것.
        let timeout = loop {
            match rtc.poll_output()? {
                Output::Transmit(t) => {
                    let _ = sock.send_to(&t.contents, t.destination);
                }
                Output::Timeout(t) => break t,
                Output::Event(e) => match e {
                    Event::IceConnectionStateChange(s) => {
                        info!(event = "session.ice", session_id, state = ?s);
                        match s {
                            IceConnectionState::Connected | IceConnectionState::Completed => {
                                down_since = None
                            }
                            IceConnectionState::Disconnected => {
                                // 뷰어가 ICE 재시작이나 새 세션으로 돌아온다(FR-010). 눌린 채 남지 않게.
                                inj.release_all();
                                down_since.get_or_insert_with(Instant::now);
                            }
                            _ => {}
                        }
                    }
                    // 뷰어가 정상으로 닫음 — 뷰어가 session.end 를 이미 보냈다.
                    Event::Closed => return Ok(None),
                    Event::Connected => {
                        connected = true;
                        video.keyframe = true;
                        info!(event = "session.connected", session_id);
                    }
                    Event::MediaAdded(m) if m.kind == MediaKind::Video => {
                        mid = Some(m.mid);
                        pt = pick_pt(&mut rtc, m.mid);
                        if pt.is_none() {
                            warn!(event = "session.no_h264", session_id);
                            return Ok(Some(EndReason::Failed));
                        }
                    }
                    Event::ChannelOpen(id, label) => match label.as_str() {
                        "input" => chans.input = Some(id),
                        "control" => chans.control = Some(id),
                        _ => {}
                    },
                    Event::ChannelData(d) if Some(d.id) == chans.input => {
                        let Ok(text) = std::str::from_utf8(&d.data) else {
                            continue;
                        };
                        match proto::parse::<Input>(text) {
                            Ok(i) => {
                                inj.apply(&i);
                                if let Some(s) = is_timed(&i) {
                                    let t = Instant::now();
                                    pending.push((s, t));
                                    // 다음 박자(최대 33ms)를 기다리지 않고 화면이 그려질 만큼만 기다려 찍는다.
                                    next_frame = next_frame.min(t + settle);
                                }
                            }
                            Err(kind) => warn!(event = "input.bad", kind = ?kind),
                        }
                    }
                    Event::ChannelClose(id) if Some(id) == chans.input => inj.release_all(),
                    Event::KeyframeRequest(_) => video.keyframe = true,
                    _ => {}
                },
            }
        };
        if !rtc.is_alive() {
            return Ok(Some(EndReason::Failed));
        }
        let now = Instant::now();
        if down_since.is_some_and(|t| now - t >= dead) {
            warn!(event = "session.dead", session_id, connected);
            return Ok(Some(EndReason::Failed));
        }

        // 3. 화면 — 바뀌었거나, 키프레임 요청이거나, VIDEO_IDLE_RESEND_MS 동안 안 보냈으면 보낸다. 끊긴 동안은 쉰다.
        let mut wrote = false;
        if connected && down_since.is_none() && now >= next_frame {
            next_frame = now + every;
            // 못 열면 1초 뒤 다시, 로그는 한 번만(앞 도우미가 복제를 쥐고 있거나 화면이 잠김).
            if cap.is_none() && now >= cap_retry {
                match Capturer::new() {
                    Ok(c) => {
                        cap = Some(c);
                        cap_warned = false;
                    }
                    Err(e) => {
                        cap_retry = now + Duration::from_secs(1);
                        if !std::mem::replace(&mut cap_warned, true) {
                            warn!(event = "capture.open_failed", why = %e);
                        }
                    }
                }
            }
            let mut fresh = false;
            match cap.as_mut().map(Capturer::next) {
                Some(Ok(Some(f))) => {
                    video.set_frame(&f.bgra, f.width, f.height);
                    last_sent = None;
                    fresh = true;
                }
                Some(Err(e)) => {
                    warn!(event = "capture.failed", why = %e);
                    cap = None;
                }
                _ => {}
            }
            // 반영된 장: 입력 뒤 INPUT_SETTLE_MS 지나 새로 찍힌 장, 또는 한 프레임 간격이 지난 장.
            // 입력이 화면을 안 바꿨어도(커서는 영상에 없다) 한 프레임 뒤엔 보낸다 — D-08 짝이 1초 재전송을 기다리지 않게.
            let applied = |t: &Instant| now - *t >= every || (fresh && now - *t >= settle);
            let input_due = pending.iter().any(|(_, t)| applied(t));
            let due = video.keyframe || input_due || last_sent.is_none_or(|t| now - t >= idle);
            if let (true, Some(mid), Some(pt)) = (due, mid, pt) {
                if let Some(bits) = video.encode()? {
                    let ts = MediaTime::from_90khz((now - t0).as_micros() as u64 * 9 / 100);
                    if let Some(w) = rtc.writer(mid) {
                        let zero = MediaTime::from_hundredths(0);
                        w.playout_delay(zero, zero).write(pt, now, ts, bits)?;
                        wrote = true;
                    }
                    last_sent = Some(now);
                    // D-08: 주입 뒤 처음 인코딩한 프레임의 RTP 타임스탬프(str0m 은 이 값을 그대로 쓴다).
                    if let Some(mut ch) = chans.control.and_then(|id| rtc.channel(id)) {
                        let (ready, wait): (Vec<_>, Vec<_>) =
                            pending.drain(..).partition(|(_, t)| applied(t));
                        pending = wait;
                        for (seq, _) in ready {
                            let m = Control::InputApplied {
                                seq,
                                rtp_ts: ts.numer() as u32,
                            };
                            let _ = ch.write(false, serde_json::to_string(&m)?.as_bytes());
                        }
                    }
                }
            }
            // 일찍 찍은 장에 반영이 없었으면(화면 안 바뀜) 다음 장은 박자가 아니라 입력 + 한 프레임 간격에.
            if let Some(t) = pending.iter().map(|p| p.1).min() {
                next_frame = next_frame.min((t + every).max(now + Duration::from_millis(1)));
            }
        }

        // 방금 쓴 화면·D-08 은 2단계(poll_output)에서야 나간다 — 기다리지 말고 바로 내보낸다(전엔 다음 박자까지 최대 33ms 묵음).
        if wrote {
            continue;
        }

        // 4. UDP 를 기다린다 — 다음 시한·다음 프레임 중 빠른 쪽까지, 신호를 놓치지 않게 길어야 50ms.
        let until = if connected {
            timeout.min(next_frame)
        } else {
            timeout
        };
        let wait = until
            .saturating_duration_since(Instant::now())
            .clamp(Duration::from_millis(1), Duration::from_millis(50));
        sock.set_read_timeout(Some(wait))?;
        match sock.recv_from(&mut buf) {
            Ok((n, source)) => {
                if let Ok(contents) = buf[..n].try_into() {
                    rtc.handle_input(str0m::Input::Receive(
                        Instant::now(),
                        Receive {
                            proto: Protocol::Udp,
                            source,
                            destination: local,
                            contents,
                        },
                    ))?;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            // Windows: 상대가 닫힌 포트면 다음 recv 가 ConnectionReset 을 낸다 — 무시하고 계속.
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
            // 아무나 보낼 수 있는 UDP 하나로 세션이 죽지 않게 — 남기고 계속.
            Err(e) => warn!(event = "session.recv_error", why = %e),
        }
        rtc.handle_input(str0m::Input::Timeout(Instant::now()))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stun_url_is_parsed() {
        let ice = vec![
            IceServer {
                urls: vec!["turn:127.0.0.1:3478?transport=udp".into()],
                username: Some("u".into()),
                credential: Some("c".into()),
            },
            IceServer {
                urls: vec!["stun:127.0.0.1:3478".into(), "stun:127.0.0.1:53".into()],
                username: None,
                credential: None,
            },
        ];
        assert_eq!(stun_server(&ice), Some("127.0.0.1:3478".parse().unwrap()));
        assert_eq!(stun_server(&[]), None);
    }

    #[test]
    fn timed_inputs_match_viewer() {
        use proto::MouseButton;
        assert_eq!(
            is_timed(&Input::MouseButton {
                seq: 1,
                button: MouseButton::Left,
                down: true
            }),
            Some(1)
        );
        assert_eq!(
            is_timed(&Input::MouseButton {
                seq: 2,
                button: MouseButton::Left,
                down: false
            }),
            None
        );
        assert_eq!(
            is_timed(&Input::Text {
                seq: 3,
                text: "가".into()
            }),
            Some(3)
        );
        assert_eq!(
            is_timed(&Input::MouseMove {
                seq: 4,
                x: 0.1,
                y: 0.1
            }),
            None
        );
    }
}

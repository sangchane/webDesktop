//! WebDesktop 에이전트(Windows, 콘솔 실행 — 서비스는 첫 작업 2).
//! 등록 코드를 한 번 입력하면 그 뒤로는 저장한 키로 허브에 붙어 있다.
mod helper;
mod platform;
mod resident;

use resident::{Home, Stop};
use std::process::ExitCode;

/// 03 상수 표 — 로그 하루 1파일, 이만큼 보관.
const AGENT_LOG_FILES: usize = 7;

fn init_log(home: &Home) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    // 폴더가 없으면 회전 도구가 옛 파일 정리에서 오류 줄을 콘솔에 찍는다.
    let _ = std::fs::create_dir_all(home.logs());
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("agent")
        .filename_suffix("log")
        .max_log_files(AGENT_LOG_FILES)
        .build(home.logs())
        .map_err(|e| eprintln!("로그 파일을 열지 못했습니다: {e}"))
        .ok()?;
    let (w, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::fmt().json().with_writer(w).init();
    Some(guard)
}

fn main() -> ExitCode {
    // WebRTC DTLS·SRTP 는 Windows CNG(str0m wincrypto) — OpenSSL·aws-lc 빌드 없음.
    str0m::crypto::from_feature_flags().install_process_default();
    let home = Home::from_env();
    // 부모(ProgramData)는 그대로, 우리 폴더는 보호 ACL 로 — 다른 로컬 사용자가 파일을 끼워 넣지 못하게.
    if let Err(e) = home
        .0
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| platform::create_private_dir(&home.0))
    {
        eprintln!("{} 폴더를 만들지 못했습니다: {e}", home.0.display());
        return ExitCode::FAILURE;
    }
    let _guard = init_log(&home);
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("실행 준비 실패: {e}");
            return ExitCode::FAILURE;
        }
    };
    loop {
        let (cfg, key) = match resident::load(&home) {
            Ok(Some(x)) => x,
            Ok(None) => {
                match resident::enroll(&home, &resident::hub_url(), &mut std::io::stdin().lock()) {
                    Ok(x) => x,
                    Err(e) => {
                        eprintln!("등록하지 못했습니다: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(e) => {
                tracing::error!(event = "config.invalid", why = %e);
                eprintln!(
                    "설정이 깨졌습니다({e}). {} 폴더를 지우고 다시 실행해 등록하세요.",
                    home.0.display()
                );
                return ExitCode::FAILURE;
            }
        };
        match rt.block_on(resident::run(&cfg, &key)) {
            Stop::Reenroll => {
                if let Err(e) = resident::forget(&home) {
                    eprintln!("다시 등록하려 했지만 {e}");
                    return ExitCode::FAILURE;
                }
                println!("허브가 이 PC 를 모릅니다(지웠거나 폐기함). 웹앱에서 새 코드를 받아 다시 등록하세요.");
            }
            Stop::Upgrade => {
                eprintln!("허브와 판이 다릅니다. 에이전트를 새 판으로 바꾸세요.");
                return ExitCode::from(3);
            }
            Stop::Replaced => {
                eprintln!("같은 PC 의 다른 에이전트가 허브에 붙어 이 에이전트를 끝냅니다.");
                return ExitCode::SUCCESS;
            }
        }
    }
}

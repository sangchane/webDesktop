//! OS 호출 계층. macOS 는 첫 작업 3.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

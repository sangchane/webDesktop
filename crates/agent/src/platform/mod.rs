//! OS 호출 계층. macOS 는 첫 작업 3.
#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod encode;
#[cfg(windows)]
pub mod inject;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

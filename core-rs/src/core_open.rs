use crate::server::QuarxCore;

/// Unified helper for external tools (quarxctl / quarxdrive / quarxnetd / etc).
///
/// Single canonical entry point:
///   - delegates to QuarxCore::open_with_default_config();
///   - external tools should NOT construct Engine directly.
pub fn open_core() -> QuarxCore {
    QuarxCore::open_with_default_config()
}

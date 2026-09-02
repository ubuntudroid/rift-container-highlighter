//! CALayer to CGS window rendering, vendored from rift.
//!
//! Source: `src/ui/common.rs` of https://github.com/acsandmann/rift at rev
//! b67cf2efc447174ca9e0cd10f558a224ed32b038 (tag v0.5.5), Apache-2.0.
//! See NOTICE at the repository root.
//!
//! Only the import paths are changed from upstream.
//!
//! Upstream's `WindowLayoutMetrics` and `compute_window_layout_metrics`
//! are stack_line-specific and are NOT vendored.

use std::ptr;

use objc2_core_foundation::{CFType, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_quartz_core::{CALayer, CATransaction};

use crate::vendor::skylight::{
    CFRelease, G_CONNECTION, SLSFlushWindowContentRegion, SLWindowContextCreate,
};

pub fn render_layer_to_cgs_window(window_id: u32, size: CGSize, layer: &CALayer) {
    unsafe {
        let ctx: *mut CGContext =
            SLWindowContextCreate(*G_CONNECTION, window_id, ptr::null_mut() as *mut CFType);
        if ctx.is_null() {
            return;
        }

        let clear = CGRect::new(CGPoint::new(0.0, 0.0), size);
        CGContext::clear_rect(Some(&*ctx), clear);
        CGContext::save_g_state(Some(&*ctx));
        CGContext::translate_ctm(Some(&*ctx), 0.0, size.height);
        CGContext::scale_ctm(Some(&*ctx), 1.0, -1.0);
        layer.renderInContext(&*ctx);
        CGContext::restore_g_state(Some(&*ctx));
        CGContext::flush(Some(&*ctx));
        SLSFlushWindowContentRegion(*G_CONNECTION, window_id, ptr::null_mut());
        CFRelease(ctx as *mut CFType);
    }
}

pub fn with_disabled_actions<F, R>(f: F) -> R
where F: FnOnce() -> R {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    let result = f();
    CATransaction::commit();
    result
}

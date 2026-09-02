//! Trimmed SkyLight FFI, vendored from rift.
//!
//! Source: `src/sys/skylight.rs` and `src/sys.rs` (for `cg_ok`) of
//! https://github.com/acsandmann/rift at rev
//! b67cf2efc447174ca9e0cd10f558a224ed32b038 (tag v0.5.5), Apache-2.0.
//! See NOTICE at the repository root.
//!
//! Only the declarations `cgs_window.rs` and `render_layer.rs` reference are
//! kept; rift's file declares 57 SkyLight functions. `SLSSetWindowLayerContext`
//! is deliberately absent — `cgs_window.rs` resolves it at runtime with
//! `dlsym`.
//!
//! Upstream credits, retained:
//!   https://github.com/asmagill/hs._asm.undocumented.spaces/blob/master/CGSSpace.h
//!   https://github.com/koekeishiya/yabai/blob/master/src/misc/extern.h

use std::ffi::{c_int, c_void};

use objc2_core_foundation::{CFString, CFType, CGRect};
use objc2_core_graphics::{CGContext, CGError};
use once_cell::sync::Lazy;

pub static G_CONNECTION: Lazy<cid_t> = Lazy::new(|| unsafe { SLSMainConnectionID() });

#[allow(non_camel_case_types)]
pub type cid_t = i32;

#[inline(always)]
pub fn cg_ok(err: CGError) -> Result<(), CGError> {
    if err == CGError::Success { Ok(()) } else { Err(err) }
}

#[allow(non_snake_case)]
unsafe extern "C" {
    pub fn SLSMainConnectionID() -> cid_t;
    pub fn CGRegionCreateEmptyRegion() -> *mut CFType;
    pub fn CGSNewRegionWithRect(rect: *const CGRect, region: *mut *mut CFType) -> CGError;
    pub fn SLSClearWindowTags(cid: cid_t, wid: u32, tags: *mut u64, tag_count: c_int) -> CGError;
    pub fn SLSNewWindowWithOpaqueShapeAndContext(
        cid: cid_t,
        r#type: c_int,
        region: *mut CFType,
        opaque_region: *mut CFType,
        options: c_int,
        tags: *mut u64,
        x: f32,
        y: f32,
        tag_count: c_int,
        out_wid: *mut u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSOrderWindow(cid: cid_t, wid: u32, order: c_int, relative_to: u32) -> CGError;
    pub fn SLSReleaseWindow(cid: cid_t, wid: u32) -> CGError;
    pub fn SLSSetWindowAlpha(cid: cid_t, wid: u32, alpha: f32) -> CGError;
    pub fn SLSSetWindowBackgroundBlurRadius(cid: cid_t, wid: u32, radius: c_int) -> CGError;
    pub fn SLSSetWindowBackgroundBlurRadiusStyle(
        cid: cid_t,
        wid: u32,
        radius: c_int,
        style: c_int,
    ) -> CGError;
    pub fn SLSSetWindowLevel(cid: cid_t, wid: u32, level: c_int) -> CGError;
    pub fn SLSSetWindowOpacity(cid: cid_t, wid: u32, opaque: bool) -> CGError;
    pub fn SLSSetWindowProperty(
        cid: cid_t,
        wid: u32,
        property: *mut CFString,
        value: *mut CFType,
    ) -> CGError;
    pub fn SLSSetWindowResolution(cid: cid_t, wid: u32, resolution: f64) -> CGError;
    pub fn SLSSetWindowShape(
        cid: cid_t,
        wid: u32,
        x_offset: f32,
        y_offset: f32,
        shape: *mut CFType,
    ) -> CGError;
    pub fn SLSSetWindowSubLevel(cid: cid_t, wid: u32, sub_level: c_int) -> CGError;
    pub fn SLSSetWindowTags(cid: cid_t, wid: u32, tags: *mut u64, tag_count: c_int) -> CGError;
    pub fn CFRelease(cf: *mut CFType);
    pub fn SLSFlushWindowContentRegion(cid: cid_t, wid: u32, dirty: *mut c_void) -> CGError;
    pub fn SLWindowContextCreate(cid: cid_t, wid: u32, options: *mut CFType) -> *mut CGContext;
}

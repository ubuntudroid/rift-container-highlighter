use anyhow::Result;
use objc2::rc::Retained;
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CGPoint, CGRect, CGSize, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::CGColor;
use objc2_quartz_core::CALayer;
use rift_protocol::Rect;

use crate::config::Config;
use crate::geometry::ContainerRect;
use crate::vendor::cgs_window::CgsWindow;
use crate::vendor::render_layer::{render_layer_to_cgs_window, with_disabled_actions};

/// Draw every container outline into one full-screen overlay, hold it for the
/// configured duration, then return. The overlay is released when the
/// `CgsWindow` drops — and, more importantly, when the process exits, since the
/// window server discards windows owned by a closed connection. That is the
/// whole cleanup story: no state file, no restore path.
pub fn flash(rects: &[ContainerRect], cfg: &Config, screen: Rect) -> Result<()> {
    if rects.is_empty() {
        return Ok(());
    }

    let frame = to_cg(screen);

    let root = CALayer::layer();
    with_disabled_actions(|| {
        root.setFrame(CGRect::new(CGPoint::new(0.0, 0.0), frame.size));
        // Outermost first, so deeper rects are added later and composite on top.
        for r in rects {
            for layer in band_layers(r, cfg, screen) {
                root.addSublayer(&layer);
            }
        }
    });

    // Setup verified against rift's stack_line.rs: level 0 plus order_above is
    // the combination the window server actually displays.
    let win = CgsWindow::new(frame)?;
    win.set_opacity(false)?;
    win.set_alpha(1.0)?;
    win.set_level(0)?;
    win.set_tags(1 << 3)?; // no system drop shadow
    win.order_above(None)?;

    render_layer_to_cgs_window(win.id(), frame.size, &root);

    // The run loop must turn or nothing is composited; a blocking sleep here
    // shows an empty screen with no error anywhere.
    CFRunLoop::run_in_mode(
        unsafe { kCFRunLoopDefaultMode },
        cfg.flash_ms as f64 / 1000.0,
        false,
    );
    Ok(())
}

/// A band that starts opaque at the container's outer edge and fades to fully
/// transparent `band_width` points inward, overlapping the member windows.
///
/// Deeper containers get narrower bands (`band_decay`): a child usually shares
/// two or three edges with its parent, so their bands land on the same pixels,
/// and differing widths turn that overlap into a broad wash with a narrower
/// core rather than one muddy band.
///
/// Built as a stack of 1pt concentric rounded-rect rings with a linear alpha
/// ramp, because a CALayer border is already a rounded outline and needs no
/// CAShapeLayer, CGPath or mask. The ceiling: one sublayer per point of band
/// width (36 by default) per container, and banding would show if the step
/// grew above 1pt. If either becomes a problem, draw a CGImage with a real
/// per-pixel ramp and set it as the layer's contents.
fn band_layers(r: &ContainerRect, cfg: &Config, screen: Rect) -> Vec<Retained<CALayer>> {
    // Each nesting level steps further in so concentric bands stay
    // distinguishable rather than sitting on top of one another.
    let level = cfg.level_inset * (r.depth.saturating_sub(1)) as f64;
    let grow = cfg.outset - level;

    let outer = CGRect::new(
        CGPoint::new(
            r.rect.origin.x - screen.origin.x - grow,
            r.rect.origin.y - screen.origin.y - grow,
        ),
        CGSize::new(
            (r.rect.size.width + 2.0 * grow).max(0.0),
            (r.rect.size.height + 2.0 * grow).max(0.0),
        ),
    );

    let dim = if r.selected { 1.0 } else { cfg.dim_factor };
    let argb = cfg.color_for_depth(r.depth);
    let steps = cfg.band_width_for_depth(r.depth).max(1.0).round() as usize;

    let mut layers = Vec::with_capacity(steps);
    for i in 0..steps {
        let inset = i as f64;
        let w = outer.size.width - 2.0 * inset;
        let h = outer.size.height - 2.0 * inset;
        if w <= 0.0 || h <= 0.0 {
            break;
        }
        // Opaque at the outer edge, zero at band_width inward.
        let ramp = 1.0 - (inset / steps as f64);
        let layer = CALayer::layer();
        layer.setFrame(CGRect::new(
            CGPoint::new(outer.origin.x + inset, outer.origin.y + inset),
            CGSize::new(w, h),
        ));
        layer.setBorderWidth(1.0);
        layer.setBorderColor(Some(&cg_color(argb, dim * ramp)));
        layer.setCornerRadius((cfg.corner_radius - inset).max(0.0));
        layers.push(layer);
    }
    layers
}

fn to_cg(r: Rect) -> CGRect {
    // No Y flip: CGS window frames share rift's top-left origin.
    CGRect::new(
        CGPoint::new(r.origin.x, r.origin.y),
        CGSize::new(r.size.width, r.size.height),
    )
}

/// `0xAARRGGBB` to a CGColor, with the alpha scaled by `dim`.
fn cg_color(argb: u32, dim: f64) -> CFRetained<CGColor> {
    let a = ((argb >> 24) & 0xff) as f64 / 255.0;
    let r = ((argb >> 16) & 0xff) as f64 / 255.0;
    let g = ((argb >> 8) & 0xff) as f64 / 255.0;
    let b = (argb & 0xff) as f64 / 255.0;
    CGColor::new_srgb(r, g, b, a * dim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_unpacks_and_dims() {
        // Nothing here touches the window server, so it is safe in a test.
        let c = cg_color(0xff80_4020, 0.5);
        assert_eq!(CGColor::number_of_components(Some(&c)), 4);
        // SAFETY: components() returns a pointer to number_of_components floats.
        let comps = unsafe { std::slice::from_raw_parts(CGColor::components(Some(&c)), 4) };
        assert!((comps[0] - 128.0 / 255.0).abs() < 1e-6, "red");
        assert!((comps[1] - 64.0 / 255.0).abs() < 1e-6, "green");
        assert!((comps[2] - 32.0 / 255.0).abs() < 1e-6, "blue");
        assert!((CGColor::alpha(Some(&c)) - 0.5).abs() < 1e-6, "alpha halved by dim");
    }
}

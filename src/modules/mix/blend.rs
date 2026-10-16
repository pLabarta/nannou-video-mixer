use crate::{FrameData, ModuleChrome};
use nannou::image;

/// The BLEND module's own config: just the 0..1 mix between its two inputs.
pub struct BlendState {
    pub chrome: ModuleChrome,
    pub mix: f32,
}

impl BlendState {
    pub fn new() -> Self {
        BlendState {
            chrome: ModuleChrome::new(),
            mix: 0.5,
        }
    }
}

/// 0 = all of `a`, 1 = all of `b`, in between a per-pixel mix. If only one
/// side has a signal, that side passes through untouched. Resizes `b` to
/// `a`'s resolution when they differ so the two cameras don't need to
/// match formats.
pub fn blend_frames(a: Option<FrameData>, b: Option<FrameData>, mix: f32) -> Option<FrameData> {
    let mix = mix.clamp(0.0, 1.0);
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (Some((aw, ah, a_rgba, a_ver)), Some((bw, bh, b_rgba, b_ver))) => {
            let b_rgba = if (bw, bh) == (aw, ah) {
                b_rgba
            } else {
                let b_image = image::RgbaImage::from_raw(bw, bh, b_rgba)?;
                image::imageops::resize(&b_image, aw, ah, image::imageops::FilterType::Nearest)
                    .into_raw()
            };

            let mut out = vec![0u8; a_rgba.len()];
            for i in 0..out.len() {
                let av = a_rgba[i] as f32;
                let bv = *b_rgba.get(i).unwrap_or(&0) as f32;
                out[i] = (av + (bv - av) * mix).round() as u8;
            }

            let version = a_ver
                .wrapping_mul(1_000_003)
                .wrapping_add(b_ver.wrapping_mul(97))
                .wrapping_add((mix * 1000.0) as u64);
            Some((aw, ah, out, version))
        }
    }
}

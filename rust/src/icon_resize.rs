//! High-quality resampling for icons obtained from foreign HWNDs.
//!
//! DrawIconEx scales icons through GDI without a selectable high-quality
//! interpolation filter. Render HICONs at their native size instead, then
//! resize *premultiplied* RGBA here so transparent edges cannot gain halos.
//! Icon sizes are small, and the results are cached per pinned window.

const LANCZOS_RADIUS: f64 = 3.0;

/// Prefer the smallest native image that does not need enlarging. If every
/// candidate is too small, prefer the largest one. Unknown sizes come last.
pub(crate) fn source_rank(dimensions: Option<(u32, u32)>, target: u32) -> (u8, u32) {
    match dimensions {
        Some((width, height)) if width != 0 && height != 0 => {
            let side = width.min(height);
            if side >= target {
                (0, side - target)
            } else {
                (1, target - side)
            }
        }
        _ => (2, u32::MAX),
    }
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let x = std::f64::consts::PI * x;
        x.sin() / x
    }
}

fn lanczos(x: f64) -> f64 {
    if x.abs() >= LANCZOS_RADIUS {
        0.0
    } else {
        sinc(x) * sinc(x / LANCZOS_RADIUS)
    }
}

/// Pixel-centre aligned contributions for one dimension. Lanczos3 filters
/// downscaling (including the source footprint); bilinear avoids ringing when
/// no larger icon exists and enlargement is unavoidable.
fn contributions(source: usize, target: usize) -> Vec<Vec<(usize, f64)>> {
    let scale = source as f64 / target as f64;
    let reducing = scale > 1.0;
    let filter_scale = if reducing { scale } else { 1.0 };
    let radius = if reducing { LANCZOS_RADIUS } else { 1.0 };

    (0..target)
        .map(|index| {
            let centre = (index as f64 + 0.5) * scale - 0.5;
            let start = ((centre - radius * filter_scale).ceil() as isize).max(0) as usize;
            let end = ((centre + radius * filter_scale).floor() as isize)
                .min(source as isize - 1) as usize;
            let mut taps = Vec::new();
            for pixel in start..=end {
                let delta = (pixel as f64 - centre) / filter_scale;
                let weight = if reducing {
                    lanczos(delta)
                } else {
                    (1.0 - delta.abs()).max(0.0)
                };
                if weight != 0.0 {
                    taps.push((pixel, weight));
                }
            }
            let total: f64 = taps.iter().map(|(_, weight)| *weight).sum();
            if total.abs() < 1e-12 {
                return vec![(centre.round().clamp(0.0, (source - 1) as f64) as usize, 1.0)];
            }
            for (_, weight) in &mut taps {
                *weight /= total;
            }
            taps
        })
        .collect()
}

/// Resize premultiplied RGBA using a separable, alpha-aware filter.
/// The returned pixels remain premultiplied for Slint's
/// Image::from_rgba8_premultiplied.
pub(crate) fn resize_premultiplied_rgba(
    source: &[u8],
    width: u32,
    height: u32,
    target: u32,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || target == 0
        || width > 512 || height > 512 || target > 512
    {
        return None;
    }
    let (sw, sh, dst) = (width as usize, height as usize, target as usize);
    if source.len() != sw.checked_mul(sh)?.checked_mul(4)? {
        return None;
    }
    if width == target && height == target {
        return Some(source.to_vec());
    }

    let x_taps = contributions(sw, dst);
    let y_taps = contributions(sh, dst);
    let mut horizontal = vec![[0.0_f64; 4]; sh.checked_mul(dst)?];

    for row in 0..sh {
        for (x, taps) in x_taps.iter().enumerate() {
            let pixel = &mut horizontal[row * dst + x];
            for &(src_x, weight) in taps {
                let offset = (row * sw + src_x) * 4;
                for channel in 0..4 {
                    pixel[channel] += f64::from(source[offset + channel]) * weight;
                }
            }
        }
    }

    let mut result = vec![0; dst.checked_mul(dst)?.checked_mul(4)?];
    for (y, taps) in y_taps.iter().enumerate() {
        for x in 0..dst {
            let mut pixel = [0.0_f64; 4];
            for &(src_y, weight) in taps {
                let sample = &horizontal[src_y * dst + x];
                for channel in 0..4 {
                    pixel[channel] += sample[channel] * weight;
                }
            }
            let offset = (y * dst + x) * 4;
            let alpha = pixel[3].round().clamp(0.0, 255.0);
            for channel in 0..3 {
                result[offset + channel] = pixel[channel].round().clamp(0.0, alpha) as u8;
            }
            result[offset + 3] = alpha as u8;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_selection_prefers_native_resolution_over_16px_icons() {
        let target = 48;
        assert!(source_rank(Some((48, 48)), target) < source_rank(Some((16, 16)), target));
        assert!(source_rank(Some((48, 48)), target) < source_rank(Some((256, 256)), target));
        assert!(source_rank(Some((256, 256)), target) < source_rank(None, target));
        assert!(source_rank(Some((32, 32)), target) < source_rank(Some((16, 16)), target));
    }

    #[test]
    fn identity_does_not_change_icon_pixels() {
        let pixels = [255, 64, 16, 255, 0, 0, 0, 0, 100, 50, 0, 128, 10, 0, 0, 10];
        assert_eq!(resize_premultiplied_rgba(&pixels, 2, 2, 2), Some(pixels.to_vec()));
    }

    #[test]
    fn checkerboard_downscale_removes_aliasing() {
        let mut source = vec![0; 16 * 16 * 4];
        for y in 0..16 {
            for x in 0..16 {
                let offset = (y * 16 + x) * 4;
                let value = if (x + y) % 2 == 0 { 255 } else { 0 };
                source[offset..offset + 4].copy_from_slice(&[value, value, value, 255]);
            }
        }
        let resized = resize_premultiplied_rgba(&source, 16, 16, 4).unwrap();
        for pixel in resized.chunks_exact(4) {
            assert!((i16::from(pixel[0]) - 128).abs() < 25, "{pixel:?}");
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn translucent_edges_remain_premultiplied() {
        let mut source = vec![0; 8 * 8 * 4];
        for y in 0..8 {
            for x in 4..8 {
                let offset = (y * 8 + x) * 4;
                source[offset..offset + 4].copy_from_slice(&[255, 48, 0, 255]);
            }
        }
        let resized = resize_premultiplied_rgba(&source, 8, 8, 5).unwrap();
        assert!(resized.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
        for pixel in resized.chunks_exact(4) {
            assert!(pixel[..3].iter().all(|channel| *channel <= pixel[3]));
            assert_eq!(pixel[2], 0);
        }
    }

    #[test]
    fn enlargement_interpolates_instead_of_repeating_square_pixels() {
        let source = [0, 0, 0, 255, 255, 255, 255, 255,
                      0, 0, 0, 255, 255, 255, 255, 255];
        let resized = resize_premultiplied_rgba(&source, 2, 2, 8).unwrap();
        assert!(resized.chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255));
        assert!(resized.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    fn rejects_invalid_dimensions_and_incomplete_buffers() {
        assert_eq!(resize_premultiplied_rgba(&[], 0, 1, 16), None);
        assert_eq!(resize_premultiplied_rgba(&[0; 3], 1, 1, 16), None);
        assert_eq!(resize_premultiplied_rgba(&[], 1024, 1024, 16), None);
    }
}

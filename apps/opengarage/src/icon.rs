//! The program's icon: the body of a blue electric guitar on a dark tile.
//!
//! The picture comes from `apps/opengarage/icon`, where `tools/icon.ps1`
//! writes it at each size from one drawing (see "External files" in the
//! documentation for where that drawing comes from). It bears no maker's
//! name or mark.
//!
//! This file is used twice: by the program, for the icon of its window, and
//! by its build script, for the icon of the executable.

/// The sizes the icon was made at, each as red, green, blue and opacity for
/// each pixel, row by row from the top.
const MADE: [(u32, &[u8]); 6] = [
    (16, include_bytes!("../icon/16.rgba")),
    (24, include_bytes!("../icon/24.rgba")),
    (32, include_bytes!("../icon/32.rgba")),
    (48, include_bytes!("../icon/48.rgba")),
    (64, include_bytes!("../icon/64.rgba")),
    (256, include_bytes!("../icon/256.rgba")),
];

/// The icon at `size` by `size` pixels: red, green, blue and opacity for each
/// pixel, row by row from the top. A size it was not made at is taken from
/// the largest one.
pub fn rgba(size: u32) -> Vec<u8> {
    if let Some((_, pixels)) = MADE.iter().find(|(made, pixels)| *made == size && pixels.len() == (size * size * 4) as usize) {
        return pixels.to_vec();
    }
    let (from, source) = MADE[MADE.len() - 1];
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // The pixel of the large picture under the middle of this one.
            let sx = ((x * 2 + 1) * from / (size.max(1) * 2)).min(from - 1);
            let sy = ((y * 2 + 1) * from / (size.max(1) * 2)).min(from - 1);
            let at = ((sy * from + sx) * 4) as usize;
            out.extend_from_slice(source.get(at..at + 4).unwrap_or(&[0, 0, 0, 0]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::rgba;

    #[test]
    fn the_icon_is_a_picture_with_see_through_corners() {
        // The sizes it was made at, and two it was not.
        for size in [16u32, 24, 32, 48, 64, 256, 20, 128] {
            let pixels = rgba(size);
            assert_eq!(pixels.len(), (size * size * 4) as usize);
            let at = |x: u32, y: u32| -> [u8; 4] {
                let i = ((y * size + x) * 4) as usize;
                [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
            };
            assert_eq!(at(0, 0)[3], 0, "the corners are see-through at {size}");
            assert_eq!(at(size - 1, size - 1)[3], 0);
            assert_eq!(at(size / 2, size / 2)[3], 255);
            // The body of the guitar, lower left, is blue: far more blue than red.
            let body = at(size * 32 / 100, size * 72 / 100);
            assert!(body[2] > 110 && body[2] > body[0].saturating_add(70), "{body:?} at {size}");
            // The tile behind it, at the left edge, is a dark grey.
            let tile = at(size * 10 / 100, size * 30 / 100);
            assert!(tile[3] == 255 &&tile[2].abs_diff(tile[0]) < 30 && tile[0] < 110, "{tile:?} at {size}");
        }
    }
}

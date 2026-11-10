use image::{Rgba, RgbaImage};

pub fn apply_pixelate(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    block_size: u32,
) {
    let block = block_size.max(2);
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());

    let mut by = y;
    while by < max_y {
        let block_h = block.min(max_y - by);
        let mut bx = x;
        while bx < max_x {
            let block_w = block.min(max_x - bx);

            // Compute average color in block
            let mut r_acc: u64 = 0;
            let mut g_acc: u64 = 0;
            let mut b_acc: u64 = 0;
            let mut a_acc: u64 = 0;
            let count = (block_w * block_h) as u64;

            for dy in 0..block_h {
                for dx in 0..block_w {
                    let p = image.get_pixel(bx + dx, by + dy);
                    r_acc += p[0] as u64;
                    g_acc += p[1] as u64;
                    b_acc += p[2] as u64;
                    a_acc += p[3] as u64;
                }
            }

            let avg_p = Rgba([
                (r_acc / count) as u8,
                (g_acc / count) as u8,
                (b_acc / count) as u8,
                (a_acc / count) as u8,
            ]);

            for dy in 0..block_h {
                for dx in 0..block_w {
                    image.put_pixel(bx + dx, by + dy, avg_p);
                }
            }

            bx += block;
        }
        by += block;
    }
}

pub fn apply_blur(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    radius: u32,
) {
    let r = radius.clamp(1, 40) as i32;
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());

    if max_x <= x || max_y <= y {
        return;
    }

    let sub_w = (max_x - x) as usize;
    let sub_h = (max_y - y) as usize;

    // Fast 2-pass separable box blur (simulating Gaussian blur with 3 iterations)
    let mut temp = vec![[0u8; 4]; sub_w * sub_h];
    let mut out = vec![[0u8; 4]; sub_w * sub_h];

    for sy in 0..sub_h {
        for sx in 0..sub_w {
            let p = image.get_pixel(x + sx as u32, y + sy as u32);
            temp[sy * sub_w + sx] = p.0;
        }
    }

    for _ in 0..3 {
        // Horizontal pass
        for sy in 0..sub_h {
            let row_offset = sy * sub_w;
            for sx in 0..sub_w {
                let mut r_acc = 0u32;
                let mut g_acc = 0u32;
                let mut b_acc = 0u32;
                let mut a_acc = 0u32;
                let mut count = 0u32;

                let start = (sx as i32 - r).max(0) as usize;
                let end = ((sx as i32 + r).min(sub_w as i32 - 1)) as usize;

                for kx in start..=end {
                    let p = temp[row_offset + kx];
                    r_acc += p[0] as u32;
                    g_acc += p[1] as u32;
                    b_acc += p[2] as u32;
                    a_acc += p[3] as u32;
                    count += 1;
                }

                out[row_offset + sx] = [
                    (r_acc / count) as u8,
                    (g_acc / count) as u8,
                    (b_acc / count) as u8,
                    (a_acc / count) as u8,
                ];
            }
        }

        // Vertical pass
        for sx in 0..sub_w {
            for sy in 0..sub_h {
                let mut r_acc = 0u32;
                let mut g_acc = 0u32;
                let mut b_acc = 0u32;
                let mut a_acc = 0u32;
                let mut count = 0u32;

                let start = (sy as i32 - r).max(0) as usize;
                let end = ((sy as i32 + r).min(sub_h as i32 - 1)) as usize;

                for ky in start..=end {
                    let p = out[ky * sub_w + sx];
                    r_acc += p[0] as u32;
                    g_acc += p[1] as u32;
                    b_acc += p[2] as u32;
                    a_acc += p[3] as u32;
                    count += 1;
                }

                temp[sy * sub_w + sx] = [
                    (r_acc / count) as u8,
                    (g_acc / count) as u8,
                    (b_acc / count) as u8,
                    (a_acc / count) as u8,
                ];
            }
        }
    }

    // Write blurred pixels back into the original image
    for sy in 0..sub_h {
        for sx in 0..sub_w {
            let p = temp[sy * sub_w + sx];
            image.put_pixel(x + sx as u32, y + sy as u32, Rgba(p));
        }
    }
}

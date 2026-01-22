/// Generate a random color from a seed using HSV color space
pub fn random_color_from_seed(seed: u16) -> [u8; 3] {
    fn pseudo_random_permutation(seed: u16) -> f32 {
        let mut num = (seed & 0xFF) as u8;

        for _ in 0..2 {
            num = num.wrapping_mul(197).rotate_left(5) ^ 0x5A;
        }

        num as f32 / (u8::MAX as f32)
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
        let h_i = (h * 6.0).floor() as u32 % 6;
        let f = h * 6.0 - h_i as f32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);

        let (r, g, b) = match h_i {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };

        [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
    }

    let hue = pseudo_random_permutation(seed);
    hsv_to_rgb(hue, 0.8, 0.9)
}

use anyhow::Context;
use image::Pixel;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone)]
pub struct Palette {
    background: image::Rgb<u8>,
    colors: Vec<image::Rgb<u8>>,
    num_colors: usize,
}

impl FromStr for Palette {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(':').collect::<Vec<&str>>();
        if parts.len() < 3 {
            anyhow::bail!("not enough colors")
        }

        let mut colors = Vec::new();

        for p in parts {
            colors.push(hex_string_to_color(p).context(format!("failed to parse color {}", p))?);
        }

        Palette::new(colors[0], colors[1..].to_owned())
    }
}

fn hex_string_to_color(s: &str) -> anyhow::Result<image::Rgb<u8>> {
    if s.len() != 6 || s.starts_with("+") {
        anyhow::bail!("invalid color")
    }

    let r = u8::from_str_radix(&s[0..2], 16)?;
    let g = u8::from_str_radix(&s[2..4], 16)?;
    let b = u8::from_str_radix(&s[4..6], 16)?;

    Ok(image::Rgb([r, g, b]))
}

impl Palette {
    fn new(bg: image::Rgb<u8>, cols: Vec<image::Rgb<u8>>) -> anyhow::Result<Palette> {
        let l = cols.len();
        if l < 2 {
            anyhow::bail!("not enough colors");
        }
        Ok(Palette {
            background: bg,
            colors: cols,
            num_colors: (l - 1) * 256,
        })
    }

    pub fn get_test_image(&self) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
        let height = 30;
        let mut image_buffer = image::ImageBuffer::new(self.size() as u32, 2 * height);

        for i in 0..(self.size()) {
            for j in 0..height {
                image_buffer.put_pixel(i as u32, j, self.get_color(i))
            }
            for j in height..(2 * height) {
                image_buffer.put_pixel(i as u32, j, self.bg());
            }
        }

        image_buffer
    }

    pub fn size(&self) -> usize {
        self.num_colors
    }

    pub fn bg(&self) -> image::Rgb<u8> {
        self.background
    }

    pub fn get_color(&self, i: usize) -> image::Rgb<u8> {
        if i >= self.num_colors {
            panic!("index out of range, expected 0..{}", self.num_colors);
        }

        let k = i / 256;
        let alpha = (i % 256) as u8;

        let mut col1 = self.colors[k].to_rgba();
        let mut col2 = self.colors[k + 1].to_rgba();
        col2.channels_mut()[3] = alpha;
        col1.blend(&col2);
        col1.to_rgb()
    }

    pub fn get_color_mapping(
        &self,
        mut values: Vec<i64>,
    ) -> Box<dyn Fn(i64) -> image::Rgb<u8> + '_> {
        values.sort();
        values.dedup();

        if values.is_empty() {
            // a bit overcooked to fit the signature, but this case can technically occur.
            // e.g. all zeros in inhabited time, filter out zeros to use background color for
            // the corresponding pixels, remaining values are none. get color mapping for no values,
            // which would then logically never be accessed while drawing the heatmap.
            // (would obviously produce a boring heatmap, but you do you)
            Box::new(move |_: i64| -> image::Rgb<u8> {
                panic!("value out of range (color mapping with empty domain)");
            })
        } else {
            if values.len() >= self.num_colors {
                let val_to_idx = values_to_fewer_colors_mapping(values, self.num_colors);
                Box::new(move |val| self.get_color(val_to_idx(val)))
            } else {
                let val_to_idx = values_to_more_colors_mapping(values, self.num_colors);
                Box::new(move |val| self.get_color(val_to_idx(val)))
            }
        }
    }
}
fn values_to_fewer_colors_mapping(values: Vec<i64>, num_colors: usize) -> impl Fn(i64) -> usize {
    assert!(!values.is_empty());
    assert!(values.len() >= num_colors);
    debug_assert!(values.is_sorted());
    debug_assert!(values.windows(2).all(|w| w[0] != w[1])); // no duplicates

    let min_val = values[0];
    let max_val = values[values.len() - 1];

    let vals_per_col = (values.len() as f64 / num_colors as f64).floor() as usize;

    let leftover = values.len() - (vals_per_col * num_colors);
    let point = num_colors - leftover;

    let bounds = (1..num_colors)
        .map(|i| {
            let regular_buckets = usize::min(point, i);
            let extended_buckets = i.saturating_sub(point);
            values[regular_buckets * vals_per_col + extended_buckets * (vals_per_col + 1) - 1]
        })
        .collect::<Vec<i64>>();

    debug_assert!(bounds.windows(2).all(|w| w[1] > w[0]));

    move |val: i64| {
        if val < min_val || val > max_val {
            panic!("value {val} out of range, expected {min_val}..={max_val}");
        }
        bounds.partition_point(|&x| x < val)
    }
}

fn values_to_more_colors_mapping(values: Vec<i64>, num_colors: usize) -> impl Fn(i64) -> usize {
    assert!(!values.is_empty());
    assert!(values.len() <= num_colors);
    debug_assert!(values.is_sorted());
    debug_assert!(values.windows(2).all(|w| w[0] != w[1])); // no duplicates

    let min_val = values[0];
    let max_val = values[values.len() - 1];

    let mut hash_map = HashMap::new();

    let f = (num_colors - 1) as f64 / (values.len() - 1) as f64;
    for (i, &val) in values.iter().enumerate() {
        let j = (i as f64 * f).floor() as usize;
        hash_map.insert(val, j);
    }

    move |val: i64| {
        if val < min_val || val > max_val {
            panic!("value {val} out of range, expected {min_val}..={max_val}");
        }
        *hash_map.get(&val).expect("programmer error")
    }
}
pub fn default_palette() -> Palette {
    Palette::new(
        image::Rgb([20, 0, 30]),
        vec![
            image::Rgb([30, 9, 151]),
            image::Rgb([172, 0, 217]),
            image::Rgb([217, 0, 0]),
            image::Rgb([217, 166, 0]),
            image::Rgb([255, 255, 255]),
        ],
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCase {
        values: Vec<i64>,
        num_colors: usize,
    }

    fn test_cases_more_colors() -> Vec<TestCase> {
        vec![
            TestCase {
                values: vec![0, 3, 7, 17],
                num_colors: 100,
            },
            TestCase {
                values: vec![0, 10, 20, 37],
                num_colors: 50,
            },
            TestCase {
                values: vec![1, 2, 3, 5, 8, 13, 21, 34],
                num_colors: 44,
            },
        ]
    }

    fn test_cases_fewer_colors() -> Vec<TestCase> {
        vec![
            TestCase {
                values: vec![0, 1, 2, 3, 4, 5],
                num_colors: 3,
            },
            TestCase {
                values: vec![1, 2, 3, 5, 8, 13, 21, 34],
                num_colors: 7,
            },
        ]
    }

    fn test_cases_equal() -> Vec<TestCase> {
        vec![
            TestCase {
                values: vec![77, 99, 139],
                num_colors: 3,
            },
            TestCase {
                values: vec![0, 1, 2, 3, 4, 5],
                num_colors: 6,
            },
            TestCase {
                values: vec![7],
                num_colors: 1,
            },
        ]
    }

    fn test_cases_all() -> Vec<TestCase> {
        let mut t1 = test_cases_more_colors();
        let mut t2 = test_cases_fewer_colors();
        let mut t3 = test_cases_equal();
        t1.append(&mut t2);
        t1.append(&mut t3);
        t1
    }

    fn get_appropriate_mapping(test_case: TestCase) -> Box<dyn Fn(i64) -> usize> {
        if test_case.num_colors > test_case.values.len() {
            Box::new(values_to_more_colors_mapping(
                test_case.values,
                test_case.num_colors,
            ))
        } else {
            Box::new(values_to_fewer_colors_mapping(
                test_case.values,
                test_case.num_colors,
            ))
        }
    }

    #[test]
    fn min_value_maps_to_min_color() {
        for t in test_cases_all() {
            let min_val = t.values[0];
            let val_to_idx = get_appropriate_mapping(t);
            assert_eq!(val_to_idx(min_val), 0);
        }
    }

    #[test]
    fn expected_range() {
        for t in test_cases_all() {
            let num_colors = t.num_colors;
            let vals = t.values.clone();
            let val_to_idx = get_appropriate_mapping(t);

            for v in vals {
                let idx = val_to_idx(v);
                assert!(idx < num_colors);
            }
        }
    }

    #[test]
    fn max_value_maps_to_max_color() {
        for t in test_cases_all() {
            let max_val = t.values[t.values.len() - 1];
            let max_col = t.num_colors - 1;
            let val_to_idx = get_appropriate_mapping(t);
            assert_eq!(val_to_idx(max_val), max_col)
        }
    }

    #[test]
    fn more_colors_strictly_monotonic() {
        for t in test_cases_more_colors() {
            let val_to_idx = values_to_more_colors_mapping(t.values.clone(), t.num_colors);
            let indices = t
                .values
                .iter()
                .map(|&i| val_to_idx(i))
                .collect::<Vec<usize>>();
            assert!(indices.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn fewer_colors_monotonic() {
        for t in test_cases_fewer_colors() {
            let val_to_idx = values_to_fewer_colors_mapping(t.values.clone(), t.num_colors);
            let indices = t
                .values
                .iter()
                .map(|&i| val_to_idx(i))
                .collect::<Vec<usize>>();
            assert!(indices.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn vals_reasonably_distributed_across_colors() {
        for t in test_cases_all() {
            let mut colors_used = vec![0u64; t.num_colors];

            let vals = t.values.clone();

            let val_to_idx = get_appropriate_mapping(t);

            for val in vals {
                let idx = val_to_idx(val);
                colors_used[idx] += 1;
            }

            let mi = colors_used.iter().min().unwrap();
            let ma = colors_used.iter().max().unwrap();

            assert!(u64::abs_diff(*mi, *ma) <= 1);
        }
    }

    #[test]
    fn agree_if_num_values_is_num_colors() {
        for t in test_cases_equal() {
            let mapping_1 = values_to_fewer_colors_mapping(t.values.clone(), t.num_colors);
            let mapping_2 = values_to_more_colors_mapping(t.values.clone(), t.num_colors);

            for v in t.values {
                let idx_1 = mapping_1(v);
                let idx_2 = mapping_2(v);
                assert_eq!(idx_1, idx_2);
            }
        }
    }

    #[test]
    fn palette_from_str_default() {
        let p_1 = "14001E:1E0997:AC00D9:D90000:D9A600:FFFFFF";
        let pal = Palette::from_str(p_1).expect("should be default palette");

        let default_palette = default_palette();
        assert_eq!(pal.background, default_palette.background);
        assert_eq!(pal.colors, default_palette.colors);
    }

    #[test]
    fn palette_from_str() {
        let p_0 = "AAAAAA:FFFFFF:FFFFFF:EEEEEE";
        assert!(Palette::from_str(p_0).is_ok());

        let p1 = "000000:000000:000000";
        assert!(Palette::from_str(p1).is_ok());

        let p_2 = "";
        assert!(Palette::from_str(p_2).is_err());

        let p_3 = "000000:000000:00000X";
        assert!(Palette::from_str(p_3).is_err());

        let p_4 = "+AAAAAA:FFFFFF:444444";
        assert!(Palette::from_str(p_4).is_err());

        let p_5 = "AAAAAA:FFFFFF:444";
        assert!(Palette::from_str(p_5).is_err());

        let p_6 = "AAAAAA:FFFFFF";
        assert!(Palette::from_str(p_6).is_err());

        let p_7 = "AAAAAA::BBBBBB";
        assert!(Palette::from_str(p_7).is_err());
    }
}

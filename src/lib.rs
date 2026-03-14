pub mod loader;
pub mod palette;

#[cfg(target_arch = "wasm32")]
mod wasm_api;

use crate::palette::Palette;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Region {
    pub x: i32,
    pub z: i32,
    pub path: PathBuf,
}

impl Region {
    pub fn new(x: i32, z: i32, path: PathBuf) -> Self {
        Region { x, z, path }
    }
}

pub struct RegionData {
    pub x: i32,
    pub z: i32,
    pub inhabited_times: [i64; 1024],
}

impl RegionData {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            inhabited_times: [0; 1024],
        }
    }
}

pub fn make_heatmap(
    palette: &Palette,
    region_data: Vec<&RegionData>,
) -> anyhow::Result<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
    if region_data.is_empty() {
        anyhow::bail!("no region data");
    }

    let vals = region_data
        .iter()
        .flat_map(|rd| rd.inhabited_times)
        .collect::<Vec<i64>>();

    let mut vals_cl = vals.clone();
    vals_cl.retain(|&v| v != 0); // will use background color for inhabited time value 0
    let mapping = palette.get_color_mapping(vals_cl);

    let mut region_x_min = region_data[0].x;
    let mut region_x_max = region_data[0].x;
    let mut region_z_min = region_data[0].z;
    let mut region_z_max = region_data[0].z;

    region_data.iter().for_each(|rd| {
        if rd.x < region_x_min {
            region_x_min = rd.x;
        }
        if rd.x > region_x_max {
            region_x_max = rd.x;
        }
        if rd.z < region_z_min {
            region_z_min = rd.z;
        }
        if rd.z > region_z_max {
            region_z_max = rd.z;
        }
    });

    let img_width = ((region_x_max - region_x_min) + 1) as u32 * 32;
    let img_height = ((region_z_max - region_z_min) + 1) as u32 * 32;
    let mut img_buf = image::ImageBuffer::from_pixel(img_width, img_height, palette.bg());

    for r in region_data {
        let r_off_x = r.x - region_x_min;
        let r_off_z = r.z - region_z_min;
        for (i, &v) in r.inhabited_times.iter().enumerate() {
            let p_x_off = i % 32;
            let p_y_off = i / 32;

            if v == 0 {
                // we use the palettes background color for value 0
                // pixels are initialized with that color
                continue;
            }

            let px_col = mapping(v);

            img_buf.put_pixel(
                r_off_x as u32 * 32 + p_x_off as u32,
                r_off_z as u32 * 32 + p_y_off as u32,
                px_col,
            );
        }
    }
    Ok(img_buf)
}

const INHABITED_TIME_BYTES: &[u8] = b"\x04\x00\x0dInhabitedTime";

pub fn extract_inhabited_time(chunk_data: &[u8]) -> anyhow::Result<i64> {
    // expected byte sequence:
    // first byte: 4u8 - byte for a "long tag" in NBT format
    // next two bytes: 14u16 - (length of the tags name)
    // next 14 bytes: bytes for "InhabitedTime" (the name)
    // next 8 bytes: i64 payload (what we want)

    // this could technically find false positives but that's extremely unlikely,
    // and would not be catastrophic.

    let p = match chunk_data
        .windows(INHABITED_TIME_BYTES.len())
        .position(|window| window == INHABITED_TIME_BYTES)
    {
        Some(i) => i,
        None => anyhow::bail!("no inhabited time found"),
    };

    let payload_offset = p + INHABITED_TIME_BYTES.len();

    if chunk_data.len() < payload_offset + 8 {
        anyhow::bail!("no inhabited time found")
    }
    let payload_bytes = &chunk_data[payload_offset..payload_offset + 8];

    let inhabited_time = i64::from_be_bytes(payload_bytes.try_into()?);

    if inhabited_time < 0 {
        // likely garbage data
        anyhow::bail!("negative inhabited time");
    }
    Ok(inhabited_time)
}

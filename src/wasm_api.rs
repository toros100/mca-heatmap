use crate::loader::McaLoader;
use crate::palette::{Palette, default_palette};
use crate::{RegionData, extract_inhabited_time, make_heatmap};
use std::io;
use std::str::FromStr;
use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;

// apparently you can get rayon to work with wasm, but it requires special build config
// and certain headers (see  https://github.com/RReverser/wasm-bindgen-rayon).
// I am testing a different architecture, where there are multiple workers that have a McaProcessor,
// which extract inhabited time values concurrently and submit them to one Renderer.
// this does require more copying, but I assume the zlib decompression still dominates by far.
// more complicated to actually use in the browser, but I can use standard build stuff,
// and it even works on static site hosts that don't allow you to set headers.

#[wasm_bindgen]
pub struct HeatmapRenderer {
    region_data: Vec<RegionData>,
    palette: Palette,
}

#[wasm_bindgen]
impl HeatmapRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        HeatmapRenderer {
            region_data: Vec::new(),
            palette: default_palette(),
        }
    }

    /// Submit inhabited time data to be stored in the renderer.
    pub fn submit_data(
        &mut self,
        region_x: i32,
        region_z: i32,
        inhabited_times: &[i64],
    ) -> Result<(), JsError> {
        if inhabited_times.len() != 1024 {
            return Err(JsError::new("bad data (array length not 1024)"));
        };
        let mut rd = RegionData::new(region_x, region_z);
        rd.inhabited_times.copy_from_slice(inhabited_times);
        // todo maybe check for duplicate data?
        self.region_data.push(rd);
        Ok(())
    }
    /// Renders a heatmap using the stored region data, filtered by the specified region coordinate ranges (inclusive).
    /// Does not clear the data.
    pub fn render_range(
        &self,
        x_range_min: i32,
        x_range_max: i32,
        z_range_min: i32,
        z_range_max: i32,
    ) -> Result<Vec<u8>, JsError> {
        if (x_range_max < x_range_min) || (z_range_max < z_range_min) {
            return Err(JsError::new("invalid range: max < min"));
        }
        let data_refs: Vec<&RegionData> = self
            .region_data
            .iter()
            .filter(|r| {
                x_range_min <= r.x && r.x <= x_range_max && z_range_min <= r.z && r.z <= z_range_max
            })
            .collect();
        make_heatmap_helper(&self.palette, data_refs)
    }

    /// Renders a heatmap using the stored region data. Does not clear the data.
    pub fn render_all(&self) -> Result<Vec<u8>, JsError> {
        let data_refs = self.region_data.iter().collect();
        make_heatmap_helper(&self.palette, data_refs)
    }

    /// Resets the processor and discards stored region data. Does not reset the palette.
    pub fn reset(&mut self) {
        self.region_data.clear();
    }

    /// Specify a custom color palette. Colon-separated list of at least three RGB hex color codes with no whitespace and no '#'.
    /// The first color will determine the background color and the remaining colors will
    /// determine the gradient from "cold" to "hot" (lower to higher inhabited time values).
    /// "14001E:1E0997:AC00D9:D90000:D9A600:FFFFFF" reproduces the default palette.
    pub fn set_palette(&mut self, palette_string: &str) -> Result<(), JsError> {
        match Palette::from_str(palette_string) {
            Ok(p) => {
                self.palette = p;
                Ok(())
            }
            Err(e) => Err(JsError::new(&e.to_string())),
        }
    }

    /// Resets palette to the default palette.
    pub fn reset_palette(&mut self) {
        self.palette = default_palette()
    }
}

fn make_heatmap_helper(
    palette: &Palette,
    region_data: Vec<&RegionData>,
) -> Result<Vec<u8>, JsError> {
    match make_heatmap(palette, region_data) {
        Ok(buf) => {
            let img_data = Vec::new();

            let mut wr = io::Cursor::new(img_data);

            match buf.write_to(&mut wr, image::ImageFormat::Png) {
                Ok(_) => Ok(wr.into_inner()),
                Err(e) => Err(JsError::new(&e.to_string())),
            }
        }
        Err(e) => Err(JsError::new(&e.to_string())),
    }
}

#[wasm_bindgen]
pub struct McaProcessor {
    loader: McaLoader,
}

#[wasm_bindgen]
impl McaProcessor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> McaProcessor {
        let loader = McaLoader::new();
        McaProcessor { loader }
    }

    // Extracts and returns inhabited time values from one mca files data. The result vector
    // will always have length 1024 (number of chunks per region), missing values will be 0.
    pub fn extract_region_data(&mut self, data: &[u8]) -> Result<Vec<i64>, JsError> {
        self.extract_region_data_internal(data)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

impl McaProcessor {
    pub fn extract_region_data_internal(&mut self, data: &[u8]) -> anyhow::Result<Vec<i64>> {
        self.loader.load_mca(data)?;

        let mut inhabited_times = vec![0i64; 1024];

        for i in 0..1024 {
            let chunk_data = self.loader.get_chunk_data(i);
            if let Ok(cd) = chunk_data {
                if let Ok(inhabited_time) = extract_inhabited_time(cd) {
                    inhabited_times[i] = inhabited_time;
                }
            }
        }
        Ok(inhabited_times)
    }
}

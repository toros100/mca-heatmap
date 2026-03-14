use std::cell::RefCell;
use std::fs::{DirEntry, File};

use mca_heatmap::loader::*;
use mca_heatmap::palette::*;
use mca_heatmap::*;

use anyhow::Context;
use std::fs::read_dir;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use clap::Parser;
use image::ImageFormat;
use rayon::prelude::*;

thread_local! {
    static LOADER: RefCell<McaLoader> = RefCell::new(McaLoader::new());
}

#[derive(clap::Parser)]
struct Args {
    /// Path to a directory containing region files (*.mca). Omit to use current directory.
    #[arg(short, long, default_value = ".")]
    input: PathBuf,

    /// Output file path (png), e.g. "-o out.png".
    #[arg(short, long)]
    output: PathBuf,

    /// Specify a custom color palette. Colon-separated list of at least three RGB hex color codes with no whitespace and no '#'.
    /// The first color will determine the background color and the remaining colors will
    /// determine the gradient from "cold" to "hot" (lower to higher inhabited time values).
    /// "-c 14001E:1E0997:AC00D9:D90000:D9A600:FFFFFF" reproduces the default palette.
    #[arg(short, long, value_name = "HEX1:HEX2:HEX3:...")]
    custom_palette: Option<Palette>,

    /// Produces a test image for the selected palette.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    test_palette: bool,

    /// Limit region x coordinate to range MIN..MAX (inclusive, may omit MIN or MAX), e.g. "-x -3..17" or "-x ..17".
    /// Note that this refers to region coordinates, which are indicated by the region file name ("r.x.z.mca")
    /// and are not block or chunk coordinates.
    #[arg(short, long, value_name = "MIN..MAX", allow_hyphen_values = true)]
    x_range: Option<RegionRangeArg>,

    /// Limit region z coordinate to range MIN..MAX (inclusive, may omit MIN or MAX), e.g. "-z -3..17" or "-z ..17".
    /// Note that this refers to region coordinates, which are indicated by the region file name ("r.x.z.mca")
    /// and are not block or chunk coordinates.
    #[arg(short, long, value_name = "MIN..MAX", allow_hyphen_values = true)]
    z_range: Option<RegionRangeArg>,
}

#[derive(Clone, Copy)]
struct RegionRangeArg {
    min: Option<i32>,
    max: Option<i32>,
}

impl FromStr for RegionRangeArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split("..").collect::<Vec<_>>();
        if parts.len() != 2 {
            anyhow::bail!("failed to parse range (expected format MIN..MAX)")
        }
        let (mi, ma) = (parts[0], parts[1]);

        let min: Option<i32> = if mi.is_empty() {
            None
        } else {
            Some(mi.parse().context("failed to parse MIN")?)
        };
        let max: Option<i32> = if ma.is_empty() {
            None
        } else {
            Some(ma.parse().context("failed to parse MAX")?)
        };

        if let (Some(min), Some(max)) = (min, max) {
            if max < min {
                anyhow::bail!("empty range (MAX < MIN)")
            }
        }
        Ok(RegionRangeArg { min, max })
    }
}

impl RegionRangeArg {
    fn contains(&self, x: i32) -> bool {
        self.min.map_or(true, |min| min <= x) && self.max.map_or(true, |max| x <= max)
    }
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(args) {
        eprintln!("Error: {e:#}");
        exit(1);
    }
}

fn run(args: Args) -> anyhow::Result<()> {
    let palette = args.custom_palette.unwrap_or_else(|| default_palette());

    if args.test_palette {
        let test_image = palette.get_test_image();
        test_image.save_with_format(args.output, ImageFormat::Png)?;
        return Ok(());
    }

    let regions: Vec<Region> = parse_regions_in_dir(&args.input, args.x_range, args.z_range)?;
    let region_data = process_regions(regions);

    // a bit weird, but I needed Vec<&RegionData> instead of Vec<RegionData>
    // to be able to filter without copying in the wasm api
    // maybe there is a nicer way?
    let region_data_refs: Vec<&RegionData> = region_data.iter().collect();

    let img_buf = make_heatmap(&palette, region_data_refs)?;
    img_buf.save_with_format(args.output, ImageFormat::Png)?;
    Ok(())
}

fn parse_mca_file_name(e: DirEntry) -> Option<Region> {
    let pb = e.path();
    if !pb.is_file() {
        return None;
    }
    let s = pb.file_name()?.to_str()?;
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 || parts[0] != "r" || parts[3] != "mca" {
        return None;
    }
    Some(Region::new(
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        pb,
    ))
}

fn parse_regions_in_dir(
    path: &Path,
    x_range: Option<RegionRangeArg>,
    z_range: Option<RegionRangeArg>,
) -> anyhow::Result<Vec<Region>> {
    Ok(read_dir(path)?
        .filter_map(|entry| parse_mca_file_name(entry.ok()?))
        .filter(|region: &Region| {
            x_range.map_or(true, |x_rg| x_rg.contains(region.x))
                && z_range.map_or(true, |z_rg| z_rg.contains(region.z))
        })
        .collect())
}
fn process_regions(regions: Vec<Region>) -> Vec<RegionData> {
    regions
        .par_iter()
        .filter_map(|r| {
            LOADER.with(|loader| {
                let read = File::open(&r.path).ok()?;
                process_region(&mut *loader.borrow_mut(), read, r.x, r.z).ok()
            })
        })
        .collect()
}

fn process_region(
    loader: &mut McaLoader,
    r: impl io::Read,
    region_x: i32,
    region_z: i32,
) -> anyhow::Result<RegionData> {
    loader.load_mca(r)?;
    let mut rd = RegionData::new(region_x, region_z);
    for i in 0..1024 {
        if let Ok(data) = loader.get_chunk_data(i) {
            if let Ok(inhabited_time) = extract_inhabited_time(data) {
                rd.inhabited_times[i] = inhabited_time;
            }
        }
    }
    Ok(rd)
}

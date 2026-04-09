# mca-heatmap
Tool for generating neat looking heatmaps from Minecraft (Java Edition) .mca files, based on inhabited time values. These values are stored per chunk and count the amount of game ticks players spent with the chunk loaded.

## Example
Each pixel represents one chunk (16x16 square of blocks)

![sample image](/images/example.png)

## Usage/Features

```
Usage: mca-heatmap [OPTIONS] --output <OUTPUT>

Options:

  -i, --input <INPUT>
          Path to a directory containing region files (*.mca). Omit to use current
          directory [default: .]

  -o, --output <OUTPUT>
          Output file path (png), e.g. "-o out.png"

  -c, --custom-palette <HEX1:HEX2:HEX3:...>
          Specify a custom color palette. Colon-separated list of at least three RGB hex
          color codes with no whitespace and no '#'. The first color will determine the
          background color and the remaining colors will determine the gradient from
          "cold" to "hot" (lower to higher inhabited time values). 
          "-c 14001E:1E0997:AC00D9:D90000:D9A600:FFFFFF" reproduces the default palette

      --test-palette
          Produces a test image for the selected palette

  -x, --x-range <MIN..MAX>
          Limit region x coordinate to range MIN..MAX (inclusive, may omit MIN or MAX),
          e.g. "-x -3..17" or "-x ..17". Note that this refers to region coordinates,
          which are indicated by the region file name ("r.x.z.mca") and are not block
          or chunk coordinates

  -z, --z-range <MIN..MAX>
          Limit region z coordinate to range MIN..MAX (inclusive, may omit MIN or MAX),
          e.g. "-z -3..17" or "-z ..17". Note that this refers to region coordinates,
          which are indicated by the region file name ("r.x.z.mca") and are not block
          or chunk coordinates

  -h, --help
          Print help
```

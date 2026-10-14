# Colour figures on the 1-bit page (#114): measurements and decision

`pdfable --mineru layout.json copy.pdf out.pdf` draws MinerU's `image` and `chart` boxes in colour (150 dpi, JPEG q65,
3 pt padding) over the 1-bit page, from the colour copy (`--colour-copy`). Without `--mineru` the output is byte-identical
to main (`BASE_BIN=<main> tests/verify.sh`: all 10 real files identical, verify ok).

## Numbers (bytes; input stated per row, same input within a row)

| Input | Command | 1-bit | 1-bit + figures (+ text layer) | All colour (`MODE=images`) | Figures |
|---|---|---|---|---|---|
| Allan Poulsen, **split** colour copy (50 pp) | `--mineru layout.json copy.pdf` | 2 602 344 | 5 144 199 | 8 398 873 | 18, 2 123 706 B |
| Allan Poulsen original (25 spreads, no geometry flags) | `--mineru layout.json <original>` | 3 117 064 | 5 492 986 | 8 873 125 | 16, 2 142 756 B |
| Materialitet (11 pp, no geometry flags) | `--mineru layout.json copy.pdf` | 1 097 920 | 1 727 561 | 4 947 740 (from the earlier run; `MODE=images` writes nothing here: "Not smaller") | 5, 443 720 B |

Do not compare across rows: `--mineru` on the original and on the colour copy gives different 1-bit pages (the colour copy is
binarised again; on Allan Poulsen pages 22 and 23 grow from 164/187 KB to 557/670 KB), 5 492 986 vs 6 434 436 B.

Spread pictures (original pp. 5 and 7, split pages 9-10 and 13-14): both halves of both pictures are in colour, no cut
through the picture; a thin black strip from the 1-bit page remains under the picture box on p. 5 and a thin black sliver
at the right edge on p. 7 (outside the boxes, as in the 1-bit page). Examples: `docs/mineru-eksempler/` (not committed).

## Decision

`--mineru-figures auto|off`, default **auto**: with `--mineru layout.json` the figures are drawn in colour from the input (the
colour copy, or a colour scan with the 1-bit pages' geometry); `off` gives only the invisible text layer. Without `--mineru`
nothing changes (byte-identical to main). Since #126 a split run gets figures too (see below); before, it got none. Cost of auto: +98 % over 1-bit on the split Allan Poulsen (2 602 344 -> 5 144 199 B), +57 % on Materialitet, but
16-38 % of the scanned original and 39-65 % below all colour. `off` on the split copy: 3 015 449 B (1-bit + text layer).

Figures are 150 dpi against sources of 300 dpi (Allan Poulsen) and 200 dpi (Materialitet): never upscaled. MinerU's own
returned PDF (`*_origin.pdf`) is a copy of the upload, so it is no size reference.

## Line art stays 1-bit (#124)

A figure box with no real colour and no flat grey is not cut as a JPEG: the page's own 1-bit is already sharp there (sheet
music, buttons, line drawings, flowcharts). Test per box, on the page rendered at 100 dpi (`FIG_TEST_DPI`):

- colour share < 1 %: pixels with max-min of RGB > 40 (`FIG_COLOUR_MAX`);
- flat mid-tone share < 1.5 %: pixels with luminance 70-190 whose 3x3 neighbourhood varies by less than 30 levels
  (`FIG_FLAT_MAX`). Shares are of all pixels in the box.

Both must hold, then the box is dropped. The first idea (near-binary > 90 %) lost a greyscale photo and kept 21 line-art boxes in
colour (#123, `docs/mineru-geometry.md` question 4). Margin is narrow: line art at most 0.009, greyscale photos at least 0.020,
measured at 100 dpi only. Re-run the measurement before moving a number. The test lives in `fig_is_line_art` (`fig_stats` gives
the two shares), so #126 can use the same function. The thresholds are constants, not options.

Colour limit 1 %, not 2 %: PDFable's own renderer (hayro) measures less colour than pdftoppm. On the 13 files 2 % dropped the
pale-blue vacuum cleaner on Uge 38 p19 (2.4 % in pdftoppm); 1 % keeps it and gives #123's 19 dropped for Uge 38. The price is one
extra box on Haase p2 (a black-and-white skull drawing with a thin blue line from the box above, 1-2 % in hayro) that becomes a
JPEG: +6 KB, nothing lost. Both boxes sit in the same 1-2 % band, so no limit in between separates them. Raising the limit
drops more boxes (3 % also dropped a coffee machine and keys on Uge 38 p19).

## MinerU reads the 1-bit output; PDFable cuts the colour itself (#126)

Two runs with the **same** flags on the original:

```
pdfable --rotate --crop --split auto --deskew original.pdf out.1bit.pdf                      # 1: upload this to MinerU
pdfable --rotate --crop --split auto --deskew --mineru layout.json original.pdf out.pdf      # 2: text layer + colour figures
```

Run 2 makes the same output pages as run 1, so MinerU's boxes (pt, top-left) fit them directly: no mapping (#121). The page
count and page size checks are against output pages, as for the text layer. A layout from other flags fails those checks
and gives neither text nor figures. Upload: Allan Poulsen 2 602 344 B instead of 48 860 813 B for the colour copy (#123).

- **Where the colour comes from:** pass B renders the input page in colour (`colour_planes`) and cuts, straightens and crops
  it exactly like the 1-bit slot, then `place_rgb` puts it where `compose` puts the slot. Same code as the #82 colour copy,
  scaled to the figure dpi: `JPEG_DPI` for the JPEG and `FIG_TEST_DPI` for the tests. Only slots whose output page has
  MinerU figures are rendered (the boxes are read before pass B). Rounding: at most one 150 dpi pixel (0.5 pt).
- **Growing the box:** on the 1-bit page MinerU can return a box smaller than the picture (Allan Poulsen p13 lost its red
  banner, p10 stopped 28 pt short, #123). Each side moves out by `FIG_GROW_STEP_PT` (2 pt) while more than `FIG_GROW_MIN`
  (10 %) of the strip just outside is colour or flat mid-tone (the two pixel tests of #124, `fig_counts`), at most
  `FIG_GROW_MAX` (50 %) of the box's size per side, never past the page. Paper and black-on-white text are neither, so a
  box beside a text column stays put. A side that reaches the 50 % limit with picture still beyond it ran into a
  coloured panel or page tint, not a picture's edge, and goes back to MinerU's edge (first measurement: Allan p37's orange
  panel was cut to an L with its text doubled at the JPEG's edge; Læremidler p1's small KLIM logo became a big pale-green
  field over a text line). A side that did not move keeps MinerU's padded edge.
- **Line art:** `fig_is_line_art` on the grown box, cut from the `FIG_TEST_DPI` colour page. Without geometry flags that page
  is the same hayro render #124 measured (a zero-angle straighten is an exact shift); with `--deskew` it is bilinearly
  straightened, which the 13-file measurement above did not cover.
- `--colour-copy` is unchanged and still works as input (no flags, page i = page i), but is no longer needed.


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
nothing changes (byte-identical to main). A split run (more output pages than input pages) gets no figures and says so on
stderr. Cost of auto: +98 % over 1-bit on the split Allan Poulsen (2 602 344 -> 5 144 199 B), +57 % on Materialitet, but
16-38 % of the scanned original and 39-65 % below all colour. `off` on the split copy: 3 015 449 B (1-bit + text layer).

Figures are 150 dpi against sources of 300 dpi (Allan Poulsen) and 200 dpi (Materialitet): never upscaled. MinerU's own
returned PDF (`*_origin.pdf`) is a copy of the upload, so it is no size reference.

## Line art stays 1-bit (#124)

A figure box with no real colour and no flat grey is not cut as a JPEG: the page's own 1-bit is already sharp there (sheet
music, buttons, line drawings, flowcharts). Test per box, on the page rendered at 100 dpi (`FIG_TEST_DPI`):

- colour share < 3 %: pixels with max-min of RGB > 40 (`FIG_COLOUR_MAX`);
- flat mid-tone share < 1.5 %: pixels with luminance 70-190 whose 3x3 neighbourhood varies by less than 30 levels
  (`FIG_FLAT_MAX`). Shares are of all pixels in the box.

Both must hold, then the box is dropped. The first idea (near-binary > 90 %) lost a greyscale photo and kept 21 line-art boxes in
colour (#123, `docs/mineru-geometry.md` question 4). Margin is narrow: line art at most 0.009, greyscale photos at least 0.020,
measured at 100 dpi only. Re-run the measurement before moving a number. The test lives in `fig_is_line_art` (`fig_stats` gives
the two shares), so #126 can use the same function. The thresholds are constants, not options.

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

`--mineru` stays opt-in (a layout.json is needed anyway): when a layout.json is given the figures are drawn, with no extra
flag. Cost is +98 % over 1-bit on a picture-rich book (split Allan Poulsen) and +57 % on Materialitet, but 39-65 % below all
colour. Whether this should be the default for every run is the owner's decision; it cannot be without a MinerU result.

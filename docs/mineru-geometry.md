# MinerU's geometry and figure detection: what PDFable should copy (#123)

Investigation only, no code. Sources read: MinerU `mineru-3.4.4-released` (the version in our `layout.json` files) and
`main` at b7f3dd0 (4.0.12, 2026-10-11), the `docvortex` 0.5.16 wheel MinerU 4 depends on, hayro-syntax 0.7.2
(`src/page.rs`), and our own `layout.json` files in `~/MinerU`. Not read: the code behind mineru.net's API; that it runs
the open-source hybrid backend is inferred from the `_backend`/`_version_name`/`_effort` fields it writes.

## Answers

**1. Geometry.** MinerU has no geometry step of its own: no deskew, crop, split or page turn. It renders each page with
pdfium (`pypdfium2`) at 200 dpi, long side capped at 3500 px (`mineru/utils/pdf_reader.py: page_to_image`). pdfium applies
the page's `/Rotate` and crop box, so the picture is the page as a viewer shows it. `page_size` is that page's size in pt,
truncated to integers (`int(page.get_size())`, `hybrid_model_output_to_middle_json.py`), and every box is the model's 0-1
coordinate times `page_size`, again truncated (`hybrid_magic_model.py: cal_real_bbox`): pt, origin top-left, in the rotated
frame. The only turns are per table (`table_orientation_cls`) and for vertical text crops, neither touching page boxes.

That frame is exactly hayro's `render_dimensions()` (crop box ∩ media box, swapped for `/Rotate` 90/270), which PDFable
already renders in; `fit_boxes` and `mineru_text` already scale by `pw / page_size` and accept the 2 pt truncation.
Confirmed in the data: Materialitet's `page_size` is `[1190, 841]` (portrait page, `/Rotate`), and on the original without
geometry flags its 5 figures land right (#114). So **#121 needs nothing more from MinerU**: its task "MinerU's frame vs the
source page: /Rotate and MinerU's own turn" is settled (there is no own turn). What is left of #121 is PDFable's own chain
per source page: `rot90` (osd) → page deskew (`m.angle`) → crop/split box → per-slot deskew about the slot centre into the
`cw × ch` canvas → `content_box` → `compose` placement. Source px = MinerU pt × dpi / 72 before all of it.

**2. Which idea.** (a), geometry only, is #121 and needs no MinerU code. The #123 idea, deciding figures without MinerU,
is (b).

**3. (b), layout detection.** In our runs the figure boxes do not come from the VLM. 21 of the 31 results in `~/MinerU` say
`_backend: hybrid`, `_version_name` 3.4.0/3.4.4, `_effort: medium`; in that path (`hybrid_analyze.py`, `effort ==
"medium"`) the small pipeline model **PP-DocLayoutV2** places all boxes and the VLM only reads what is inside them.
`image`, `seal` → image, `chart` → chart, class threshold 0.5.

- Model: PaddlePaddle PP-DocLayoutV2, Apache-2.0, 25 classes, 800×800 input. ONNX `inference.onnx` is 214 MB
  (huggingface.co/PaddlePaddle/PP-DocLayoutV2_onnx; PaddleOCR lists 203.8 MB). Its detector half is PP-DocLayout_plus-L:
  510 ms per page on CPU (PaddleOCR 3.0 table). The small relatives: PP-DocLayout-S 4.8 MB (mAP 70.9, ~10 ms CPU),
  -M 22.6 MB (75.2), -L 123.8 MB (90.4).
- Rust: **oar-ocr** 0.10.0 (github.com/greatv/oar-ocr, Apache-2.0) runs PP-DocLayout S/M/L, Plus-L and V2/V3 through
  `ort` (ONNX Runtime, MIT/Apache) and already depends on hayro 0.7 like PDFable. It can load a model from bytes
  (`include_bytes!`), but ONNX Runtime is a native library (downloaded at build time by default), so "one small binary"
  ends: + ONNX Runtime + 5-214 MB of model. ItsWiz (#111) would have to download it.
- DocLayout-YOLO (in MinerU's desktop config as `layout_model`) is AGPL-3.0: not usable in PDFable.
- MinerU 4 moved PDF handling into DocVortex (Rust core via pyo3, MIT/Apache/BSD); nothing there we need.

What it would not fix: a local PP-DocLayoutV2 gives the same boxes, so sheet music would still be "image". And MinerU is
still needed for the text layer (#113), so the original is uploaded anyway; the extra upload (the colour copy) goes away with
#121 alone. Nothing in MinerU's Python is worth porting for figures: its cut is `pixel box = pt box × scale`.

**4. Rule-based figure decision, measured.** On the 13 files (the 11 of #114, Allan Poulsen original, Materialitet), MinerU's
`image`/`chart` bodies on the uploaded original (`*_origin.pdf`), rendered at 100 dpi (pdftoppm). Each low-colour box was
looked at on contact sheets.

- *#124's rule* (colour < 2 % **and** near-binary > 90 %): 343 boxes, 123 dropped. It loses **one greyscale photo**
  (Haase p11, a baby's face, near-binary 0.904) and keeps 21 boxes that are line art in colour JPEG: the "Form" buttons
  (Gennem byen p7, p11, 0.821/0.784), staves at 0.886-0.899, icons, logos, a flowchart (Allan Poulsen p1, 0.898). Photos
  reach 0.904, line art goes down to 0.78: one near-binary threshold cannot separate them.
- *Better second test: flat mid-tone* = share of pixels with luminance 70-190 whose 3×3 neighbourhood varies less than 30
  levels. Line art has its grey only on stroke edges (not flat); a photo has flat grey. Line art: at most 0.009. Greyscale
  photos: at least 0.020 (the Haase baby; Gennem byen p17 photo 0.027; Gandhi 0.192, AJAX advert 0.156). Rule colour < 2 %
  **and** flat mid-tone < 1.5 %: 142 dropped, no photo lost, Form buttons and staves dropped. The margin is narrow
  (0.009 vs 0.020) and measured at 100 dpi only.

| File | boxes | dropped, #124 rule | dropped, flat mid-tone |
|---|---|---|---|
| Begrundelse | 0 | 0 | 0 |
| Gennem byen 1. | 79 | 65 | 74 |
| JEPPE slides | 12 | 2 | 1 |
| Sange med bevægelse | 50 | 34 | 38 |
| Uge 38 E26 Signe | 36 | 15 | 19 |
| Fought 2019 | 37 | 1 | 2 |
| Gissel 2016 | 4 | 0 | 1 |
| Jørn Ingemann | 12 | 0 | 0 |
| Kopierne | 28 | 1 | 2 |
| Haase | 62 | 4 | 3 |
| Produktiv læring | 2 | 0 | 0 |
| Allan Poulsen (original) | 16 | 1 | 2 |
| Materialitet | 5 | 0 | 0 |

  JEPPE p13 (staves on a grey slide with 1.7 % colour) moves from dropped to kept: costs bytes, loses nothing.
- *A detector with no boxes at all* (2.5 mm tiles with > 20 % colour or > 35 % mid-tone, closed, ≥ 1.5 cm²) is not usable:
  only 0-71 % of what it marks lies in a MinerU figure, it covers 16-87 % of the figures, and it marks large areas on
  Begrundelse, a plain text scan with no figures.
- Page tint: the median colour of the paper outside figures is 0 on every file; the "max-min > 40" colour test is not
  tripped by paper tint here (higher values only on slides/pages with coloured backgrounds: JEPPE 17, Kopierne 51,
  Ingemann 57).

**5. Recommendation: do not build (b); send MinerU the 1-bit output instead (#126), which makes (a)/#121 unnecessary.**
Keep MinerU's boxes and decide colour ourselves per box with the flat mid-tone test (#124, threshold 1.5 % instead of
near-binary 90 %). (b) would add ONNX Runtime and a 5-214 MB model for boxes we already get.

Follow-up test (2026-10-14, batch `2f9395f4-…`): MinerU on `pdfable --rotate --crop --split auto --deskew` output.
Allan Poulsen 2 602 344 B against 48 860 813 B for the colour copy; all 18 figures found again (box overlap ≥ 0.5, 14 at
≥ 0.86), text word similarity 0.9898; Materialitet 5 of 5 figures, 0.9932. The boxes already fit the output pages, so no
mapping. Catch: on 1-bit two boxes came back smaller than the picture (Allan p13 loses the red banner at the top, p10 stops
about 28 pt short), so #126 grows each box to the picture's edge on PDFable's own colour page. Details on #123.

Measurement scripts, numbers and contact sheets: `~/Developer/PDFable-123-sheets/` (not committed).

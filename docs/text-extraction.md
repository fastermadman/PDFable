# Markdown straight from a digital PDF's text layer (#134)

Experiment: can PDFable write the note for a digital PDF itself, with page markers, instead of sending it to MinerU?
Code: `rust/src/text.rs`, flag `pdfable --text input.pdf > note.md` (report per page on stderr). Not wired into ItsWiz.
Compare script: `tests/text-compare.py mineru.md pdfable.md`. Read: hayro-interpret 0.7.0 `src/font/mod.rs`
(`Glyph::as_unicode`, `advance_width`), `src/interpret/text.rs`; ItsWiz `plugin/main.js` (`insertPageMarkers`,
`formatPageMarker`, `printedPageNumber`). Not read: ValdiVault `06-start-her.md` (no vault access from the cloud session).

## How it works

hayro already reports every glyph it draws with its Unicode text, transform and advance width, so no new dependency and no
OCR. Per page: glyphs → rows (shared baseline; small raised glyphs join the row they sit on) → fragments (split at gaps over
1.5 letter sizes, or over 0.6 where a gap lines up over many rows: a column gutter) → running headers/footers and page
numbers out → footnotes (small text with no body text below it) → reading order: strips of overlapping fragments grouped
into sections that keep the same gutters, read column by column, recursing into each column (an inset table) → lines →
paragraphs, headings (size ≥ 1.15 × body, or a bold one-line paragraph), list items, hyphen joins. A section of short cells
in 3+ real columns is written as a pipe table.

Page labels: a number standing alone, or first/last in a header/footer line, is a candidate. When candidates on several
pages sit at the same offset from the PDF page index, every page gets index + offset (so excerpts get 190, 191, … and
chapter openers without a printed number follow; a year or ISSN never keeps one offset). Spreads print "a–b". Otherwise the
PDF page number. Markers and footnotes use exactly ItsWiz's `formatPageMarker` text.

stderr report per page: `page label kind glyphs unmapped rotated tables image_share` (kind = digital/scan; image_share =
share of the page under pictures, whose text the text layer does not have).

## Measurements (round 3, commit 611177b, 2026-10-15)

15 RAW PDFs from ViaVault that are fully digital (`--classify` all D) and already have a MinerU note (ItsWiz), compared
with that note's body. Nothing was uploaded. Run on the Mac; files in `~/Developer/PDFable-134-sheets/`.
"pic pages" = pages with image_share ≥ 0.15.

| file | pages | seconds | MinerU words in PDFable | PDFable words in MinerU | order kept | MinerU's page labels found | pic pages |
|---|---|---|---|---|---|---|---|
| rosaC | 17 | 0.62* | 0.986 | 0.991 | 0.952 | 14/17 | 0 |
| buber8C | 60 | 0.05 | 0.996 | 0.980 | 0.941 | 50/57 | 0 |
| lybke | 52 | 0.06 | 0.997 | 0.996 | 0.987 | 51/51 | 1 |
| revital | 20 | 0.01 | 0.982 | 0.977 | 0.976 | 6/26 | 0 |
| madariaga | 12 | 0.01 | 0.891 | 0.987 | 0.885 | 12/12 | 1 |
| raskin | 11 | 0.01 | 0.997 | 0.991 | 0.997 | 11/11 | 0 |
| ronlarsen | 13 | 0.02 | 0.978 | 0.983 | 0.975 | 13/13 | 0 |
| materialme | 19 | 0.02 | 0.775 | 0.995 | 0.769 | 18/18 | 13 |
| rosesuhr | 21 | 0.03 | 0.990 | 0.974 | 0.973 | 17/21 | 0 |
| dyregrov | 28 | 0.04 | 0.975 | 0.977 | 0.975 | 28/28 | 1 |
| modernitet | 25 | 0.03 | 0.985 | 0.974 | 0.928 | 24/25 | 4 |
| buberped | 9 | 0.03 | 1.000 | 1.000 | 1.000 | 9/9 | 0 |
| holgersen | 17 | 0.00 | 0.999 | 0.998 | 0.999 | 17/17 | 0 |
| webinar275 | 6 | 0.01 | 0.986 | 0.953 | 0.668 | 6/6 | 4 |
| afrapport | 9 | 0.02 | 0.978 | 0.961 | 0.977 | 8/8 | 0 |

\* cold first file of the run. "Order kept" = share of MinerU's word sequence found in the same order (difflib matching
blocks). Where labels differ, the round-2 dumps showed MinerU wrong, not PDFable: rosaC (MinerU gives "3", "15", "17" for
192, 204, 206 from chapter-heading numbers), revital (MinerU has 28 markers for 20 pages, "2023" for p. 2). On a synthetic
2-column article with known text (Chromium-made, header, footer numbers, table, list) PDFable got 1331/1331 words and
0.999 order; `pdftotext` without layout got 0.567 order.

What changed between rounds: round 1 gave excerpts the PDF page index (materialme 0/18, madariaga 1/12, rosaC 3/17) and
interleaved ronlarsen's columns (order 0.533, a 1.1-letter gutter bridged by space glyphs). Both fixed.

## What is still worse than MinerU (known limits)

- **Text inside pictures** (materialme p. 1, madariaga's screenshots of student work): only MinerU reads it. image_share
  flags materialme (13 pages) but not madariaga's screenshots (only p. 4 reaches 0.15), so the flag misses some.
- **Ligatures without a Unicode map** ("fi"/"fl" in ronlarsen, modernitet: 39 and 63 unmapped glyphs, "kon ikter",
  "indes"). MinerU drops the same glyph in ronlarsen.
- **Card layouts** (webinar275): words are all there (0.986), but the cards come in a different order than MinerU's.
- **Footnotes at a page break** (modernitet): a sentence running over the page gets the page's footnotes and marker in
  the middle ("by- [notes] livets"); MinerU joins the word first. Same marker format either way.
- **Leftover header/footer text**: materialme's first-page footer differs from the others ("ISSN 2369-8721 | …") and stays
  in the body; ronlarsen's p. 2 running head "Nordiske udkast • Nr. 2 • 2013" stays.
- Tables: detected on 3 files (buber8C, rosesuhr, webinar275); their quality was not checked against MinerU.

## Recommendation

Good enough for digital files without pictures: on the 9 files with no page at image_share ≥ 0.15 every one has ≥ 0.978
of MinerU's words and ≥ 0.94 order, in hundredths of a second and with no upload. Page labels agree with MinerU or MinerU
was shown wrong, except buber8C (50/57) and rosesuhr (17/21), which were not checked page by page.
A rule for ItsWiz to try (new issue, not this PR): `--text` when `--classify` is all D, no page has image_share ≥ 0.15 and
unmapped glyphs are under 1 %; otherwise MinerU as today. madariaga shows the picture flag is not yet complete, so keep
MinerU as the default until the flag also catches screenshot pages.

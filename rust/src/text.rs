// --text input.pdf (#134, experiment): Markdown from a digital PDF's own text layer, with the page markers ItsWiz writes for
// MinerU's notes (`*— side N —*`, printed page number when there is one). Nothing here touches the 1-bit pipeline.
//
// Steps per page: glyphs (hayro) -> rows (shared baseline) -> fragments (split at wide gaps) -> running headers/footers and page
// numbers out -> footnotes (small text below the body) -> reading order (strips grouped into sections that keep the same column
// gutters, read column by column) -> lines -> paragraphs and headings. A section of short cells in 3+ columns is written as a pipe
// table. stderr gets one report row per page, so a caller can see which pages to leave to MinerU (scan, unmapped glyphs, tables).
use crate::page_is_digital;
use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::page::Page;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode,
    SoftMask, TransformExt, interpret_page,
};
use kurbo::{Affine, BezPath, Rect, Shape};

const FRAG_GAP: f64 = 1.5; // a sideways gap wider than this (letter sizes) ends a fragment: column gutters, table cells
const WORD_GAP: f64 = 0.18; // a gap wider than this (letter sizes) between glyphs without a space glyph is a word space
const EDGE_FRAC: f64 = 0.12; // running headers/footers and page numbers sit in this share of the page height at the top/bottom
const HEADING_SIZE: f64 = 1.15; // a line this much larger than the body size is a heading
const SMALL_SIZE: f64 = 0.85; // footnote text is at most this share of the body size
const MAX_HEADING_WORDS: usize = 25;

#[derive(Clone, Debug)]
pub struct Ch {
    pub s: String,
    pub x0: f64,
    pub x1: f64,
    pub y: f64, // baseline, y down
    pub size: f64,
    pub bold: bool,
}

#[derive(Default)]
pub struct PageText {
    pub h: f64,
    pub chars: Vec<Ch>,
    pub digital: bool,
    pub unmapped: usize,
    pub rotated: usize,
}

/// Device that keeps every visible, upright glyph with its Unicode text, position and size.
struct Collect {
    chars: Vec<Ch>,
    unmapped: usize,
    rotated: usize,
}

impl<'a> Device<'a> for Collect {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph(&mut self, g: &Glyph<'a>, t: Affine, gt: Affine, _: &Paint<'a>, m: &GlyphDrawMode) {
        if matches!(m, GlyphDrawMode::Invisible) {
            return;
        }
        // Glyph space is 1000 units per em; y up in glyph space is y down on the page.
        let [a, b, c, d, e, f] = (t * gt).as_coeffs();
        if a <= 0.0 || d >= 0.0 || b.abs() > 0.05 * a || c.abs() > 0.05 * -d {
            self.rotated += 1;
            return;
        }
        let size = -d * 1000.0;
        let Some(u) = g.as_unicode() else {
            self.unmapped += 1;
            return;
        };
        let s = match u {
            BfString::Char(c) => lig(c),
            BfString::String(s) => s.chars().map(lig).collect(),
        };
        if s.chars().all(|c| c.is_control()) {
            return;
        }
        let (adv, bold) = match g {
            Glyph::Outline(o) => {
                let fd = o.font_data();
                let bold = fd.as_ref().is_some_and(|f| {
                    f.weight.is_some_and(|w| w >= 600)
                        || f.postscript_name.as_deref().is_some_and(|n| ["Bold", "Black", "Heavy", "Semibold"].iter().any(|k| n.contains(k)))
                });
                let adv = o.advance_width().map(f64::from).filter(|w| *w > 0.0).unwrap_or_else(|| o.outline().bounding_box().x1.max(500.0));
                (adv, bold)
            }
            Glyph::Type3(_) => (500.0, false),
        };
        // FillStroke draws each glyph twice.
        if self.chars.last().is_some_and(|l| l.s == s && (l.x0 - e).abs() < 0.1 * size && (l.y - f).abs() < 0.1 * size) {
            return;
        }
        self.chars.push(Ch { s, x0: e, x1: e + adv * a, y: f, size, bold });
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

fn lig(c: char) -> String {
    match c {
        '\u{FB00}' => "ff".into(),
        '\u{FB01}' => "fi".into(),
        '\u{FB02}' => "fl".into(),
        '\u{FB03}' => "ffi".into(),
        '\u{FB04}' => "ffl".into(),
        '\u{FB05}' | '\u{FB06}' => "st".into(),
        '\u{00A0}' => " ".into(),
        c => c.to_string(),
    }
}

pub fn page_text(page: &Page) -> PageText {
    let (w, h) = page.render_dimensions();
    let mut dev = Collect { chars: vec![], unmapped: 0, rotated: 0 };
    let ic = InterpreterCache::new();
    let mut ctx = Context::new(page.initial_transform(true).to_kurbo(), Rect::new(0.0, 0.0, w as f64, h as f64), &ic, page.xref(), InterpreterSettings::default());
    interpret_page(page, &mut ctx, &mut dev);
    PageText { h: h as f64, chars: dev.chars, digital: page_is_digital(page), unmapped: dev.unmapped, rotated: dev.rotated }
}

// ── Pure layout logic from here on (tested below) ──

#[derive(Clone, Debug)]
struct Frag {
    x0: f64,
    x1: f64,
    base: f64,
    size: f64,
    bold: bool,
    text: String,
}

impl Frag {
    fn top(&self) -> f64 {
        self.base - 0.7 * self.size
    }
    fn bot(&self) -> f64 {
        self.base + 0.2 * self.size
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Glyphs -> fragments: rows of a shared baseline (raised/lowered small glyphs join the row they sit on), split at wide gaps.
fn fragments(chars: &[Ch]) -> Vec<Frag> {
    let mut cs: Vec<&Ch> = chars.iter().filter(|c| c.size > 0.5).collect();
    cs.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x0.total_cmp(&b.x0)));
    let mut rows: Vec<Vec<&Ch>> = vec![];
    for c in cs {
        match rows.last_mut() {
            Some(r) if c.y - r[0].y <= 0.3 * c.size.min(r[0].size) => r.push(c),
            _ => rows.push(vec![c]),
        }
    }
    // Superscripts/subscripts: a row of clearly smaller glyphs inside a larger row's band joins that row (footnote markers).
    let size_of = |r: &Vec<&Ch>| median(r.iter().map(|c| c.size).collect());
    let base_of = |r: &Vec<&Ch>| median(r.iter().map(|c| c.y).collect());
    let mut merged = vec![false; rows.len()];
    for i in 0..rows.len() {
        let (si, bi) = (size_of(&rows[i]), base_of(&rows[i]));
        let (lo, hi) = (rows[i].iter().map(|c| c.x0).fold(f64::MAX, f64::min), rows[i].iter().map(|c| c.x1).fold(f64::MIN, f64::max));
        let host = (0..rows.len()).find(|&j| {
            let (sj, bj) = (size_of(&rows[j]), base_of(&rows[j]));
            j != i
                && !merged[j]
                && sj >= 1.25 * si
                && bi > bj - 0.8 * sj
                && bi < bj + 0.3 * sj
                && rows[j].iter().any(|c| c.x1 > lo - sj && c.x0 < hi + sj)
        });
        if let Some(j) = host {
            let moved: Vec<&Ch> = rows[i].drain(..).collect();
            rows[j].extend(moved);
            merged[i] = true;
        }
    }
    let mut out = vec![];
    for mut r in rows.into_iter().filter(|r| !r.is_empty()) {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        let row_base = base_of(&r);
        let mut cur: Vec<&Ch> = vec![];
        let mut flush = |cur: &mut Vec<&Ch>| {
            if cur.is_empty() {
                return;
            }
            let mut text = String::new();
            let mut prev: Option<&Ch> = None;
            for c in cur.iter() {
                if let Some(p) = prev
                    && c.x0 - p.x1 > WORD_GAP * c.size.max(p.size)
                    && !text.ends_with(' ')
                    && !c.s.starts_with(' ')
                {
                    text.push(' ');
                }
                if !(c.s == " " && text.ends_with(' ')) {
                    text.push_str(&c.s);
                }
                prev = Some(c);
            }
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                let size = median(cur.iter().filter(|c| !c.s.trim().is_empty()).map(|c| c.size).collect());
                let body: Vec<&&Ch> = cur.iter().filter(|c| c.size >= 0.8 * size && !c.s.trim().is_empty()).collect();
                out.push(Frag {
                    x0: cur.iter().map(|c| c.x0).fold(f64::MAX, f64::min),
                    x1: cur.iter().map(|c| c.x1).fold(f64::MIN, f64::max),
                    base: row_base,
                    size,
                    bold: !body.is_empty() && body.iter().all(|c| c.bold),
                    text,
                });
            }
            cur.clear();
        };
        for c in r {
            if let Some(p) = cur.iter().map(|c| c.x1).reduce(f64::max)
                && c.x0 - p > FRAG_GAP * c.size
            {
                flush(&mut cur);
            }
            if !(cur.is_empty() && c.s.trim().is_empty()) {
                cur.push(c);
            }
        }
        flush(&mut cur);
    }
    out
}

/// "12", "- 12 -", "Side 12", "page 3 of 10", "12 / 24" -> 12.
fn page_number(text: &str) -> Option<u32> {
    let t = text.trim().trim_matches(|c: char| "-–—|·•()[] ".contains(c)).to_lowercase();
    let t = ["side ", "page ", "s. ", "p. "].iter().fold(t.as_str(), |t, p| t.strip_prefix(p).unwrap_or(t));
    let num = |w: &str| !w.is_empty() && w.len() <= 4 && w.chars().all(|c| c.is_ascii_digit());
    let w: Vec<&str> = t.split([' ', '/']).filter(|w| !w.is_empty()).collect();
    let ok = match w.as_slice() {
        [n] => num(n),
        [n, m] => num(n) && num(m) && t.contains('/'),
        [n, of, m] => num(n) && num(m) && ["of", "af"].contains(of),
        _ => false,
    };
    ok.then(|| w[0].parse().ok()).flatten()
}

fn norm(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect::<String>().to_lowercase()
}

pub struct PageReport {
    pub label: String,
    pub digital: bool,
    pub chars: usize,
    pub unmapped: usize,
    pub rotated: usize,
    pub tables: usize,
}

struct Line {
    x0: f64,
    x1: f64,
    base: f64,
    size: f64,
    bold: bool,
    text: String,
    col_start: bool, // first line of a column/section: the previous line's geometry does not apply
}

enum Block {
    Lines(Vec<Line>),
    Table(Vec<Vec<String>>),
}

pub fn to_markdown(pages: &[PageText]) -> (String, Vec<PageReport>) {
    let frags: Vec<Vec<Frag>> = pages.iter().map(|p| if p.digital { fragments(&p.chars) } else { vec![] }).collect();
    let body = {
        let mut v = vec![];
        for f in frags.iter().flatten() {
            for _ in 0..f.text.chars().count() {
                v.push((f.size * 2.0).round() / 2.0);
            }
        }
        mode(&v).unwrap_or(10.0)
    };

    // Running headers/footers: an edge fragment whose text (digits ignored) repeats on several pages. Page numbers: an edge
    // fragment that reads as a number.
    let edge = |p: &PageText, f: &Frag| f.bot() < EDGE_FRAC * p.h || f.top() > (1.0 - EDGE_FRAC) * p.h;
    let outer = |fs: &[Frag], f: &Frag, p: &PageText| {
        // nothing else lies between this fragment and the page edge
        // and it stands clear of the text block: no line within 1.8 letter heights on the page-centre side
        let top = f.top() < p.h / 2.0;
        let (d0, d1) = (0.3 * f.size, 1.8 * f.size);
        !fs.iter().any(|g| g.x1 > f.x0 && g.x0 < f.x1 && if top { g.base < f.base - d0 } else { g.base > f.base + d0 })
            && !fs.iter().any(|g| if top { g.base > f.base + d0 && g.base < f.base + d1 } else { g.base < f.base - d0 && g.base > f.base - d1 })
    };
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for (p, fs) in pages.iter().zip(&frags) {
        let mut seen = std::collections::HashSet::new();
        for f in fs.iter().filter(|f| edge(p, f)) {
            if seen.insert(norm(&f.text)) {
                *counts.entry(norm(&f.text)).or_default() += 1;
            }
        }
    }
    let repeat_min = 2.max(pages.len() / 4);
    let mut numbers: Vec<Vec<u32>> = vec![];
    let mut kept: Vec<Vec<Frag>> = vec![];
    for (p, fs) in pages.iter().zip(&frags) {
        let mut nums = vec![];
        let mut keep = vec![];
        for f in fs {
            if edge(p, f) && outer(fs, f, p) {
                if let Some(n) = page_number(&f.text) {
                    nums.push(n);
                    continue;
                }
                if counts.get(&norm(&f.text)).is_some_and(|&c| c >= repeat_min) {
                    continue;
                }
            }
            keep.push(f.clone());
        }
        nums.sort();
        nums.dedup();
        numbers.push(nums);
        kept.push(keep);
    }
    let labels = page_labels(&numbers);

    let mut heading_sizes: Vec<f64> = kept.iter().flatten().filter(|f| f.size >= HEADING_SIZE * body).map(|f| (f.size * 2.0).round() / 2.0).collect();
    heading_sizes.sort_by(|a, b| b.total_cmp(a));
    heading_sizes.dedup();
    let level = |size: f64| heading_sizes.iter().position(|&s| (size * 2.0).round() / 2.0 >= s).map_or(heading_sizes.len() + 1, |i| i + 1).min(4);

    let mut out = String::new();
    let mut reports = vec![];
    for (i, (p, fs)) in pages.iter().zip(kept).enumerate() {
        // Footnotes: small text with no body-size text below it in its own column.
        let is_note = |f: &Frag, fs: &[Frag]| {
            f.size <= SMALL_SIZE * body
                && f.top() > p.h / 2.0
                && !fs.iter().any(|g| g.size > SMALL_SIZE * body && g.x1 > f.x0 && g.x0 < f.x1 && g.base > f.base)
        };
        let (notes, rest): (Vec<Frag>, Vec<Frag>) = fs.iter().cloned().partition(|f| is_note(f, &fs));
        let blocks = reading_order(rest, body);
        let tables = blocks.iter().filter(|b| matches!(b, Block::Table(_))).count();
        let mut paras: Vec<String> = vec![];
        for b in blocks {
            match b {
                Block::Lines(ls) => paragraphs(&ls, body, &level, &mut paras),
                Block::Table(rows) => paras.push(table_md(&rows)),
            }
        }
        let mut notes = notes;
        notes.sort_by(|a, b| a.x0.total_cmp(&b.x0).then(a.base.total_cmp(&b.base)));
        let notes = footnotes(&notes);
        for para in &paras {
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push_str("\n\n");
            }
            out.push_str(para);
        }
        let last = i + 1 == pages.len();
        out.truncate(out.trim_end_matches('\n').len());
        out.push_str(&page_marker(&labels[i], &notes, !last));
        reports.push(PageReport { label: labels[i].clone(), digital: p.digital, chars: p.chars.len(), unmapped: p.unmapped, rotated: p.rotated, tables });
    }
    (out.trim_start().to_string(), reports)
}

fn mode(v: &[f64]) -> Option<f64> {
    let mut m = std::collections::HashMap::<u64, usize>::new();
    for x in v {
        *m.entry(x.to_bits()).or_default() += 1;
    }
    m.into_iter().max_by_key(|&(k, c)| (c, k)).map(|(k, _)| f64::from_bits(k))
}

/// One label per page: the printed number where the pages agree on an offset from the PDF page index (pages without a printed
/// number, e.g. chapter openers, follow it); two numbers on one page (a spread) print as "a–b"; otherwise the PDF page number.
fn page_labels(numbers: &[Vec<u32>]) -> Vec<String> {
    let offs: Vec<i64> = numbers.iter().enumerate().filter(|(_, n)| n.len() == 1).map(|(i, n)| n[0] as i64 - i as i64).collect();
    let k = {
        let mut best = None;
        for &o in &offs {
            let c = offs.iter().filter(|&&x| x == o).count();
            if c >= 2 && c * 2 >= offs.len() && best.is_none_or(|(_, bc)| c > bc) {
                best = Some((o, c));
            }
        }
        best.map(|b| b.0)
    };
    numbers
        .iter()
        .enumerate()
        .map(|(i, n)| match (n.as_slice(), k) {
            ([a, .., b], _) => format!("{a}–{b}"),
            (_, Some(k)) if i as i64 + k >= 1 => (i as i64 + k).to_string(),
            ([a], _) => a.to_string(),
            _ => (i + 1).to_string(),
        })
        .collect()
}

/// Strips (runs of fragments that overlap vertically) grouped into sections that keep the same column gutters; a section is
/// read column by column, each column top to bottom.
fn reading_order(mut fs: Vec<Frag>, body: f64) -> Vec<Block> {
    let gmin = body;
    fs.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let mut strips: Vec<Vec<Frag>> = vec![];
    let mut bot = f64::MIN;
    for f in fs {
        if f.top() < bot - 0.05 * f.size
            && let Some(s) = strips.last_mut()
        {
            bot = bot.max(f.bot());
            s.push(f);
        } else {
            bot = f.bot();
            strips.push(vec![f]);
        }
    }
    let spans = |s: &[Frag]| {
        let mut v: Vec<(f64, f64)> = s.iter().map(|f| (f.x0, f.x1)).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut m: Vec<(f64, f64)> = vec![];
        for (a, b) in v {
            match m.last_mut() {
                Some(l) if a <= l.1 => l.1 = l.1.max(b),
                _ => m.push((a, b)),
            }
        }
        m
    };
    let interior = |s: &[Frag]| spans(s).windows(2).map(|w| (w[0].1, w[1].0)).filter(|g| g.1 - g.0 >= gmin).collect::<Vec<_>>();
    // A gutter narrowed by a strip's spans: the widest free piece left, if still wide enough.
    let narrow = |g: (f64, f64), sp: &[(f64, f64)]| -> Option<(f64, f64)> {
        let mut free = vec![g];
        for &(a, b) in sp {
            free = free.into_iter().flat_map(|(x, y)| [(x, y.min(a)), (x.max(b), y)]).filter(|p| p.1 - p.0 > 0.0).collect();
        }
        free.into_iter().filter(|p| p.1 - p.0 >= gmin).max_by(|p, q| (p.1 - p.0).total_cmp(&(q.1 - q.0)))
    };
    let mut sections: Vec<(Vec<Frag>, Vec<(f64, f64)>)> = vec![];
    // A strip without gutters of its own that fits the columns of a strip just below it (a column's first line standing higher
    // than the other column's) starts that column section, so it is not read before the other column's first lines.
    let ahead = |k: usize, sp: &[(f64, f64)]| -> Vec<(f64, f64)> {
        let Some(g) = strips[k + 1..].iter().take(3).map(|s| interior(s)).find(|g| !g.is_empty()) else { return vec![] };
        let ng: Vec<(f64, f64)> = g.iter().filter_map(|&g| narrow(g, sp)).collect();
        if ng.len() == g.len() { ng } else { vec![] }
    };
    for k in 0..strips.len() {
        let s = &strips[k];
        let sp = spans(s);
        if let Some((cur, gut)) = sections.last_mut() {
            if !gut.is_empty() {
                let ng: Vec<(f64, f64)> = gut.iter().filter_map(|&g| narrow(g, &sp)).collect();
                if ng.len() == gut.len() {
                    *gut = ng;
                    cur.extend(s.iter().cloned());
                    continue;
                }
            } else if interior(s).is_empty() && ahead(k, &sp).is_empty() {
                cur.extend(s.iter().cloned());
                continue;
            }
        }
        let g = interior(s);
        let g = if g.is_empty() { ahead(k, &sp) } else { g };
        sections.push((s.clone(), g));
    }
    let mut blocks: Vec<Block> = vec![];
    // Consecutive line runs form one block, so a paragraph can run on across sections.
    let push_lines = |blocks: &mut Vec<Block>, mut ls: Vec<Line>, col_start: bool| {
        if let Some(l) = ls.first_mut() {
            l.col_start |= col_start;
        }
        match blocks.last_mut() {
            Some(Block::Lines(prev)) => prev.extend(ls),
            _ if !ls.is_empty() => blocks.push(Block::Lines(ls)),
            _ => {}
        }
    };
    for (fs, gut) in sections {
        let mut cols: Vec<Vec<Frag>> = vec![vec![]; gut.len() + 1];
        for f in &fs {
            let c = gut.iter().filter(|g| (f.x0 + f.x1) / 2.0 > g.1).count();
            cols[c].push(f.clone());
        }
        let cols: Vec<Vec<Frag>> = cols.into_iter().filter(|c| !c.is_empty()).collect();
        // Real columns have text on at least two baselines in every column; otherwise (a justified line with wide word gaps,
        // a short line under it) the section is read row by row.
        let columns = cols.len() >= 2 && cols.iter().all(|c| rows_of(c).len() >= 2);
        let words = median(fs.iter().map(|f| f.text.split_whitespace().count() as f64).collect());
        let rows = rows_of(&fs);
        if columns && cols.len() >= 3 && words <= 4.0 {
            let table = rows
                .iter()
                .map(|r| {
                    let mut cells = vec![String::new(); cols.len()];
                    for f in r {
                        let c = cols.iter().position(|col| col.iter().any(|g| g.x0 == f.x0 && g.base == f.base)).unwrap_or(0);
                        if !cells[c].is_empty() {
                            cells[c].push(' ');
                        }
                        cells[c].push_str(&f.text);
                    }
                    cells
                })
                .collect();
            blocks.push(Block::Table(table));
            continue;
        }
        if columns {
            // each column on its own: a table or an inset inside one column
            for col in cols {
                let mut sub = reading_order(col, body).into_iter();
                match sub.next() {
                    Some(Block::Lines(ls)) => push_lines(&mut blocks, ls, true),
                    Some(t) => blocks.push(t),
                    None => {}
                }
                for b in sub {
                    match b {
                        Block::Lines(ls) => push_lines(&mut blocks, ls, false),
                        t => blocks.push(t),
                    }
                }
            }
            continue;
        }
        let lines = rows
            .into_iter()
            .map(|r| Line {
                x0: r[0].x0,
                x1: r.last().unwrap().x1,
                base: r[0].base,
                size: r.iter().map(|f| f.size).fold(0.0, f64::max),
                bold: r.iter().all(|f| f.bold),
                text: r.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" "),
                col_start: false,
            })
            .collect();
        push_lines(&mut blocks, lines, false);
    }
    blocks
}

/// Fragments on the same baseline, top to bottom, each row left to right.
fn rows_of(fs: &[Frag]) -> Vec<Vec<Frag>> {
    let mut v = fs.to_vec();
    v.sort_by(|a, b| a.base.total_cmp(&b.base));
    let mut rows: Vec<Vec<Frag>> = vec![];
    for f in v {
        match rows.last_mut() {
            Some(r) if (f.base - r[0].base).abs() < 0.3 * f.size.min(r[0].size) => r.push(f),
            _ => rows.push(vec![f]),
        }
    }
    for r in &mut rows {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    rows
}

fn ends_sentence(t: &str) -> bool {
    t.trim_end().ends_with(['.', '!', '?', ':', '"', '”', '»'])
}

fn starts_lower(t: &str) -> bool {
    t.chars().next().is_some_and(char::is_lowercase)
}

fn bullet(t: &str) -> Option<&str> {
    ["• ", "● ", "▪ ", "◦ ", "■ ", "– ", "\u{F0B7} "].iter().find_map(|b| t.strip_prefix(b))
}

fn numbered(t: &str) -> bool {
    let d = t.chars().take_while(char::is_ascii_digit).count();
    (1..=2).contains(&d) && t[d..].starts_with([')', '.']) && t[d + 1..].starts_with(' ')
}

/// Lines in reading order -> Markdown paragraphs, headings and list items.
fn paragraphs(ls: &[Line], body: f64, level: &dyn Fn(f64) -> usize, out: &mut Vec<String>) {
    let pitch = median(ls.windows(2).filter(|w| !w[1].col_start).map(|w| w[1].base - w[0].base).filter(|&d| d > 0.9 * body && d < 2.0 * body).collect());
    let pitch = if pitch > 0.0 { pitch } else { 1.2 * body };
    let col_x = |k: usize| {
        let s = (0..=k).rev().find(|&j| ls[j].col_start).unwrap_or(0);
        let e = (k + 1..ls.len()).find(|&j| ls[j].col_start).unwrap_or(ls.len());
        let (x0, x1) = (ls[s..e].iter().map(|l| l.x0).fold(f64::MAX, f64::min), ls[s..e].iter().map(|l| l.x1).fold(f64::MIN, f64::max));
        (x0, x1)
    };
    let heading = |l: &Line| l.size >= HEADING_SIZE * body && l.text.split_whitespace().count() <= MAX_HEADING_WORDS;
    let mut cur = String::new();
    let mut cur_heading: Option<usize> = None;
    let mut cur_lines = 0;
    let mut cur_bold = true;
    let mut flush = |cur: &mut String, h: Option<usize>, n: usize, bold: bool| {
        let t = cur.trim();
        if !t.is_empty() {
            // A bold one-line paragraph of body size is a heading too, one level below the size-based ones.
            let h = h.or((n == 1 && bold && t.split_whitespace().count() <= 12 && !ends_sentence(t)).then(|| level(body)));
            out.push(match h {
                Some(k) => format!("{} {}", "#".repeat(k), t),
                None if t.starts_with(['#', '>']) => format!("\\{t}"),
                None => t.to_string(),
            });
        }
        cur.clear();
    };
    for (k, l) in ls.iter().enumerate() {
        let lh = heading(l).then(|| level(l.size));
        let brk = k == 0 || {
            let p = &ls[k - 1];
            let (cx0, cx1) = col_x(k);
            lh != cur_heading
                || bullet(&l.text).is_some()
                || numbered(&l.text)
                || if l.col_start {
                    // across a column/section: continue only a sentence that clearly runs on
                    ends_sentence(&p.text) || !(starts_lower(&l.text) || p.text.ends_with('-'))
                } else {
                    l.base - p.base > 1.3 * pitch
                        || (l.size - p.size).abs() > 0.1 * body
                        || l.bold != p.bold && lh.is_none()
                        || l.x0 > cx0 + 0.8 * body && p.x0 <= cx0 + 0.8 * body && ends_sentence(&p.text)
                        || p.x1 < cx1 - 3.0 * body && ends_sentence(&p.text)
                }
        };
        if brk {
            flush(&mut cur, cur_heading, cur_lines, cur_bold);
            (cur_heading, cur_lines, cur_bold) = (lh, 0, true);
        }
        cur_lines += 1;
        cur_bold &= l.bold;
        let text = bullet(&l.text).map_or(l.text.clone(), |t| format!("- {t}"));
        if cur.is_empty() {
            cur = text;
        } else if let Some(stem) = cur.strip_suffix('-').filter(|s| s.chars().last().is_some_and(char::is_alphabetic) && starts_lower(&text)) {
            cur = format!("{stem}{text}");
        } else if let Some(stem) = cur.strip_suffix('\u{AD}') {
            cur = format!("{stem}{text}");
        } else {
            cur.push(' ');
            cur.push_str(&text);
        }
    }
    flush(&mut cur, cur_heading, cur_lines, cur_bold);
}

fn table_md(rows: &[Vec<String>]) -> String {
    let esc = |s: &str| s.replace('|', "\\|");
    let mut t = String::new();
    for (i, r) in rows.iter().enumerate() {
        t.push_str(&format!("| {} |\n", r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")));
        if i == 0 {
            t.push_str(&format!("|{}\n", " --- |".repeat(r.len())));
        }
    }
    t.trim_end().to_string()
}

/// Footnote lines -> one string per note: a line starting with a number or symbol starts a new note.
fn footnotes(fs: &[Frag]) -> Vec<String> {
    let mut notes: Vec<String> = vec![];
    for r in rows_of(fs) {
        let t = r.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" ");
        let starts = t.chars().next().is_some_and(|c| c.is_ascii_digit() || "*†‡§".contains(c));
        match notes.last_mut() {
            Some(n) if !starts => {
                if n.ends_with('-') && starts_lower(&t) {
                    n.pop();
                } else {
                    n.push(' ');
                }
                n.push_str(&t);
            }
            _ => notes.push(t),
        }
    }
    notes
}

/// Same text as ItsWiz's formatPageMarker (plugin/main.js), so notes look alike whether MinerU or PDFable wrote them.
fn page_marker(label: &str, notes: &[String], trailing_rule: bool) -> String {
    let trailer = if trailing_rule { "\n\n---\n\n" } else { "\n" };
    if notes.is_empty() {
        format!("\n\n*— side {label} —*{trailer}")
    } else {
        format!("\n\n---\n{}\n\n*— side {label} —*{trailer}", notes.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One fragment per word run: glyph width 0.5 em, word space 0.3 em.
    fn line(text: &str, x: f64, y: f64, size: f64) -> Vec<Ch> {
        let mut v = vec![];
        let mut cx = x;
        for c in text.chars() {
            if c == ' ' {
                cx += 0.3 * size;
                continue;
            }
            v.push(Ch { s: c.to_string(), x0: cx, x1: cx + 0.5 * size, y, size, bold: false });
            cx += 0.5 * size;
        }
        v
    }

    fn page(chars: Vec<Ch>) -> PageText {
        PageText { h: 800.0, chars, digital: true, ..Default::default() }
    }

    #[test]
    fn two_columns_read_left_then_right() {
        let mut c = line("Title", 50.0, 100.0, 20.0);
        for i in 0..5 {
            let y = 200.0 + 12.0 * i as f64;
            c.extend(line(&format!("left{i} aaa"), 50.0, y, 10.0));
            c.extend(line(&format!("right{i} bbb"), 300.0, y, 10.0));
        }
        let (md, _) = to_markdown(&[page(c)]);
        let l = md.find("left4").unwrap();
        let r = md.find("right0").unwrap();
        assert!(l < r, "{md}");
        assert!(md.starts_with("# Title"), "{md}");
    }

    #[test]
    fn hyphen_joins_and_page_number_labels() {
        let mut pages = vec![];
        for p in 0..3 {
            let mut c = line("Running head", 50.0, 40.0, 9.0);
            c.extend(line("a long sen-", 50.0, 200.0, 10.0));
            c.extend(line("tence ends here.", 50.0, 212.0, 10.0));
            c.extend(line(&format!("{}", p + 11), 280.0, 780.0, 9.0));
            pages.push(page(c));
        }
        let (md, rep) = to_markdown(&pages);
        assert!(md.contains("a long sentence ends here."), "{md}");
        assert!(!md.contains("Running head"), "{md}");
        assert_eq!(rep.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["11", "12", "13"]);
        assert!(md.contains("*— side 11 —*\n\n---\n\n"), "{md}");
        assert!(md.ends_with("*— side 13 —*\n"), "{md}");
    }

    #[test]
    fn labels_follow_offset_and_spreads() {
        let n = |v: &[u32]| v.to_vec();
        assert_eq!(page_labels(&[n(&[]), n(&[4]), n(&[5]), n(&[])]), ["3", "4", "5", "6"]);
        assert_eq!(page_labels(&[n(&[]), n(&[])]), ["1", "2"]);
        assert_eq!(page_labels(&[n(&[2, 3])]), ["2–3"]);
        assert_eq!(page_number("Side 7"), Some(7));
        assert_eq!(page_number("— 12 —"), Some(12));
        assert_eq!(page_number("3 / 24"), Some(3));
        assert_eq!(page_number("2023 Report"), None);
        assert_eq!(page_number("Page 3 of 10"), Some(3));
    }

    #[test]
    fn footnote_goes_before_marker() {
        let mut c = line("Body text with a note.", 50.0, 200.0, 10.0);
        c.extend(line("1 The note itself.", 50.0, 700.0, 7.0));
        let (md, _) = to_markdown(&[page(c)]);
        assert_eq!(md, "Body text with a note.\n\n---\n1 The note itself.\n\n*— side 1 —*\n");
    }

    #[test]
    fn table_of_short_cells() {
        let mut c = line("Intro paragraph that is long enough to be prose.", 50.0, 100.0, 10.0);
        for (i, r) in [["Name", "Age", "Town"], ["Ann", "31", "Aarhus"], ["Bo", "45", "Vejle"]].iter().enumerate() {
            let y = 200.0 + 14.0 * i as f64;
            c.extend(line(r[0], 50.0, y, 10.0));
            c.extend(line(r[1], 200.0, y, 10.0));
            c.extend(line(r[2], 350.0, y, 10.0));
        }
        let (md, rep) = to_markdown(&[page(c)]);
        assert!(md.contains("| Name | Age | Town |\n| --- | --- | --- |\n| Ann | 31 | Aarhus |"), "{md}");
        assert_eq!(rep[0].tables, 1);
    }
}

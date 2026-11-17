#!/usr/bin/env python3
"""#154: dev tool (not shipped). Per page: did PDFable lose text the original had?

  python3 -I tests/loss-eval.py DIR [--flags "..."] [--bin PDFABLE] [--base OLD_PDFABLE] [--min-ratio 0.98]
                                [--top 6 --sheets OUT_DIR] [--only SUBSTRING] [--jobs 8]

For every PDF in DIR: run PDFable (default flags and threshold as in #152), OCR original and output page by page
(Tesseract dan, 300 dpi) and count, per page: words (>= 3 letters), digit tokens, å/ø/æ and whether a page number
(a line of 1-4 digits first or last on the page) is found. A page is flagged when output words < min-ratio * original.
--base runs a second build (BASE_BIN as in verify.sh) so two builds can be compared. --sheets writes original|output
contact sheets (PNG) of the worst --top flagged pages per file, for the eye.

A flag is a count, not a verdict. The "note" column says when it is probably not lost text: "few-words" (original page has
< 30 words, e.g. a cover), "turned" (Tesseract's orientation differs between original and output, e.g. a page that
comes out 180 degrees), "small" (loses <= 12 words). Page count changed (--split): only totals are compared.
No tesseract / pdftoppm / PDFable binary: a message and exit 1, no crash.
"""
import argparse, os, re, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def run(*a):
    return subprocess.run(a, capture_output=True, text=True)

def npages(pdf):
    m = re.search(r"Pages:\s+(\d+)", run("pdfinfo", pdf).stdout)
    return int(m.group(1)) if m else 0

def ocr_page(pdf, p, tmp, tag):
    png = f"{tmp}/{tag}-{p}"
    run("pdftoppm", "-gray", "-png", "-r", "300", "-f", str(p), "-l", str(p), "-singlefile", pdf, png)
    t = run("tesseract", png + ".png", "-", "-l", "dan", "--psm", "3").stdout.lower()
    lines = [l.strip() for l in t.splitlines() if l.strip()]
    pn = any(re.fullmatch(r"\d{1,4}", l) for l in lines[:1] + lines[-1:])
    return dict(words=len(re.findall(r"[a-zæøå]{3,}", t)), nums=len(re.findall(r"\d+", t)),
                aoe=sum(t.count(c) for c in "åøæ"), pn=pn, png=png + ".png")

def orient(png):  # Tesseract OSD: degrees the page must be turned; None when it cannot tell
    m = re.search(r"Rotate:\s+(\d+)", run("tesseract", png, "-", "--psm", "0").stdout)
    return int(m.group(1)) if m else None

def ocr_pdf(pdf, tmp, tag, jobs):
    with ThreadPoolExecutor(jobs) as ex:
        return list(ex.map(lambda p: ocr_page(pdf, p, tmp, tag), range(1, npages(pdf) + 1)))

def sheet(orig_png, out_png, path):
    from PIL import Image
    a, b = Image.open(orig_png).convert("L"), Image.open(out_png).convert("L")
    h = 1100
    a, b = (i.resize((max(1, i.width * h // i.height), h)) for i in (a, b))
    s = Image.new("L", (a.width + b.width + 12, h), 128)
    s.paste(a, (0, 0)); s.paste(b, (a.width + 12, 0)); s.save(path)

def evaluate(pdf, builds, a):
    tmp = tempfile.mkdtemp()
    try:
        o = ocr_pdf(pdf, tmp, "o", a.jobs)
        res = []
        for name, binary in builds:
            out = f"{tmp}/{name}.pdf"
            cmd = [binary, *a.flags.split(), pdf, out] + ([str(a.threshold)] if a.threshold else [])
            run(*cmd)
            if not os.path.exists(out):  # e.g. "Not smaller": nothing written, nothing lost
                res.append((name, None, None)); continue
            res.append((name, out, ocr_pdf(out, tmp, name, a.jobs)))
        return o, res, tmp
    except Exception:
        shutil.rmtree(tmp, ignore_errors=True); raise

def flagged(o, w, ratio):
    if len(o) != len(w): return []
    return [i for i in range(len(o)) if w[i]["words"] < ratio * o[i]["words"]]

def note(o, w, orig_png, out_png):
    if o["words"] < 30: return "few-words"
    if orient(orig_png) != orient(out_png): return "turned"
    if o["words"] - w["words"] <= 12: return "small"
    return ""

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--bin", default=os.path.join(ROOT, "rust/target/release/pdfable"))
    ap.add_argument("--base", default=os.environ.get("BASE_BIN"))
    ap.add_argument("--flags", default="--rotate --crop --split auto --deskew")
    ap.add_argument("--threshold", type=int, default=60)
    ap.add_argument("--min-ratio", type=float, default=0.98)
    ap.add_argument("--top", type=int, default=0)
    ap.add_argument("--sheets", default="")
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    for tool in ("tesseract", "pdftoppm", "pdfinfo"):
        if not shutil.which(tool): sys.exit(f"loss-eval: {tool} not found (brew install tesseract tesseract-lang poppler)")
    if "dan" not in run("tesseract", "--list-langs").stdout.split(): sys.exit("loss-eval: tesseract language 'dan' missing (brew install tesseract-lang)")
    builds = [("new", a.bin)] + ([("base", a.base)] if a.base else [])
    for _, b in builds:
        if not os.access(b, os.X_OK): sys.exit(f"loss-eval: no PDFable binary at {b} (cd rust && cargo build --release)")
    pdfs = sorted(f for f in os.listdir(a.dir) if f.lower().endswith(".pdf") and a.only in f) if os.path.isdir(a.dir) else []
    if not pdfs: sys.exit(f"loss-eval: no PDFs in {a.dir}")
    if a.top and a.sheets:
        os.makedirs(a.sheets, exist_ok=True)
        try: import PIL  # noqa
        except ImportError: sys.exit("loss-eval: --sheets needs Pillow (pip install pillow)")
    print("file\tbuild\tpages\twords_orig\twords_out\tnums_orig\tnums_out\taoe_orig\taoe_out\tpagenum_orig\tpagenum_out\tbytes_orig\tbytes_out\tpages_below")
    all_flags = []
    for f in pdfs:
        pdf = os.path.join(a.dir, f)
        o, res, tmp = evaluate(pdf, builds, a)
        try:
            for name, out, w in res:
                s = lambda L, k: sum(int(p[k]) for p in L)
                if w is None:
                    print(f"{f}\t{name}\t{len(o)}\t{s(o,'words')}\t-\t-\t-\t-\t-\t-\t-\t{os.path.getsize(pdf)}\t- (no output written)\t-"); continue
                bad = flagged(o, w, a.min_ratio)
                print(f"{f}\t{name}\t{len(o)}->{len(w)}\t{s(o,'words')}\t{s(w,'words')}\t{s(o,'nums')}\t{s(w,'nums')}\t{s(o,'aoe')}\t{s(w,'aoe')}\t{s(o,'pn')}\t{s(w,'pn')}\t{os.path.getsize(pdf)}\t{os.path.getsize(out)}\t{len(bad) if len(o)==len(w) else 'n/a (page count changed)'}", flush=True)
                for i in bad:
                    n = note(o[i], w[i], o[i]["png"], w[i]["png"])
                    all_flags.append((f, name, i + 1, o[i]["words"], w[i]["words"], n))
                if name == "new" and a.top and a.sheets:
                    worst = sorted(bad, key=lambda i: w[i]["words"] - o[i]["words"])[:a.top]
                    for i in worst:
                        sheet(o[i]["png"], w[i]["png"], os.path.join(a.sheets, f"{os.path.splitext(f)[0][:30].replace(' ','_')}-p{i+1}.png"))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    print("\nflagged pages (output words < %.0f%% of original):\nfile\tbuild\tpage\twords_orig\twords_out\tnote" % (a.min_ratio * 100))
    for r in all_flags: print("\t".join(map(str, r)))

if __name__ == "__main__":
    main()

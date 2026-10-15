#!/usr/bin/env python3
"""#134: compare PDFable's --text note with MinerU's note for the same PDF.

  python3 -I tests/text-compare.py mineru.md pdfable.md

Prints one TSV row: words in each, share of MinerU's words also in PDFable's (and the other way round), share of the word
order kept (longest matching blocks, difflib), page markers in each and how many of PDFable's markers sit at the same place
in the text (the 40 words before the marker match), and the headings in each. Images, LaTeX, HTML tags and Markdown
syntax are left out of the word lists, so only the reading text is compared.
"""
import difflib, re, sys
from collections import Counter

MARK = re.compile(r"\*— side (.+?) —\*")

def words(md):
    md = re.sub(r"!\[[^\]]*\]\([^)]*\)|!\[\[[^\]]*\]\]", " ", md)       # images
    md = re.sub(r"\$\$.*?\$\$|\$[^$\n]*\$", " ", md, flags=re.S)          # LaTeX
    md = re.sub(r"<[^>]+>", " ", md)                                      # HTML (MinerU's tables)
    md = MARK.sub(" ", md)
    md = re.sub(r"^\s*(#+|-|\*|>|---|\|[-\s|:]*\|)\s*", " ", md, flags=re.M)
    return [w for w in re.findall(r"\w+", md.lower())]

def markers(md):
    """(label, the 40 words before the marker) per page marker."""
    out = []
    for m in MARK.finditer(md):
        out.append((m.group(1), words(md[: m.start()])[-40:]))
    return out

def headings(md):
    return [l.strip() for l in md.splitlines() if re.match(r"#+ ", l)]

def main(a, b):
    ma, mb = open(a, encoding="utf-8").read(), open(b, encoding="utf-8").read()
    wa, wb = words(ma), words(mb)
    ca, cb = Counter(wa), Counter(wb)
    common = sum((ca & cb).values())
    sm = difflib.SequenceMatcher(None, wa, wb, autojunk=False)
    order = sum(bl.size for bl in sm.get_matching_blocks()) / max(len(wa), 1)
    ka, kb = markers(ma), markers(mb)
    # A PDFable marker is in place when its label exists in MinerU's note and the words before it end the same way there.
    by_label = {l: ws for l, ws in ka}
    placed = sum(1 for l, ws in kb if l in by_label and ws[-10:] == by_label[l][-10:])
    labels_same = sum(1 for (la, _), (lb, _) in zip(ka, kb) if la == lb)
    print("\t".join(["words_mineru", "words_pdfable", "mineru_in_pdfable", "pdfable_in_mineru", "order_kept", "markers_mineru", "markers_pdfable", "labels_same", "markers_in_place", "headings_mineru", "headings_pdfable"]))
    print("\t".join(map(str, [len(wa), len(wb), f"{common / max(len(wa), 1):.3f}", f"{common / max(len(wb), 1):.3f}", f"{order:.3f}", len(ka), len(kb), labels_same, placed, len(headings(ma)), len(headings(mb))])))

if __name__ == "__main__":
    main(*sys.argv[1:3])

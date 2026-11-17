# PDFable: working agreements

## Plain-language summary (owner is not a specialist in this code)

Every session that finishes a piece of work (PR opened, issue closed, investigation ended) ends its final report with a block
headed **"Kort fortalt"**, in plain Danish or English, 3-5 lines, no internal jargon (no "band rule", "plan box", "G4", issue
numbers without saying what they are):

1. What was the problem, as the end user would see it?
2. What did we change, in one sentence?
3. What the owner must judge or decide (if anything), and what is just FYI.
4. Is it safe to merge, and what is the sensible next step?

A session that starts work on an issue opens with the same kind of 2-3 lines: what this issue is for, and whether the result
will be visible in the output PDFs at all.

**The summary is added, never a substitute.** Everything else stays as strict as before: verbatim numbers (not paraphrased),
every metric in the issue checked and reported, "if a number differs, stop and report", known limits and anything that got worse
listed explicitly. The block must not hide a failed check or a "needs a look"; if one exists it goes in "Kort fortalt" too.

## Lukke issues via PR

PR-beskrivelsen skal have `Closes #N` (engelsk nøgleord; "Lukker" virker ikke), ét nøgleord pr. issue, og PR skal merges til main. Delvis løsning: `Refs #N`. Se `.github/pull_request_template.md`.

## Visuel kontrol af sider (checker-agenten)

Ja/nej-kontrol af, om en side er afskåret, drejet eller forringet, på side-om-side-ark (original til venstre, resultat til højre): kør `checker` med `model: haiku`. Flager Haiku noget, eller er arket uklart, kør samme ark igen med Sonnet. Haiku 5.5 svarede rigtigt på 8 af 8 ark (17. nov. 2026, grov test, #152). Farve- og pixelmålinger er ikke testet på Haiku: dem kører `checker` på sin normale model. OCR-ordtal alene kan give falske alarmer (Behrendt side 6) og misforstå en vendt side som tabt tekst (side 25); se arket, før et tab antages.

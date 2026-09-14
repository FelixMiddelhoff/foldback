# Docs Style Guide

Conventions for anyone writing or editing pages under `docs/src/`, so contributions read as one voice.

- **Cookbook recipes** assume a stated minimal setup and explicitly skip error handling (`Result` unwraps stand in for real handling) — state this once at the top of the cookbook, not per recipe.
- **Prefer a concrete worked example with real numbers** over abstract description. `velocity.x: 3.14159 vs 3.14158`, not "the values differ." A measured `193.19 µs` beats "it's fast."
- **Every engine integration page follows the same skeleton**: who this is for, install, minimal example, reflective hashing (if applicable), CI integration, troubleshooting. A new binding's docs PR should be reviewable against that checklist, not freeform.
- **No marketing language** — no "blazingly fast," no "revolutionary." This audience trusts specificity and measured claims (real benchmark numbers, once they exist for a given feature) over adjectives.
- **Mark unshipped features honestly.** A page for something not built yet says so plainly near the top ("Not implemented yet — Phase N") rather than describing it as if it exists. Better to have an honest stub than a page someone copies code from that doesn't compile.
- **Docs track `main`.** Write against what's actually shipped in the repo today, not the plan for what will ship — if a planning doc and the real API disagree, the real API wins and the page gets corrected.

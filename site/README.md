# ShieldSpec website

The production-oriented, developer-first landing page is a standalone static site with zero runtime dependencies.

## Local preview

From the repository root on your own machine:

```bash
python3 -m http.server 8765 --directory site
```

Then open `http://127.0.0.1:8765`.

Serve over HTTP rather than opening `index.html` as a file: the interactive evidence explorer fetches `data/cases.json`. Its initial historical case remains visible even when fetch is unavailable.

## Validation

```bash
node --test site/tests/site.test.mjs
node --check site/app.js
```

## Deployment

The `site/` folder may be deployed directly to any static hosting provider. Configure the web/public root to `site/`. No Node build step, account, wallet, secrets, or backend required.

For GitHub Pages using a branch whose pages folder is `/docs`, either publish the `site/` contents as a standalone pages artifact or copy only these assets into the selected output folder as part of the deployment workflow. Do not accidentally publish repository evidence logs as user-facing browser data.

## Evidence integrity

- The interactive explorer reads an explicitly curated JSON representation of the upstream historical Zallet issue [zcash/zallet#677](https://github.com/zcash/zallet/pull/677) and two controls.
- The real deterministic reproduction and raw logs are in `evidence/historical-zallet-677/`. The viewer does **not** run Zallet, SDK fuzzing, mining, wallet state, or transactions in the browser.
- The affected revision `f496c9b6b2c6ebb0065b329c11bda4a5fdf73568` accepted the seeded Ironwood→Sapling FullPrivacy fixture; corrected `e966780216b7a86b0a1a6618f2fde5b574da7171` rejected it. The controls were equivalent across versions.
- This is a previously documented and fixed upstream issue, **not** a newly discovered vulnerability or an automatic historical-policy finding by the current generalized ShieldSpec fuzzer.
- The hero terminal demonstrates an independently injected and clearly labeled regression, not the historical upstream policy failure.
- A passing ShieldSpec campaign never guarantees privacy or anonymity outside its configured invariants and tested scenarios.

## Update discipline

If evidence or labels change, update `site/data/cases.json` only alongside reviewed reproduction logs and corresponding test updates. Do not add vanity adoption claims or imply external users without verified evidence.

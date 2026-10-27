import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (name) => readFileSync(new URL("../" + name, import.meta.url), "utf8");
const html = () => read("index.html");
const css = () => read("styles.css");
const js = () => read("app.js");
const cases = () => JSON.parse(read("data/cases.json"));

test("a real public product page has metadata, main landmark, and working section ids", () => {
  const page = html();
  for (const token of ["<title>ShieldSpec", 'name="description"', 'name="viewport"', "<main", 'id="how-it-works"', 'id="findings"', 'id="quickstart"', 'id="integrations"', 'id="roadmap"']) {
    assert.ok(page.includes(token), `Missing: ${token}`);
  }
  assert.match(page, /Catch privacy regressions before they reach production\./);
});

test("the finding explorer identifies the documented historical behavior without a novel discovery claim", () => {
  const all = cases();
  assert.equal(all.findings.length >= 3, true);
  const historical = all.findings.find((f) => f.id === "zallet-677");
  assert.equal(historical.kind, "historical reproduction");
  assert.equal(historical.before.verdict, "accepted");
  assert.equal(historical.after.verdict, "rejected");
  assert.match(historical.provenance, /zcash\/zallet\/pull\/677/);
  assert.equal(historical.discoveredByShieldSpec, false);
  assert.equal(historical.newVulnerability, false);
});

test("accessible native controls and fail-honest demo copy", () => {
  const page = html();
  assert.match(page, /role="tablist"/);
  assert.match(page, /aria-selected="true"/);
  assert.match(page, /aria-live="polite"/);
  assert.match(page, /Historical reproduction/);
  assert.match(page, /not a newly discovered vulnerability/i);
  assert.match(js(), /textContent/);
  assert.match(js(), /aria-selected/);
});

test("design works responsively and respects reduced motion", () => {
  assert.match(css(), /@media\s*\(max-width:/);
  assert.match(css(), /prefers-reduced-motion/);
  assert.match(css(), /:focus-visible/);
});

test("integration commands and links are usable rather than placeholders", () => {
  const page = html();
  assert.match(page, /cargo run --locked/);
  assert.match(page, /uses: Ay-obami\/ShieldSpec/);
  assert.match(page, /https:\/\/github.com\/Ay-obami\/ShieldSpec/);
  assert.doesNotMatch(page, /href="#"/);
  assert.doesNotMatch(page, /lorem ipsum/i);
});

test("the engine names the shipped cross-pool observation without claiming an upstream policy verdict", () => {
  const page = html();
  assert.match(page, /SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE/);
  assert.match(page, /wallet policy verdict/);
});

test("the sticky navigation is not trapped by an overflow scrolling ancestor", () => {
  assert.doesNotMatch(css(), /\.site-shell\s*\{\s*overflow:\s*hidden\s*\}/);
});

test("GitHub Pages workflow publishes only the reviewed static site after main merges", () => {
  const workflow = readFileSync(new URL("../../.github/workflows/site.yml", import.meta.url), "utf8");
  assert.match(workflow, /push:/);
  assert.match(workflow, /branches:\s*\[main\]/);
  assert.match(workflow, /path:\s*site\/?/);
  assert.match(workflow, /actions\/deploy-pages@/);
  assert.match(workflow, /pages:\s*write/);
  assert.match(workflow, /id-token:\s*write/);
});

test("all local section links resolve to real ids", () => {
  const page = html();
  const ids = new Set([...page.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
  for (const match of page.matchAll(/href="#([^"]+)"/g)) {
    assert.ok(ids.has(match[1]), `Broken internal link: #${match[1]}`);
  }
});

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

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
  assert.match(page, /a wallet acceptance verdict/);
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

test("current SDK Orchard-only observation is distinguished from historical FullPrivacy", () => {
  const page = html();
  assert.match(page, /zcash_spend_orchard_only_v1/);
  assert.match(page, /SpendPolicy::shielded_pools/);
  assert.match(page, /Orchard-only/);
  assert.match(page, /distinct from (?:Zallet's|Zallet’s) FullPrivacy/i);
  assert.match(page, /comparable_wallet_policy_cases/);
});

test("four NU6.3 control cases have explicit funding and activation and a runnable campaign", () => {
  const page = html();
  const rows = [...page.matchAll(/<tr data-case-ordinal="(\d+)">([\s\S]*?)<\/tr>/g)];
  assert.deepEqual(rows.map((match) => Number(match[1])), [0, 1, 2, 3]);
  for (const [index, funding, activation] of [
    [0, "Orchard", "At activation"],
    [1, "Orchard", "Before activation"],
    [2, "Sapling", "At activation"],
    [3, "Sapling", "Before activation"]
  ]) {
    assert.match(rows[index][2], new RegExp(funding));
    assert.match(rows[index][2], new RegExp(activation));
  }
  for (const arg of [
    "--baseline-runner", "--candidate-runner", "--baseline-arg=--wallet-spend-policy",
    "--candidate-arg=--wallet-spend-policy", "--scenario-family ironwood", "--cases 4"
  ]) {
    assert.ok(page.includes(arg), "Missing campaign argument: " + arg);
  }
});

test("copy controls announce success and recover with selectable code when clipboard is denied", async () => {
  const page = html();
  assert.match(page, /id="copy-status"[^>]*role="status"[^>]*aria-live="polite"/);
  assert.match(page, /id="start-command"[^>]*tabindex="0"/);
  for (const willCopy of [true, false]) {
    const events = {};
    const code = { textContent: "  cargo test --locked  ", focused: false, focus() { this.focused = true; } };
    const status = { textContent: "" };
    const button = {
      textContent: "Copy ↗",
      dataset: { copyTarget: "start-command" },
      addEventListener(type, handler) { events[type] = handler; }
    };
    const copied = [];
    const doc = {
      querySelectorAll(selector) {
        return selector === "[data-copy-target]" ? [button] : [];
      },
      getElementById(id) {
        return id === "start-command" ? code : id === "copy-status" ? status : null;
      }
    };
    runInNewContext(js(), {
      document: doc,
      navigator: { clipboard: { async writeText(value) {
        if (!willCopy) throw new Error("permission denied");
        copied.push(value);
      } } },
      setTimeout() {},
      console
    });
    await events.click();
    if (willCopy) {
      assert.deepEqual(copied, ["cargo test --locked"]);
      assert.match(status.textContent, /copied/i);
    } else {
      assert.equal(code.focused, true);
      assert.match(status.textContent, /select|clipboard/i);
    }
  }
});

test("historical tab navigation moves the panel's accessible name with selection", async () => {
  const listeners = [];
  const tabs = ["zallet-677", "orchard-sapling", "ironwood-same-pool"].map((id) => ({
    id: "case-tab-" + id,
    dataset: { caseId: id },
    classList: { toggle() {} },
    setAttribute() {},
    addEventListener(name, listener) {
      if (name === "click") listeners.push(listener);
    }
  }));
  const panel = { attributes: {}, setAttribute(name, value) { this.attributes[name] = value; } };
  const doc = {
    querySelectorAll(selector) { return selector === "[data-case-id]" ? tabs : []; },
    getElementById(id) { return id === "case-panel" ? panel : null; }
  };
  runInNewContext(js(), {
    document: doc,
    fetch: async () => ({ ok: true, json: async () => cases() }),
    navigator: {},
    console
  });
  await new Promise(setImmediate);
  listeners[1]();
  assert.equal(panel.attributes["aria-labelledby"], "case-tab-orchard-sapling");
});

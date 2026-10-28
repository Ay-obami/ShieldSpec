import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const read = (name) => readFileSync(new URL("../" + name, import.meta.url), "utf8");
const html = () => read("index.html");
const css = () => read("styles.css");
const js = () => read("app.js");
const cases = () => JSON.parse(read("data/cases.json"));

test("developer docs link opens a complete on-site, copyable CLI guide", () => {
  assert.match(html(), /href="\.\/docs\.html"/);
  const guide = read("docs.html");
  assert.match(guide, /<main\b/);
  assert.match(guide, /Rust 1\.95\.0/);
  assert.match(guide, /cargo run --locked -p shieldspec-cli/);
  assert.match(guide, /shieldspec-fuzz fuzz/);
  assert.match(guide, /shieldspec-fuzz replay/);
  assert.match(guide, /zcash\/zallet\/pull\/677/);
  assert.match(guide, /actions\/checkout@/);
  assert.match(guide, /Exit codes/);
  assert.match(guide, /expected.*exit.*2|exit.*2.*expected/i);
  assert.match(guide, /<script src="\.\/app\.js\?/);
  assert.match(guide, /href="\.\/styles\.css\?v=clarity-20261028"/);
  const ids = new Set([...guide.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
  const targets = [...guide.matchAll(/data-copy-target="([^"]+)"/g)].map((match) => match[1]);
  assert.ok(targets.length >= 3, "The guide needs multiple directly copyable workflows");
  for (const target of targets) assert.ok(ids.has(target), `Broken docs copy target: ${target}`);
  for (const [, anchor] of guide.matchAll(/href="#([^"]+)"/g)) {
    assert.ok(ids.has(anchor), `Broken docs section link: #${anchor}`);
  }
});

test("a real public product page has metadata, main landmark, and working section ids", () => {
  const page = html();
  for (const token of ["<title>ShieldSpec", 'name="description"', 'name="viewport"', "<main", 'id="how-it-works"', 'id="findings"', 'id="quickstart"', 'id="integrations"', 'id="roadmap"']) {
    assert.ok(page.includes(token), `Missing: ${token}`);
  }
  assert.match(page, /Catch privacy regressions before they ship\./);
});

test("the finding explorer identifies automatically rediscovered historical behavior without a novel vulnerability claim", () => {
  const all = cases();
  assert.equal(all.findings.length >= 3, true);
  const historical = all.findings.find((f) => f.id === "zallet-677");
  assert.equal(historical.kind, "historical reproduction");
  assert.equal(historical.before.verdict, "accepted");
  assert.equal(historical.after.verdict, "rejected");
  assert.match(historical.provenance, /zcash\/zallet\/pull\/677/);
  assert.equal(historical.discoveredByShieldSpec, true);
  assert.equal(historical.newVulnerability, false);
  assert.equal(historical.before.revision, "f496c9b6b2c6ebb0065b329c11bda4a5fdf73568");
  assert.equal(historical.after.revision, "e966780216b7a86b0a1a6618f2fde5b574da7171");
  assert.match(all.meta.proof, /ShieldSpec\/pull\/45/);
  assert.match(all.meta.notice, /automatically/i);
});

test("the redesign uses flat gold and charcoal instead of green surfaces or gradients", () => {
  const styles = css();
  assert.match(styles, /--bg:\s*#f8f9fb/i);
  assert.match(styles, /--accent:\s*#c49336/i);
  assert.match(styles, /--terminal:\s*#15171c/i);
  assert.doesNotMatch(styles, /(?:linear|radial|conic)-gradient\s*\(/i);
  assert.doesNotMatch(styles, /--green|#8de6ab|#abfac4|#080e0d/i);
  assert.match(styles, /prefers-reduced-motion/);
});

test("published site requests versioned CSS and JS to avoid mixing the new markup with cached old assets", () => {
  const page = html();
  assert.match(page, /href="\.\/styles\.css\?v=cohesive-20261028"/);
  assert.match(page, /src="\.\/app\.js\?v=cohesive-20261028"/);
});

test("the single live evidence panel contains the full real Zallet proposal and source-linked verdicts", () => {
  const page = html();
  assert.equal((page.match(/class="case-panel"/g) || []).length, 1);
  assert.match(page, /id="case-proposal"/);
  assert.match(page, /independently compiled historical Zallet revisions/i);
  assert.match(page, /id="before-summary"/);
  assert.match(page, /id="after-summary"/);
  assert.match(page, /id="before-source" href="https:\/\/github\.com\/zcash\/zallet\/commit\/f496c9b6b2c6ebb0065b329c11bda4a5fdf73568"/);
  assert.match(page, /id="after-source" href="https:\/\/github\.com\/zcash\/zallet\/commit\/e966780216b7a86b0a1a6618f2fde5b574da7171"/);
  assert.match(js(), /setText\("before-sha", example\.before\.revision\)/);
  assert.match(js(), /setText\("after-sha", example\.after\.revision\)/);
  assert.match(js(), /setText\("case-proposal"/);
});

test("sparse hero presents real historical policy comparison with a path to proof", () => {
  const page = html();
  const hero = page.split('class="hero section"')[1].split('class="section problem-section"')[0];
  assert.match(hero, /Zallet FullPrivacy/);
  assert.match(hero, /f496c9b/);
  assert.match(hero, /e966780/);
  assert.match(hero, /ACCEPTED/);
  assert.match(hero, /REJECTED/);
  assert.match(hero, /#findings/);
  assert.doesNotMatch(hero, /injected-fault demo|hero-footnote|hero-bottom|hero-glow/);
});

test("first-time visitor can identify the input, comparison, and actionable output from the homepage", () => {
  const page = html();
  const hero = page.split('class="hero section"')[1].split('class="section problem-section"')[0];
  assert.match(hero, /transaction proposals/i);
  assert.match(hero, /baseline/i);
  assert.match(hero, /candidate/i);
  assert.match(hero, /privacy-policy/i);
  assert.match(hero, /replay/i);
  assert.match(page, /What ShieldSpec checks/);
  assert.match(page, /transparent change/i);
  assert.match(page, /cross-pool/i);
  assert.match(page, /receiver/i);
  assert.match(page, /named wallet-policy verdict/i);
  assert.match(page, /runner (?:hashes|provenance)/i);
});

test("important site copy and controls never fall below readable type sizes", () => {
  const styles = css();
  const selectors = [
    ".nav-links a", ".hero-lede", ".terminal-body", ".terminal-row code",
    ".problem-card p", ".workflow-step p", ".detail-note", ".section-description",
    ".case-tab", ".case-panel > p", ".case-meta b", ".compare-panel code",
    ".provenance", ".policy-proof p", ".text-link", ".command-card pre",
    ".command-caption", ".roadmap-list p", ".faq-list p", ".copy-button"
  ];
  for (const selector of selectors) {
    const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const rules = [...styles.matchAll(new RegExp(`(?:^|\\n)[^\\n{}]*${escaped}[^{}]*\\{([^}]*)\\}`, "g"))];
    assert.ok(rules.length, `Missing critical selector: ${selector}`);
    const sizes = rules.flatMap((rule) => [...rule[1].matchAll(/(?:font-size:\s*|font:\s*(?:\d+\s+)?)(\d+)px/g)].map((match) => Number(match[1])));
    assert.ok(sizes.length, `Missing explicit font size: ${selector}`);
    assert.ok(sizes.every((size) => size >= 14), `Too-small critical text in ${selector}: ${sizes.join(", ")}`);
  }
});

test("accessible native controls and fail-honest demo copy", () => {
  const page = html();
  assert.match(page, /role="tablist"/);
  assert.match(page, /aria-selected="true"/);
  assert.match(page, /aria-live="polite"/);
  assert.match(page, /Historical reproduction/);
  assert.match(page, /not a new vulnerability/i);
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

test("the copyable four-case campaign runs from merged main without detached PR checkout", () => {
  const match = html().match(/<pre id="sdk-command"[^>]*><code>([\s\S]*?)<\/code><\/pre>/);
  assert.ok(match, "Missing copyable SDK campaign commands");
  const command = match[1];
  assert.ok(command.includes("cargo build --locked -p shieldspec-zcash-runner --bins"),
    "Main-branch quickstart must build the actual Zcash runner");
  assert.ok(command.includes("--scenario-family ironwood") && command.includes("--cases 4"),
    "Quickstart must exercise the real four-case NU6.3 campaign");
  assert.equal(command.includes("git fetch origin pull/42/head"), false,
    "PR #42 is merged; its detached checkout instructions are obsolete");
  assert.equal(command.includes("git switch --detach FETCH_HEAD"), false,
    "Main users must not be instructed to switch to a detached feature head");
});

test("the NU6.3 runner evidence links to a pinned source revision, not stale main", () => {
  const match = html().match(/href="([^"]+)"[^>]*>Inspect runner tests/);
  assert.ok(match, "Missing test provenance link");
  const url = new URL(match[1]);
  assert.equal(url.host, "github.com");
  assert.match(url.pathname,
    /^\/Ay-obami\/ShieldSpec\/blob\/[0-9a-f]{40}\/crates\/shieldspec-zcash-runner\/tests\/scenario_generation\.rs$/);
});

test("public site documents the published v0.3.0 Action and honest runner boundaries", () => {
  const home = html();
  const guide = read("docs.html");
  assert.ok(home.includes("https://github.com/marketplace/actions/shieldspec-privacy-check"));
  assert.ok(guide.includes("uses: Ay-obami/ShieldSpec@v0.3.0"));
  assert.ok(guide.includes("preset: zcash-sdk"));
  assert.ok(guide.includes("preset: custom"));
  assert.ok(guide.includes("examples/runner-adapter/README.md"));
  assert.ok(guide.includes("Level 3 automatic wallet-runner generation remains deferred"));
});

test("new CLI onboarding is discoverable without implying the built-in control tests a wallet", () => {
  const guide = read("docs.html");
  assert.match(guide, /id="quick-try"/);
  assert.match(guide, /cargo run --locked -p shieldspec-cli -- try/);
  assert.match(guide, /not your wallet or a live differential campaign/);
  assert.match(guide, /docs\/simple-workflows\.md/);
});

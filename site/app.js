(() => {
  "use strict";

  const tabs = [...document.querySelectorAll("[data-case-id]")];
  const field = (id) => document.getElementById(id);
  const setText = (id, value) => {
    const target = field(id);
    if (target) target.textContent = value;
  };

  function setVerdict(id, verdict) {
    const target = field(id);
    if (!target) return;
    const normalized = verdict === "accepted" ? "accepted" : verdict === "rejected" ? "rejected" : "neutral";
    target.className = "verdict is-" + normalized;
    target.textContent = (normalized === "neutral" ? "○" : "●") + " " + verdict[0].toUpperCase() + verdict.slice(1);
  }

  function renderCase(example) {
    setText("case-kind", example.kind);
    setText("case-policy", example.policy + " policy");
    setText("case-title", example.title);
    setText("case-description", example.description);
    setText("case-scenario", example.scenario);
    setText("case-balance", example.publicBalance);
    setText("before-sha", example.before.revision.slice(0, 12));
    setText("after-sha", example.after.revision.slice(0, 12));
    setVerdict("before-verdict", example.before.verdict);
    setVerdict("after-verdict", example.after.verdict);
    setText(
      "case-provenance",
      "This is a " + example.kind + " using previously documented upstream behavior. " +
      (example.discoveredByShieldSpec
        ? "See reproduction evidence and exact runner provenance."
        : "It was not a new vulnerability or automatically discovered by ShieldSpec's general fuzzer.")
    );
    for (const tab of tabs) {
      const selected = tab.dataset.caseId === example.id;
      tab.setAttribute("aria-selected", String(selected));
      tab.classList.toggle("is-active", selected);
      tab.tabIndex = selected ? 0 : -1;
      if (selected && tab.id) field("case-panel")?.setAttribute("aria-labelledby", tab.id);
    }
  }

  async function loadCases() {
    if (!tabs.length) return;
    try {
      const response = await fetch("./data/cases.json", { cache: "no-store" });
      if (!response.ok) throw new Error("HTTP " + response.status);
      const data = await response.json();
      if (!Array.isArray(data.findings)) throw new Error("Invalid findings format");
      const byId = new Map(data.findings.map((item) => [item.id, item]));
      const show = (id) => {
        const value = byId.get(id);
        if (value) renderCase(value);
      };
      for (const tab of tabs) {
        tab.addEventListener("click", () => show(tab.dataset.caseId));
        tab.addEventListener("keydown", (event) => {
          const selected = tabs.indexOf(tab);
          const next = event.key === "ArrowRight" ? (selected + 1) % tabs.length
            : event.key === "ArrowLeft" ? (selected - 1 + tabs.length) % tabs.length
            : event.key === "Home" ? 0
            : event.key === "End" ? tabs.length - 1 : -1;
          if (next < 0) return;
          event.preventDefault();
          tabs[next].focus();
          show(tabs[next].dataset.caseId);
        });
      }
      show(tabs[0].dataset.caseId);
    } catch (error) {
      const note = field("case-provenance");
      if (note) note.textContent = "Verified case data could not be loaded. Serve this page over HTTP, or inspect the repository evidence directly.";
      // Preserve the static case in the HTML as a no-script/fetch-failure fallback.
      console.warn("ShieldSpec historical case data unavailable:", error);
    }
  }

  async function copyCode(button) {
    const target = field(button.dataset.copyTarget);
    if (!target) return;
    const original = button.textContent;
    const value = target.textContent.trim();
    const status = field("copy-status");
    if (status) status.textContent = "";
    try {
      await navigator.clipboard.writeText(value);
      button.textContent = "Copied ✓";
      if (status) status.textContent = "Commands copied to clipboard.";
    } catch (_) {
      button.textContent = "Select code to copy";
      target.focus?.();
      if (status) status.textContent = "Clipboard unavailable. Select the focused code block and copy manually.";
    }
    setTimeout(() => { button.textContent = original; }, 2200);
  }

  for (const button of document.querySelectorAll("[data-copy-target]")) {
    button.addEventListener("click", () => copyCode(button));
  }
  loadCases();
})();

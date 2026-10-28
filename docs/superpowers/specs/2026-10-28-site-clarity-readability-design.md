# ShieldSpec Website Clarity and Readability — Design Revision

## Intent

Retain the approved flat off-white/charcoal/ZEC-gold visual direction, but restore the technical substance lost during the minimalist redesign. A first-time visitor should understand within the initial screen what ShieldSpec does, why privacy regressions matter, and how to inspect real proof; a developer should be able to follow an actual start-to-finish workflow from the website itself.

## Requirements from the user

- Minimal *visual noise*, not minimal information: adequate line length, whitespace between explanations, and clear headings instead of removing substance.
- Make text genuinely readable: normal copy around 16–18px, technical code around 14–15px, strong foreground contrast on light and dark panels, clear visible links and controls.
- Preserve no-gradient constraint, gold accent, slight motion, real Zallet evidence, all existing interactive findings, SDK controls, and copy buttons.
- Provide meaningful developer instructions in an accessible `/docs.html` page; link it prominently from the navigation and relevant homepage blocks.

## Approaches considered

1. Add more words only to the hero. Rejected: it would not address the missing developer instructions or other tiny text.
2. Convert the entire homepage into a long tutorial. Rejected: overwhelming for first-time evaluation and difficult to scan.
3. **Recommended: progressive disclosure.** A substantial but calm homepage explains the problem, concrete checks, process, and verified evidence, while a separately linked developer guide gives commands, expected results, result interpretation, and troubleshooting. Keeps a clean first impression without sacrificing depth.

## Information architecture

- Homepage hero: small meaningful context label, clear claim, full description of baseline/candidate proposal comparison and replayable output, primary **Get started** button to developer guide, secondary **View real evidence** anchor. Real Zallet card remains clearly historical.
- Problem section: explain the user consequence of a valid yet revealing proposal; add concrete check categories (transparent recipient/change, cross-pool value balance, receiver/change routing, actual wallet-policy verdicts) with honest scope descriptions.
- Engine section: expand four steps into understandable input/output descriptions and a compact callout defining a saved differential result (seed, case, hashes, replay).
- Workflow section: describe who uses it (wallet maintainers, SDK reviewers, CI integrators) and where to start.
- Existing evidence explorer and controls: retain, emphasize that historical Zallet #677 is a documented, fixed issue rediscovered automatically, not a new vulnerability.
- New developer guide: prerequisites and safe synthetic execution; snapshot check and diff; real SDK control; independent runner comparison and replay; optional historical Zallet reproduction; CI setup; exit codes and troubleshooting; links to repo source docs.

## Readability and accessibility

- Body 17px desktop, 16px mobile. Primary paragraphs >=16px, commands >=14px, important dark technical panel body >=15px, navigation >=14px. Small eyebrow numbering can be 12px but not carry essential information.
- Muted copy should use near-charcoal on white; dark technical copy should be light gray. WCAG 4.5:1 contrast for normal text is the target.
- Section rhythm around 92–100px desktop and 65–76px mobile; readable 60–75ch text measure; no gradients/glows; no sideways page overflow.
- Keyboard-usable links/tabs/copy buttons, reduced-motion behavior, responsive wide code blocks with horizontal scrolling.

## Claims and boundaries

- Snapshot rules are not equivalent to real wallet policy decisions.
- Identical-runner controls do not prove cross-version compatibility; exit 2 requires inspecting both verdicts.
- Site explorer is a read-only representation; actual fuzzing runs locally/in CI, no transaction signing/broadcast.
- Historical Zallet result is genuinely rediscovered with independently compiled pinned code and reproducible provenance, but was already fixed upstream.

## Acceptance

Homepage renders with comprehensible explanation and legible text; Docs resolves to a real on-site developer guide; every code example matches committed CLI/reproduction instructions; current 18 regressions remain green; fresh browser inspection confirms readability, navigation, evidence controls, and no gradients.

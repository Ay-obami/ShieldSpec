# ShieldSpec site redesign — approved design

## Intent

Replace the crowded, green-tinted developer landing page with a spacious, credible Zcash developer-tool presentation. The user chose a hybrid light/dark aesthetic, requested ZEC-inspired gold accents, forbade **all gradients**, and approved the second, substantially simplified homepage mockup. The narrated demo follows after the website and will be no more than 2 minutes 30 seconds.

## Visual and content contract

- Flat off-white `#F8F9FB` page, white content surfaces, near-black `#17191F` text, charcoal `#15171C` technical panel. Use restrained warm gold `#C49336` and darker accessible gold for light-surface links. Never use green, violet, background glow, or any CSS gradient.
- Clean, mostly light page with a single modest dark technical panel; no tilt, splash, grid overlay, floating decorative elements, or heavy shadows.
- Above the fold: slim navigation, modest three-line statement, concise two-line support, primary gold “Explore the evidence” link, understated GitHub link, and compact actual historical Zallet #677 result. Plenty of blank space. Push feature cards and detailed explanations below the fold.
- Minimal transitions on hover/focus/tabs and restrained progressive enhancement; respect reduced-motion settings. Do not hide essential content pending JavaScript.
- Preserve all existing interactive evidence tabs, copyable commands, source links, SDK evidence, FAQ, keyboard/focus accessibility, and responsive behavior.
- Update historical Zallet content after merged PR #45: general fuzzer automatically found a difference with real pinned affected/corrected Zallet policy runners; controls agree, minimization and verified replay worked. This is a previously disclosed fixed upstream issue, not a new vulnerability. Website visualizations are static views of evidence, not browser-executed fuzzing.

## Approaches considered

- Light-only editorial tool: maximally airy, but undersells technical CLI/evidence.
- Dark-only developer console: authentic, but repeats the old dense appearance.
- **Selected:** hybrid light layout and compact dark evidence surfaces. The hero mockup with large negative space was explicitly approved for implementation.

## Constraints and anti-goals

- Keep standalone static `site/` on existing GitHub Pages, no new dependencies or infrastructure.
- No gradients anywhere. No invented security findings, production transactions, vanity metrics, or fabricated runner results.
- Do not alter the Rust toolchain, historical reproducibility artifact, or demo-video assets as part of the site redesign.
- Test desktop and mobile, keyboard tab behavior, links, copy controls, all site tests, and actual deployed output.

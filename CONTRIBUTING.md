# Contributing

Thanks for helping. This file is the short version of how the project works.

## Before you open a pull request

Run the checks in the README. They are the same ones CI runs. A change that touches Rust types also needs `pnpm bindings`, so `src/lib/bindings.ts` stays in step.

## Rules that are easy to break by accident

**Licence.** ZSortMail is GPL-3.0-or-later. The licence is declared once for the whole repository in `REUSE.toml`, so source files carry no header or comments about it. A new dependency must have a licence compatible with that, and `cargo deny check` has to pass for Rust crates. Do not add GSAP, `@gsap/react` or code copied from React Bits: their licences cannot be combined with a public GPL repository. Rebuild an effect from scratch instead; the ideas are free, the code is not.

**Design.** Colours, type, radii and shadows come from `src/styles/tokens.css`. The Tailwind default palette is switched off on purpose, so `bg-indigo-500` does not exist. If you need a colour that is not a token, add a token, with a reason and a contrast check. Read `/__design` before adding UI.

**Motion.** It explains a change or confirms an action; it is never decoration. Animate `transform` and `opacity` only. No `transition: all`. Every animation must have a lower tier (see `src/motion/governor.ts`) and must never delay input. All motion code lives in `src/motion/`.

**Words.** Plain verbs, sentence case, things named for what they are. Errors say what happened and what happens next. No "Unlock", "Supercharge", "Seamless", "Empower", "Streamline" or "Leverage".

**Privacy.** Nothing leaves the machine except calls to the user's own mail provider. No telemetry, no font CDN, no remote images in mail unless the user allows them. Mail HTML is rendered only in a sandboxed iframe from the `zsm://` protocol. See [SECURITY.md](SECURITY.md).

**Data safety.** The app never permanently deletes server mail. Hiding is local. Destructive actions are undoable.

## Tests

Pure logic gets unit tests next to it (`*.test.ts`, `#[cfg(test)]`). The strand's gap function, the motion governor and the query language are the kind of thing that must stay pinned down. UI changes need a screenshot taken and looked at in light and dark, at a wide and a narrow window.

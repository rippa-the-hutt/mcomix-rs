# `.agents/` — project skillset

Agent-facing documentation for **mcomix-rs**, a Rust/GTK4 comic book reader
(a from-scratch port of the Python MComix3).

| Skill | Use it when |
|---|---|
| [`mcomix-rs-dev`](skills/mcomix-rs-dev/SKILL.md) | building, testing, running or modifying the Rust code |
| [`mcomix-rs-python-parity`](skills/mcomix-rs-python-parity/SKILL.md) | behaviour differs from the Python original, or you port a feature |
| [`mcomix-rs-release`](skills/mcomix-rs-release/SKILL.md) | bumping the version, cutting a release, updating the AUR package |

Ground rules for every change:

1. **The Python implementation is the behavioural reference.** It is no longer
   in the working tree — read it from git history (see the parity skill).
2. **Verify before claiming success:** `cd rust && cargo test` (41 tests) *and* a
   smoke run of the built binary on a real archive.
3. **`cargo test` does not rebuild the binary.** Run `cargo build` before
   smoke-testing, otherwise you are testing a stale binary (this has caused
   false conclusions before).
4. **Keep tests hermetic:** read fixtures only from `test/files/` (they are in
   the repo) and create per-test temp dirs; never depend on a path that exists
   only on the developer's machine (that broke CI once).
5. **User-visible strings go through `crate::i18n::tr(...)` / `trf(...)`.**
6. **Put logic in pure functions with unit tests** (spreads, stepping, layout,
   natsort, parsing, prefs) and keep the GTK code thin.

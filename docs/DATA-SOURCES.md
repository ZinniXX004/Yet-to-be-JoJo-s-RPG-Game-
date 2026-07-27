# External Resources and Data Provenance

This file exists so that every number and every external dependency in the
repository can be traced to where it came from. On a portfolio project, being
able to answer "where did this data come from and are you allowed to use it" is
worth more than the data itself.

---

## 1. Current status: no scraped data, no external datasets

**As of `v0.1.0`, nothing in `data/*.json` is scraped, imported or derived from a
third-party dataset.** Every stat, skill and cost is hand-authored for balance
purposes in this repository.

That is a deliberate choice, not an omission:

| Reason | Detail |
| --- | --- |
| Canon stats do not translate | Published character stat charts use letter grades (A to E) across abstract axes. Turning those into `atk`/`def`/`spd`/`will` integers is a **design decision**, not a data import. Scraping them would produce a false impression of objectivity |
| Legal exposure | Names and abilities are third-party IP. Numbers invented here are ours; scraped tables carry the licence and terms of their source |
| Balance beats fidelity | A skill that is canonically strong but ruins the tempo economy has to be nerfed anyway. Fidelity to a wiki table is not a design goal |
| Reproducibility | A hand-authored file always builds. A scraper breaks when a page changes its markup, and CI turns red for a reason unrelated to the code |

If you disagree and want canon-derived stats, do it under the rules in section 3,
not ad hoc.

---

## 2. Content schema provenance fields

Any entry that is ever derived from an external source must carry its own
provenance. Reserved optional fields on `SkillDef`, `StandDef` and
`CombatantDef`:

```json
{
  "id": "stand.example",
  "name": "Example",
  "provenance": {
    "origin": "hand-authored | derived | imported",
    "source": "human-readable source name",
    "source_url": "https://...",
    "retrieved": "2026-07-27",
    "licence": "e.g. CC BY-SA 4.0",
    "transform": "how the source value became this number"
  }
}
```

Entries without a `provenance` block are, by definition, `hand-authored`. The
validator ignores unknown keys today; when the first derived entry lands, add a
rule that requires `source_url` and `retrieved` whenever `origin != "hand-authored"`.

---

## 3. Scraping policy (must be followed if scraping is ever added)

No scraper exists in this repository yet. If one is added under
`src/data-pipeline/`, it must satisfy all of the following. Anything less does
not get merged.

1. **Check `robots.txt` and the site's Terms of Service first**, and record the
   verdict in a comment at the top of the scraper. "It technically worked" is not
   permission.
2. **Identify yourself** with a descriptive `User-Agent` including a contact
   route. No spoofing a browser.
3. **Rate limit** to at most one request per second, sequential, with retry and
   backoff on 429/5xx. Never parallelise across a single host.
4. **Cache raw responses locally and gitignore the cache.** Re-running analysis
   must not re-hit the network. Never commit scraped HTML: it is someone else's
   copyrighted text.
5. **Commit only derived numbers**, each with the `provenance` block from
   section 2. Facts are not copyrightable; prose and tables as compiled works can
   be.
6. **Attribute the licence.** Wiki content is commonly
   [CC BY-SA](https://creativecommons.org/licenses/by-sa/4.0/), which requires
   attribution and share-alike on derived *content* even when the project code is
   MIT. Record it here and in the entry.
7. **The scraper must be optional.** CI must never depend on a network fetch; a
   third party's outage cannot be allowed to fail your build.
8. **Never scrape media.** No images, audio, fonts or sprites. That is a
   takedown, not a grey area.

Suggested shape if it happens: `src/data-pipeline/import_stats.py`, writing
`data/imported/<source>.json`, with a separate reviewed step that maps letter
grades to integers via an explicit, auditable table checked into this file.

---

## 4. Intellectual property notice

- **Code** in this repository is MIT licensed (see [LICENSE](../LICENSE)).
- **Character and ability names** reference *JoJo's Bizarre Adventure*, created by
  Hirohiko Araki, published by Shueisha. This is an unaffiliated,
  non-commercial fan project. No official assets are included or redistributed.
- The engine is deliberately **IP-agnostic**: `rpg-core` contains no character
  names, so replacing `data/*.json` yields an original game with zero code
  changes. That is both a design property and the mitigation if a takedown ever
  arrives.

---

## 5. External tooling and services actually used

Everything the project depends on that is not a Rust crate. Crates are listed in
[DEVELOPMENT.md](DEVELOPMENT.md#13-rust-dependencies).

| Resource | Role | Licence / terms |
| --- | --- | --- |
| [Godot Engine](https://godotengine.org) | Presentation layer | MIT |
| [godot-rust (gdext)](https://github.com/godot-rust/gdext) | GDExtension bindings | MPL-2.0 |
| [GitHub Actions](https://docs.github.com/actions) | CI/CD | GitHub ToS |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) | Build cache | LGPL-3.0 |
| [EmbarkStudios/cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Supply-chain audit | MIT / Apache-2.0 |
| [crate-ci/typos](https://github.com/crate-ci/typos) | Spellcheck | MIT / Apache-2.0 |
| [lycheeverse/lychee](https://github.com/lycheeverse/lychee) | Link check | Apache-2.0 / MIT |
| [softprops/action-gh-release](https://github.com/softprops/action-gh-release) | Publishes releases | MIT |
| [Shields.io](https://shields.io) | README badges | CC0 |
| [Simple Icons](https://simpleicons.org) | Logos inside badges | CC0 (logo trademarks remain their owners') |

### Algorithms and prior art referenced

| Reference | Used for |
| --- | --- |
| Steele, Lea, Flood, *Fast Splittable Pseudorandom Number Generators*, OOPSLA 2014 ([ACM](https://dl.acm.org/doi/10.1145/2714064.2660195)) | SplitMix64, the RNG in `rpg-core::rng` |
| Vigna's reference implementation ([prng.di.unimi.it](https://prng.di.unimi.it/splitmix64.c)) | Constants used in `next_u64` |
| [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) / [SemVer](https://semver.org/spec/v2.0.0.html) | Release documentation format |
| [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) | Commit message format |

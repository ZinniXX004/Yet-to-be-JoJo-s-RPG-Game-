# README assets

Images used by the [README](../../README.md) and the
[documentation index](../README.md). They are kept in plain Git rather than Git
LFS (see `.gitattributes`): they are small, change rarely, and GitHub shows them
without spending LFS bandwidth. Game assets still go to LFS.

## Artwork

| File | What it is |
| --- | --- |
| `banner-dark.svg`, `banner-light.svg` | The hero banner, shown in whichever theme the reader uses. The tempo bars and the title shine are CSS animations inside the SVG, and they stop when the reader's system asks for reduced motion |
| `icons/*.svg` | Feature icons for the README: determinism, content as data, measured balance, presentation |

All of it is original geometry. The lettering is set in
[Inter](https://rsms.me/inter/) (SIL Open Font License 1.1) and DejaVu Sans Mono
and converted to outlines, because an SVG shown on GitHub cannot load a font.
Nothing here reproduces artwork, logos or characters from *JoJo's Bizarre
Adventure*.

To change a word in a banner, edit both variants the same way in a vector
editor, and keep the text as outlines.

## Battle captures

| File | Source |
| --- | --- |
| `battle-dio-boss.png` | Frame 330 of `matchup.dio_boss` at seed 7 |
| `battle-victory.png` | The last frame (379) of `matchup.dancer_rush` at seed 1 |
| `battle-clip.webp` | Frames 290 to 409 of `matchup.dio_boss` at seed 7, scaled to 960 pixels wide, 10 frames per second, looping |

Each is Godot 4.7.1 Movie Maker output of a battle on autoplay, captured on
2026-11-03 from `development` at `7af7f52` on Linux with the compatibility
renderer, then placed in a window frame whose title names the encounter and the
seed. The window frame is the only addition; the screen inside it is untouched.

The command, from the repository root after building the bridge and syncing
`data/`, is the one in
[DEVELOPMENT.md section 3.3](../DEVELOPMENT.md#33-recording-a-playthrough) with
a `.png` movie path, for example in `cmd.exe`:

```bat
"C:\Tools\Godot\Godot_v4.7.1-stable_win64_console.exe" --path src\game --write-movie "%CD%\recordings\dio7\f.png" --fixed-fps 10 -- --encounter=matchup.dio_boss --seed=7 --autoplay --quit-on-end
```

The battle is a pure function of its seed, so the same command replays the same
fight. Retake the captures when the battle screen changes; a balance change
alone moves the numbers in the log but not what the screen is made of.

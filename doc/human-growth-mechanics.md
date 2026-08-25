# Human growth mechanics

Research checked on 2026-08-25.

## Confirmed model

The community wiki describes human growth as requiring power, fresh water, food, and memories. A growth must reach the general physical minimums of 20 kg weight, 30 cm height, and 10 years life expectancy. Food and memories contribute physical and mental traits, and the wiki currently lists 40 profession predictions.

The application treats every profession value in `data/Humans.csv` as a minimum threshold. A profession is achievable only when the available inventory can meet every non-empty threshold. Empty CSV cells are interpreted as zero, not as an exact target.

Sources:

- [Humans — The Last Caretaker Wiki](https://thelastcaretaker.wiki.gg/wiki/Humans)
- [The Last Caretaker on Steam](https://store.steampowered.com/app/1783560/The_Last_Caretaker/)

## Important uncertainty: overlapping professions

One recipe can clear the thresholds of more than one profession. The public game documentation does not define a dependable tie-break rule. The independent Root-DE calculator reaches the same practical conclusion and attempts to reduce collateral profession matches.

This application therefore does not claim that a feasible recipe guarantees the selected profession. It displays all matched professions and warns when the result is ambiguous. The current optimization objective minimizes resource excess or item count; profession-specific collision avoidance remains a possible future objective.

Reference reviewed without copying code:

- [Root-DE human growth calculator](https://root-de.github.io/LastCaretaker/)
- [Root-DE/LastCaretaker repository](https://github.com/Root-DE/LastCaretaker) — CC BY-NC-ND 4.0

## Current calculation semantics

- Availability is limited to imported backpack items, configured player chests, and any local planning adjustments.
- Food and memory sections are optimized independently and combined.
- `Minimum waste` ranks recipes by summed stat excess, then by item count.
- `Minimum items` ranks recipes by item count, then by summed stat excess.
- If no feasible recipe exists, the best partial combination and its remaining deficits are shown.
- The calculator never consumes items in the `.sav`.

The game is in active development and community data can change. Updated CSV files can be imported through the interface without rebuilding the application.


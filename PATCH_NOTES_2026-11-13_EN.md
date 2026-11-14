# Erdtree Tower 2.1.1

This update adds quality-based builds for spells, incantations and talismans, adjusts enemy affixes and pools, and introduces journey suspension and lobby password checks. These notes compare against the 2.1.0 bundle and include the intervening launcher improvements.

## Spell and incantation quality

- New journeys can obtain Common, Rare, Epic and Legendary variants with applicable affixes. The catalog covers 198 spells and incantations and 3,124 effective variants; unsuitable or ineffective combinations are excluded.
- Random quality is separate from a spell's original legendary status. A legendary spell can still receive Common or Rare affix quality.

| Affix | Effect | Common / Rare / Epic / Legendary |
| --- | --- | --- |
| Potent | Damage parameters | +10% / 18% / 28% / 40% |
| Efficient | FP cost | -10% / 18% / 28% / 40% |
| Lightweight | Stamina cost | -15% / 25% / 35% / 50% |
| Staggering | Poise damage | +15% / 25% / 40% / 60% |
| Charged | Charged damage parameters of supported spells | +15% / 25% / 40% / 55% |
| Lasting | Supported effect duration | +20% / 35% / 50% / 75% |
| Merciful | Instant healing | +15% / 25% / 40% / 60% |
| Corrosive | Status buildup inflicted on targets | +10% / 18% / 28% / 40% |

- Integer rounding affects costs and some values. These are parameter adjustments, not a guarantee of identical final damage scaling against every enemy.
- Reward cards and native item descriptions retain the original spell description. Quality and affix remain in the name, without filling the description with rule or replacement notices.
- Replacing a variant of the same spell preserves its memory-slot position. The new variant must be confirmed in the usable inventory before the old one is removed, preventing a storage-only delivery from deleting an equipped spell.
- English names and descriptions are included, with Comet Azur compatibility retained.

## Talisman quality and additional passives

- 154 original talismans receive four quality variants each, for 616 variants.
- Original effects, weight and same-family equip restrictions remain. Quality adds passives rather than replacing the talisman's original effect.
- Common and Rare variants have one extra passive, Epic has two, and Legendary has three. Bonuses cover HP, FP, stamina, recovery, attacks, damage reduction, skills, critical hits, jump attacks, guard counters and more.
- Extra passives apply only while the talisman is equipped. New journeys use the new pool; existing journeys and older talismans remain supported, without converting every item already owned.

## Shared weapon and spirit reinforcement progression

- Weapon and spirit-ash upgrades now share a progression baseline. Improving either side can advance the other side's upgrade baseline.
- You still select and upgrade individual items. This does not automatically upgrade the entire inventory.
- Existing conversions remain: spirit ash +10, somber weapon +10 and standard weapon +25 represent the same stage. Spirit ashes can still progress to +39.

## Embers and enemy affixes

- New universal Epic ember, **Frost Shackles**: when an enemy suffers Frostbite, action speed is reduced by 30% and damage dealt by 20% for 10 seconds. A new Frostbite proc refreshes the duration without stacking the multipliers.
- Twin Spiritcalling Bell is temporarily retired from rewards and debug grants. Loading older saves removes the retired held effect and handles pending offers.
- Regenerating enemies begin healing only after three seconds without taking damage. Further damage restarts the delay.
- Leeching healing per hit falls from 1.5% to 1% of maximum HP, or from 2.5% to 1.5% with empowered affixes.
- Golden Order now creates a five-second golden shield every 25 seconds, reducing damage taken by 60%. Normal and empowered versions use this shield cycle.
- A separate **Swift** suffering law is worth 20 points. Without it, Swift is excluded from random rolls. With it, bosses always receive Swift; elite encounters have a 25% chance when elite affixes are enabled.
- Related descriptions, icons and law-code compatibility are updated.

## Enemy pools and events

- Act I ordinary pools gain Those Who Live in Death and small Living Jars.
- Act II restores Marionette Soldiers and adds Warhawks to ordinary pools, plus Flame Chariots to elites. Map placements and multiplayer entity data are included.
- Royal Revenants and Cleanrot Knights are excluded from the Act I boss pool. Royal Revenants remain in Act III elites; this does not remove Cleanrot Knights from later acts.
- In the solo Patches gambling event, Patches can now be attacked and killed. Damage makes him hostile and able to retaliate. Hostile or dead Patches refuses further bets, while the exit portal remains usable. This change does not add multiplayer support to that gambling event.

## Save and suspend — experimental support

- F6 and the Save & Suspend button provide an option separate from ending a journey.
- Solo suspension saves progress and returns to Roundtable Hold. Suspending during combat restarts the same unfinished room on resumption; partial enemy state is not restored and spent runes or flask uses are not refunded.
- The host can request party suspension. Everyone must finish the room and rewards, return to the route map, and confirm successful saving before the party commits suspension.
- Guest departure saves progress and obtains host acknowledgement first. Follow the prompt to use Seamless Co-op's Separating Mist. The party waits for the actual disconnect before entering another room.
- At Roundtable Hold, quit normally through the game menu. The feature does not kill the process or bypass Seamless disconnection.
- Resuming together requires matching saved seed, act, route and laws. Catching up with a party that has already progressed further is not supported.
- Snapshots are character-bound, written completely before replacement, and retain a valid backup. Travel, revival, purchases and reward delivery can delay or block suspension.
- Automated logic tests have passed, but a complete live multiplayer validation is still pending. This is not a guarantee against progress loss after force-closing, power failure or every crash.

## Launcher and multiplayer lobby

- Launcher 1.6.3 limits newly created rooms to 2–6 players across the interface and validation paths.
- The list gains numbered pages with 20 rooms per page. Grouped queries and deduplication retrieve more rooms than a single capped response.
- Steam may still truncate saturated groups: Loaded is not the total number of online rooms. Incomplete results are identified, and exact room-name search remains available.
- Private-room passwords are checked before launch. Wrong passwords no longer launch the game; missing verification metadata asks the host to update and recreate the room.
- The lobby password differs from the in-game Session password. The launcher derives a matching connection password for both sides. Enter the password chosen by the host when creating the launcher room, not the in-game Session password.
- Refresh/query handling is improved, and Enter no longer activates a hidden room's join button on another page.

## Other fixes and update guidance

- Corrected reward-card weapon categories for multiple curved swords, greatswords, katanas, thrusting weapons, greataxes, spears and shields using game parameters.
- Removed the repeated footer on ember cards; actual class restrictions remain.
- Death retries gain a safe-hub fallback for continuous falling while awaiting revival, reducing void-loop risk. This does not establish that every platforming or multiplayer revival problem is fixed.
- Update the entire party to the same complete bundle. Do not mix an older DLL with newer maps, parameters or text. Start a new journey to experience the new spell and talisman quality pools.
- Client crashes around reward screens, Steam lobby-search timeouts and multiplayer disconnections remain under investigation and are not claimed as fixed. High memory usage alone is not proof of a disconnection's cause.

Thank you for reporting issues. Please include the time, host/client role, room and log from the same session. A matching crash dump is especially helpful for crashes.

"""Audited download exclusions. Keep all source files and unknown runtime assets."""

EMBEDDED = frozenset({
    'affix-catalog.txt', 'affix-details.tsv', 'affix-rarities.tsv',
    'affix-v2-catalog.txt', 'affix-v2-details.tsv', 'affix-v2-rarities.tsv',
    'protected-ash-pools.tsv', 'random-ash-details.tsv', 'random-ash-weapon-rows.txt',
    'reward-rarities.tsv', 'talisman-definitions.tsv', 'talisman-details.tsv', 'talisman-rarities.tsv',
})

BUILD_ONLY = frozenset({
    'heretical-rise-start.json', 'vanilla-talisman-quality.tsv',
    'custom-spirit-definitions.tsv',
    'affix-effect-remap.tsv', 'random-ashes.tsv', 'talisman-catalog.txt',
    'retired-spell-strip.json', 'reward-art/extra-icons.txt',
    'reward-art/rare-blue-generation.txt', 'reward-art/rarity-generation.txt',
})

EXCLUSIONS = {**dict.fromkeys(EMBEDDED, 'embedded in the DLL via include_str!'),
              **dict.fromkeys(BUILD_ONLY, 'scene-builder input/output or art-generation notes')}


def include_scene_asset(relative):
    return str(relative).replace('\\', '/') not in EXCLUSIONS

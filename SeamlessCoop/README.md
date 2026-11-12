# Seamless Co-op 2.0.3

Runtime by Yui: https://www.nexusmods.com/eldenring/mods/510
Official release mirror: https://github.com/yuiamoroll/EldenRingSeamlessCoopRelease

No open-source license is claimed for this component. Local build inputs
(`ersc.dll`, `locale/`, `crashpad/`) are ignored by Git. `distribution/bundle_seamless.py`
requires DLL SHA-256 `2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c`.
The tower's resurrection and spirit adapters pass offline checks; this does not
constitute a completed two-player resurrection test.
The upstream author's terms apply; this repository's license does not relicense
Seamless Co-op. Refer to the official Nexus Mods page for its terms.

`default.ini` contains no player password. The first launch creates private
`config/SeamlessCoop/ersc_settings.ini`, preserving an existing package runtime's
settings when present. Neither private configuration nor crash reports belong in releases.

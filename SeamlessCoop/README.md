# Seamless Co-op 2.0.2

Runtime by Yui: https://www.nexusmods.com/eldenring/mods/510
Official release mirror: https://github.com/yuiamoroll/EldenRingSeamlessCoopRelease

No open-source license is claimed for this component. Local build inputs
(`ersc.dll`, `locale/`, `crashpad/`) are ignored by Git. `distribution/bundle_seamless.py`
requires DLL SHA-256 `0a093d8490967bf19c62f895fdf1fda21840282d2b2b371b9b4fc24a0cfe146e`.
The upstream author's terms apply; this repository's license does not relicense
Seamless Co-op. Refer to the official Nexus Mods page for its terms.

`default.ini` contains no player password. The first launch creates private
`config/SeamlessCoop/ersc_settings.ini`, preserving an existing package runtime's
settings when present. Neither private configuration nor crash reports belong in releases.

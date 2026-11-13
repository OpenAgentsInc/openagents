# Horizons fixtures (book chapter 39)

Retrieved 2026-11-05 from the JPL Horizons API (`https://ssd.jpl.nasa.gov/api/horizons.api`)
with `format=text`, `EPHEM_TYPE=VECTORS`, `REF_PLANE=ECLIPTIC`, `REF_SYSTEM=ICRF`,
`OUT_UNITS=KM-S`, `VEC_TABLE=2`, `CSV_FORMAT=YES`. Every target is DE441 and every
time is TDB. `horizons.coax` holds each response's `$$SOE`..`$$EOE` block as a
string; the SHA-256 is over that block's record lines, each ending in a newline.

| File | Constant | COMMAND | CENTER | VEC_CORR | START | STOP | STEP | Block SHA-256 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `horizons-emb.txt` | `HZ_EMB` | `3` | `500@10` | `NONE` | 2000-01-01 | 2200-01-02 | `1826 d` | `f996d09c3ef85a3673ea443a37053ae38090d2406d1ba654cac61032ec04e9d0` |
| `horizons-mars.txt` | `HZ_MARS` | `499` | `500@10` | `NONE` | 2000-01-01 | 2200-01-02 | `1826 d` | `b7b114c7480160f4328e58ecefc72c2111db368df282cd057e245c9561dcede4` |
| `horizons-jupiter.txt` | `HZ_JUPITER` | `599` | `500@10` | `NONE` | 2000-01-01 | 2200-01-02 | `1826 d` | `3c238cc71066f1808181920d0b562ed50d4f15bbad270fbac2233d2325c120b6` |
| `horizons-moon.txt` | `HZ_MOON` | `301` | `500@399` | `NONE` | 2000-01-01 | 2000-01-29 | `1 d` | `acc214e7c530a34a2b5a20504da9fbf17cf4d38a4b3fea02410b1de6555c473f` |
| `horizons-mars-ssb.txt` | `HZ_MARS_SSB` | `499` | `500@0` | `NONE` | 2025-01-01 | 2025-01-02 | `1 d` | `37187ee747d4e3e1fe13a9f4e7b6fd7f701ede4d5a5a7993ae1b9e65b52b67e1` |
| `horizons-earth-ssb.txt` | `HZ_EARTH_SSB` | `399` | `500@0` | `NONE` | 2025-01-01 | 2025-01-02 | `1 d` | `2f8f3ddb542066cad7507ad7c0eba0aafd520a8ed0d984f94ee73b7ca040ff40` |
| `horizons-mars-geo.txt` | `HZ_MARS_GEO` | `499` | `500@399` | `NONE` | 2025-01-01 | 2025-01-02 | `1 d` | `c0aed49e37052ce49bb56603a7f0574ff93b5ce483fcdcf66d3ea53cc2287cd7` |
| `horizons-mars-geo-lt.txt` | `HZ_MARS_GEO_LT` | `499` | `500@399` | `LT` | 2025-01-01 | 2025-01-02 | `1 d` | `b1574bd3e1f369a8990dc0b199fbb4ad6d31c155c2581b22ec8cfc322075a1f8` |
| `horizons-mars-geo-lts.txt` | `HZ_MARS_GEO_LTS` | `499` | `500@399` | `LT+S` | 2025-01-01 | 2025-01-02 | `1 d` | `83ea4303efe17baa924efe09cbaa310014e1691ab0725002c052adbc78932d61` |

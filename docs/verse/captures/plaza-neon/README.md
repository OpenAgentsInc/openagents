# Plaza neon stage captures

Offline renders of the plaza at 1280 × 800, reduced to 960 × 600. The
before images use `VERSE_PLAZA_LEGACY=1`, the flat amber path; the after
images use the neon stage. Regenerate with:

```sh
verse --offline --size 1280x800 --capture out.png
verse --offline --size 1280x800 --pitch 8 --orbit 30 --capture out.png
```

| File | View |
| --- | --- |
| `before-spawn.jpg`, `after-spawn.jpg` | The spawn view toward the pylon |
| `before-low.jpg`, `after-low.jpg` | A low camera across the Gym and the computer |

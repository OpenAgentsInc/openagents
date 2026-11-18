# Sea charts

A world on real water needs to know where it is: how deep the water is, how far
the nearest land is, which legal line it is inside, and whether land shows on the
horizon. `metaverse-engine/world/chart.coax` answers those from a small piece of
real data. It draws nothing. A page turns its segments into its own strokes.

SLAB's world is the first user: `metaverse/slab-where.coax` holds its seven
regions and `apps/openagents-web/slab-chart.coax` draws them.

## The data

A chart is a coastline and two grids of heights, cut once from public sources and
committed as Coax constants (`metaverse/slab-chart-data.coax` is the first):

- **Coast.** `s16` pairs: thousandths of a degree east and north of the far
  grid's south-west corner. A line's first point has 16,000 added to its east
  value.
- **Heights.** A far grid over the whole region and a near grid round the place
  the chart is about. One printable character a sample, rows from the south, each
  row from the west. `33 + code`: codes 0 to 63 are depths of `2 × 1.14^code − 2`
  metres, codes 64 to 93 are land of `4 × 1.27^(code − 64)` metres. A height is
  stored to about 7%.
- **Window.** Twelve numbers: the far grid's south, west, north, east, rows and
  columns, then the near grid's.

SLAB's regions come from Natural Earth 1:10m coastlines and NOAA ETOPO1 heights,
both public domain. Nothing is fetched when the world runs: no map tiles, no
outside service.

Positions are degrees north and east. Distances are nautical miles on a flat
chart at the place's latitude, good to about half a percent across a few hundred
miles. A chart must not cross the 180th meridian. Chart accuracy only: not for
navigation.

## What you can ask

| You want | Call |
| --- | --- |
| A chart from its data | `chart_of(coast, far, near, window)` |
| The height at a point (land positive, seabed negative, `CHART_OFF` off the chart) | `chart_height(c, lat, lon)` |
| The water's depth (0 on land, −1 off the chart) | `chart_depth(c, lat, lon)` |
| Miles between two points, the bearing, a point a distance along a bearing | `chart_nmi`, `chart_bearing`, `chart_move` |
| The coast's points, to draw it | `chart_coast_points(c)`, `chart_coast_point(c, i)` (`first` starts a new line) |
| The nearest coast to a point and how far it is | `chart_shore(c, lat, lon)` |
| A line a set distance off the coast (a legal line) | `chart_field_shore` then `chart_march` at that many miles, then `chart_keep_sea` |
| A depth contour | `chart_field_heights` then `chart_march` at minus the depth |
| The chart on a rectangle of the screen, and the point under a click | `ChartView`, `chart_to_screen`, `chart_from_screen`, `chart_view_holds`, `chart_clip` |
| Whether land shows along a bearing from a deck, and how high it stands | `chart_skyline(c, lat, lon, eye, bearing, clear, reach)` |
| How far the first coast is along a bearing | `chart_ray_coast` |

A field is a grid of `f32` the caller keeps (`ChartField` says where its corners
are). `chart_march` appends segments as `lat, lon, lat, lon` and never writes
past the buffer or the count you give it.

`chart_skyline` uses both the heights and the coastline, so an island too small
for the height grid still shows. Land nearer than the sea horizon is seen under
the horizon line, down to its waterline (`base`).

## Drawing a route on a chart

The construction timelapse's route off Texas uses the same chart. Load the
region, make a view, and draw your own points through `chart_to_screen`:

```
#load "../../metaverse/slab-where.coax";

ws: [1] Where; where_load(ws[..], SITE_TEXAS);
c := ws[0].chart;
v := ChartView.{x = 0, y = 0, w = 800, h = 600, lat = 27.7, lon = -96.8, miles = 90};
a := chart_to_screen(v, WHERE_YARD[0], WHERE_YARD[1]);         // the casting yard at Ingleside
b := chart_to_screen(v, WHERE_JETTIES[0], WHERE_JETTIES[1]);   // the channel's mouth
site := where_site(SITE_TEXAS);
e := chart_to_screen(v, site.lat, site.lon);                   // the berth
```

`WHERE_BROWNSVILLE` is the freight port. `where_place(c, site, lat, lon)` gives
the depth, the miles to land and the legal zone at any point on the way.

## Adding a region

Cut the coastline and the two height grids for the new place in the format
above, add its constants to a data file, and give it a line in the loader
(`where_load` for SLAB). Say where the data is from and how exact it is in the
data file's header.

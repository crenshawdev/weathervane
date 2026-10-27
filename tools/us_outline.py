# /// script
# requires-python = ">=3.10"
# dependencies = ["pyshp>=2.3", "shapely>=2.0"]
# ///
"""Generates src/us_outline.txt, the US outline detect_region tests a point against.

Sources, both public domain:
  TIGER/Line 2023 state file (US Census Bureau): the 50 states, DC and the
  five territories, with the Great Lakes water out to the international
  boundary and 3 nautical miles of coastal water. Its land border is the
  IBC / IBWC line (checked against the State Department's LSIB v11.4: median
  offset 1 m, 99th percentile 202 m). Not subject to copyright as a work of
  the US Government (17 U.S.C. 105; TIGER/Line Shapefiles Technical
  Documentation 2023, ch. 1 s. 1.2). Source: U.S. Census Bureau. TIGER/Line
  is a registered trademark of the U.S. Census Bureau.
  Natural Earth 1:10m admin-0 countries (lakes version), 5.1.1: foreign land
  used only as a mask, so its 1-2 km border error does not matter.

Construction: the TIGER polygon as is, plus a 0.3 degree (about 33 km) pad
on every water boundary, pulled back 0.3 degree from any foreign land. The
pad keeps coastal places inside after simplification; the pull-back keeps
every Canadian, Mexican, Russian and British Virgin Islands shore outside.
Canadian lake water more than 33 km from its own shore stays inside the pad,
since the lakes version of Natural Earth holds no lake water to pull back
from; only open water is affected. The result is simplified with
Douglas-Peucker at 0.005 degree, about 500 m.

Output: one ring per line, "lat,lon lat,lon ...", the format point_in_polygon
already parses. Hawaii keeps its hand-written box in geo.rs, so rings in its
bounding box are dropped here; so are uninhabited islets, by the KEEP list.

Run:  uv run tools/us_outline.py  (or: pip install pyshp shapely; python3 tools/us_outline.py)
"""
import io, os, sys, urllib.request, zipfile
import shapefile
from shapely.geometry import shape, Point, box
from shapely.ops import unary_union

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, "cache")
OUT = os.path.join(HERE, "..", "src", "us_outline.txt")
TIGER = "https://www2.census.gov/geo/tiger/TIGER2023/STATE/tl_2023_us_state.zip"
NE = "https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_0_countries_lakes.zip"
PAD = 0.3
TOLERANCE = 0.005
# One inhabited place per ring to keep; rings containing none are dropped.
KEEP = [
    ("lower 48", 39.0, -98.0),
    ("Alaska mainland", 61.22, -149.90),
    ("Adak (Andreanof Islands)", 51.87, -176.64),
    ("Attu (Near Islands)", 52.83, 173.18),
    ("Gambell (St Lawrence Island)", 63.78, -171.74),
    ("St Paul (Pribilof Islands)", 57.12, -170.28),
    ("Little Diomede", 65.75, -168.93),
    ("San Juan (Puerto Rico)", 18.38, -66.05),
    ("Hagåtña (Guam)", 13.47, 144.75),
    ("Pago Pago (Tutuila)", -14.28, -170.70),
    ("Ta'u (Manu'a)", -14.22, -169.50),
]
HAWAII_BOX = box(-179.0, 18.0, -154.0, 29.5)  # geo.rs keeps its own Hawaii box


def fetch(url):
    os.makedirs(CACHE, exist_ok=True)
    path = os.path.join(CACHE, url.rsplit("/", 1)[1])
    if not os.path.exists(path):
        print("downloading", url, file=sys.stderr)
        urllib.request.urlretrieve(url, path)
    z = zipfile.ZipFile(path)
    name = next(n for n in z.namelist() if n.endswith(".shp"))[:-4]
    return shapefile.Reader(shp=io.BytesIO(z.read(name + ".shp")), dbf=io.BytesIO(z.read(name + ".dbf")))


def main():
    tiger = unary_union([shape(s.__geo_interface__) for s in fetch(TIGER).iterShapes()])
    coarse = tiger.simplify(0.01, preserve_topology=True)
    near = coarse.buffer(PAD + 0.1)
    foreign = unary_union([
        shape(r.shape.__geo_interface__)
        for r in fetch(NE).iterShapeRecords()
        if r.record["SOV_A3"] != "US1" and shape(r.shape.__geo_interface__).intersects(near)
    ])
    pad = coarse.buffer(PAD, quad_segs=2).simplify(0.1, preserve_topology=True)
    pad = pad.difference(foreign.buffer(PAD))
    outline = unary_union([tiger, pad]).intersection(box(-180, -90, 180, 90))

    rings = []
    for ring in outline.geoms:
        if ring.intersects(HAWAII_BOX) and ring.centroid.x < -150:
            continue
        keep = [n for n, lat, lon in KEEP if ring.contains(Point(lon, lat))]
        if not keep:
            print(f"dropped ring at {ring.centroid.y:.2f},{ring.centroid.x:.2f} ({ring.area:.3f} deg^2, {len(ring.exterior.coords)} vertices)", file=sys.stderr)
            continue
        simplified = ring.simplify(TOLERANCE, preserve_topology=True)
        assert not simplified.interiors, keep
        rings.append((keep[0], simplified))
    rings.sort(key=lambda r: -r[1].area)

    with open(OUT, "w") as f:
        for name, ring in rings:
            coords = list(ring.exterior.coords)[:-1]  # the parser closes the ring itself
            f.write(" ".join(f"{lat:.4f},{lon:.4f}" for lon, lat in coords) + "\n")
            print(f"{len(coords):5} vertices  {name}", file=sys.stderr)
    print(f"{sum(len(r.exterior.coords) - 1 for _, r in rings)} vertices in {len(rings)} rings -> {os.path.relpath(OUT)}", file=sys.stderr)


if __name__ == "__main__":
    main()

"""Builds assets/fonts/GhoreSerif.ttf — the game's display serif.

Inspired by Athoric / Various Fantasy Game Serifs (chiseled broad-nib strokes, flared wedge serifs)
and by the chameleon: round letters are pointed "slit-pupil" lenses, periods are slit pupils, the Q tail
is a curled prehensile tail, and the apex/terminals are cut like scales.

Lowercase is small caps (the usual fantasy-serif convention).

    pip install fonttools shapely
    python tools/font/make_font.py [out.ttf]
"""
import math
import sys
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from shapely import affinity
from shapely.geometry import MultiPolygon, Polygon, box
from shapely.geometry.polygon import orient
from shapely.ops import unary_union

# ---- metrics -------------------------------------------------------------------------------------
UPM = 1000
CAP = 700
SMALL = 520          # small-cap height
B, T = 30, CAP - 30  # skeleton bottom / top (nib half-height inset)
SB = 110             # left side bearing of the first stem centre
SERIF = 74           # serif half-width

NIB = affinity.rotate(box(-46, -13, 46, 13), 30, origin=(0, 0))  # chisel pen: thick verticals, thin horizontals
NIB_PTS = list(NIB.exterior.coords)[:-1]


# ---- geometry helpers ----------------------------------------------------------------------------
def arc(cx, cy, rx, ry, a0, a1, step=7):
    n = max(2, int(abs(a1 - a0) / step))
    return [(cx + rx * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             cy + ry * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def bez(p0, p1, p2, n=14):
    return [((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t * t * p2[0],
             (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t * t * p2[1]) for t in (i / n for i in range(n + 1))]


def lens_side(cx, cy, w, h, side, p=1.75, n=24):
    """One side of a pointed oval (slit-pupil lens), top to bottom; side=+1 right, -1 left."""
    pts = []
    for i in range(n + 1):
        u = 1 - 2 * i / n
        pts.append((cx + side * (w / 2) * (1 - abs(u) ** p), cy + u * h / 2))
    return pts


def lens(cx, cy, w, h):
    return lens_side(cx, cy, w, h, 1) + lens_side(cx, cy, w, h, -1)[::-1][1:]


def bowl(x0, xc, rx, y0, y1):
    cy, ry = (y0 + y1) / 2, (y1 - y0) / 2
    return [(x0, y1), (xc, y1)] + arc(xc, cy, rx, ry, 90, -90) + [(x0, y0)]


def curl(start, direction_deg, length, turn_deg, r0, r1=None):
    """Tail that runs straight then spirals tighter (the chameleon tail)."""
    pts = [start]
    x, y = start
    a = math.radians(direction_deg)
    for _ in range(int(length / 14)):
        x += 14 * math.cos(a)
        y += 14 * math.sin(a)
        pts.append((x, y))
    steps = int(abs(turn_deg) / 8)
    r = r0
    sign = 1 if turn_deg > 0 else -1
    for _ in range(steps):
        a += sign * math.radians(8)
        r = max(r * 0.975, r1 or 20)
        x += 8 * math.radians(1) * 0 + (r * math.radians(8)) * math.cos(a)
        y += (r * math.radians(8)) * math.sin(a)
        pts.append((x, y))
    return pts


def stroke(pts):
    """Sweep the chisel nib along a polyline."""
    parts = []
    if len(pts) == 1:
        pts = pts * 2
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        cloud = [(x + dx, y + dy) for (x, y) in ((x0, y0), (x1, y1)) for dx, dy in NIB_PTS]
        parts.append(Polygon(cloud).convex_hull)
    return unary_union(parts)


def foot(x, s=SERIF):
    return Polygon([(x - s, 0), (x + s, 0), (x + s - 8, 22), (x + 30, 62), (x - 30, 62), (x - s + 14, 22)])


def head(x, top, s=SERIF):
    """Flared flag serif on the upper left of a stem (angled cut, like a scale)."""
    return Polygon([(x - s, top - 52), (x - s + 24, top), (x + 44, top), (x + 44, top - 90), (x - 8, top - 70)])


def pupil(cx, y0, h, w=88):
    return Polygon(lens(cx, y0 + h / 2, w, h))


# ---- glyph skeletons -----------------------------------------------------------------------------
# Each: dict(w=stem-to-stem width, s=[polylines], feet=[x], heads=[x], extra=[Polygon in skeleton space])
M = 350


def G(w, s, feet=(), heads=(), extra=(), sb=SB):
    return dict(w=w, s=s, feet=list(feet), heads=list(heads), extra=list(extra), sb=sb)


def tick(x, y, dy):
    return [(x, y), (x, y + dy)]


def C_path(cx, w, tip_top=T, tip_bot=B, arm=0.42):
    h = tip_top - tip_bot
    left = lens_side(cx, (tip_top + tip_bot) / 2, w, h, -1)
    top = [(cx + w * arm, tip_top - 62), (cx + w * 0.12, tip_top - 8)]
    bot = [(cx + w * 0.12, tip_bot + 8), (cx + w * arm, tip_bot + 62)]
    return top + left + bot


def spiral_S():
    up = arc(210, 511, 182, 161, 24, 272)
    lo = arc(210, 189, 198, 161, 90, -158)
    return up + lo[1:]


def six():
    loop = lens(220, 238, 400, 416)
    loop = [p for p in loop] + [loop[0]]
    stem = bez((395, T - 8), (60, T + 20), (20, 300)) + [(20, 238)]
    return [stem, loop]


def eight():
    return [lens(200, 520, 300, 300) + [lens(200, 520, 300, 300)[0]],
            lens(200, 190, 380, 320) + [lens(200, 190, 380, 320)[0]]]


def rot180(g, cx, cy):
    out = dict(g)
    out["s"] = [[(2 * cx - x, 2 * cy - y) for x, y in pl] for pl in g["s"]]
    return out


def closed(pts):
    return pts + [pts[0]]


GLYPHS = {
    "A": G(600, [[(0, B), (300, T + 6)], [(600, B), (300, T + 6)], [(135, 220), (465, 220)]], feet=[0, 600], sb=80),
    "B": G(470, [[(0, B), (0, T)], bowl(0, 235, 190, 345, T), bowl(0, 255, 205, B, 345)], feet=[0], heads=[0]),
    "C": G(560, [C_path(290, 540)], sb=100),
    "D": G(520, [[(0, B), (0, T)], bowl(0, 190, 330, B, T)], feet=[0], heads=[0]),
    "E": G(420, [[(0, B), (0, T)], [(0, T), (420, T), (420, 565)], [(0, M), (330, M)], [(0, B), (430, B), (430, 150)]], heads=[0]),
    "F": G(400, [[(0, B), (0, T)], [(0, T), (400, T), (400, 565)], [(0, M), (300, M)]], feet=[0], heads=[0]),
    "G": G(580, [C_path(290, 560) + [], [(290, B), (470, B + 30), (555, 130), (555, 300), (320, 300)]], sb=100),
    "H": G(560, [[(0, B), (0, T)], [(560, B), (560, T)], [(0, M), (560, M)]], feet=[0, 560], heads=[0, 560]),
    "I": G(0, [[(0, B), (0, T)]], feet=[0], heads=[0]),
    "J": G(300, [[(300, T), (300, 190)] + arc(170, 190, 130, 160, 0, -165)], heads=[300], sb=SB),
    "K": G(520, [[(0, B), (0, T)], [(520, T), (60, 300)], [(215, 395), (545, B)]], feet=[0, 545], heads=[0, 520], sb=90),
    "L": G(400, [[(0, B), (0, T)], [(0, B), (420, B), (420, 190)]], heads=[0]),
    "M": G(700, [[(0, B), (0, T), (350, 150), (700, T), (700, B)]], feet=[0, 700], heads=[0]),
    "N": G(580, [[(0, B), (0, T), (580, B), (580, T)]], feet=[0], heads=[0, 580]),
    "O": G(560, [closed(lens(280, 350, 560, T - B))], sb=100),
    "P": G(460, [[(0, B), (0, T)], bowl(0, 225, 205, 290, T)], feet=[0], heads=[0]),
    "Q": G(560, [closed(lens(280, 350, 560, T - B)),
                 curl((330, 190), -55, 190, 250, 60, 22)], sb=100),
    "R": G(500, [[(0, B), (0, T)], bowl(0, 225, 210, 310, T), [(215, 320), (540, B)]], feet=[0, 540], heads=[0]),
    "S": G(420, [spiral_S()], sb=100),
    "T": G(560, [[(0, T - 110), (0, T), (560, T), (560, T - 110)], [(280, T), (280, B)]], feet=[280], sb=80),
    "U": G(540, [[(0, T), (0, 240)] + arc(270, 240, 270, 210, 180, 360) + [(540, T)]], heads=[0, 540]),
    "V": G(600, [[(0, T), (300, B), (600, T)]], heads=[0, 600], sb=80),
    "W": G(860, [[(0, T), (215, B), (430, 430), (645, B), (860, T)]], heads=[0, 860], sb=80),
    "X": G(560, [[(0, T), (560, B)], [(560, T), (0, B)]], feet=[0, 560], heads=[0, 560], sb=90),
    "Y": G(560, [[(0, T), (280, 350), (560, T)], [(280, 350), (280, B)]], feet=[280], heads=[0, 560], sb=80),
    "Z": G(480, [[(0, T - 110), (0, T), (480, T), (0, B), (490, B), (490, B + 110)]], sb=100),
    # digits
    "0": G(420, [closed(lens(210, 350, 420, T - B))], sb=100),
    "1": G(0, [[(0, B), (0, T)], [(-150, 520), (0, T)]], feet=[0], sb=240),
    "2": G(420, [arc(210, 500, 190, 168, 165, -40) + [(0, B), (430, B), (430, 150)]], sb=100),
    "3": G(420, [arc(205, 520, 165, 150, 155, -90) + arc(200, 195, 205, 165, 90, -155)[:]], sb=100),
    "4": G(480, [[(390, T), (0, 200), (500, 200)], [(390, T), (390, B)]], feet=[390], sb=90),
    "5": G(420, [[(420, T), (55, T), (45, 365)] + arc(205, 225, 200, 195, 128, -160)], sb=100),
    "6": G(420, six(), sb=100),
    "7": G(440, [[(0, T - 110), (0, T), (440, T), (110, B)]], sb=100),
    "8": G(380, eight(), sb=110),
    # punctuation (extra polygons live in glyph space directly: see PUNCT)
}
GLYPHS["9"] = rot180(GLYPHS["6"], 210, 350)
GLYPHS["9"]["w"] = 420
GLYPHS["9"]["sb"] = 100


def u_(a):  # arrow shaft helper
    return a


# punctuation glyphs: (advance, polygons-in-final-coords)
def P(adv, *polys):
    return dict(adv=adv, polys=list(polys))


def sk(poly_pts, dx=0):
    return stroke([(x + dx, y) for x, y in poly_pts])


PUNCT = {
    " ": P(300),
    ".": P(260, pupil(130, 0, 150)),
    ",": P(260, pupil(130, 20, 130), stroke([(130, 10), (110, -70), (60, -120)])),
    ":": P(260, pupil(130, 0, 150), pupil(130, 330, 150)),
    ";": P(260, pupil(130, 330, 150), pupil(130, 20, 130), stroke([(130, 10), (110, -70), (60, -120)])),
    "!": P(260, Polygon([(70, 700), (190, 700), (160, 250), (100, 250)]), pupil(130, 0, 150)),
    "?": P(440, stroke(arc(220, 520, 170, 150, 160, -60) + [(230, 340), (200, 270)]), pupil(205, 0, 150)),
    "'": P(240, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)])),
    '"': P(340, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)]),
           Polygon([(215, 700), (275, 700), (255, 480), (235, 480)])),
    "-": P(380, stroke([(80, 270), (300, 270)])),
    "–": P(560, stroke([(80, 270), (480, 270)])),
    "—": P(860, stroke([(80, 270), (780, 270)])),
    "_": P(520, stroke([(40, -110), (480, -110)])),
    "/": P(420, stroke([(70, -60), (350, 700)])),
    "|": P(240, stroke([(120, -150), (120, 760)])),
    "(": P(330, stroke(arc(330, 300, 200, 480, 135, 225))),
    ")": P(330, stroke(arc(0, 300, 200, 480, 45, -45))),
    "[": P(320, stroke([(250, 740), (110, 740), (110, -120), (250, -120)])),
    "]": P(320, stroke([(70, 740), (210, 740), (210, -120), (70, -120)])),
    "+": P(520, stroke([(90, 350), (430, 350)]), stroke([(260, 180), (260, 520)])),
    "=": P(520, stroke([(90, 250), (430, 250)]), stroke([(90, 450), (430, 450)])),
    "*": P(400, stroke([(200, 330), (200, 640)]), stroke([(60, 410), (340, 560)]), stroke([(60, 560), (340, 410)])),
    "%": P(780, stroke(arc(180, 530, 100, 120, 0, 360)), stroke(arc(600, 170, 100, 120, 0, 360)), stroke([(640, 700), (140, 0)])),
    "#": P(560, stroke([(190, 60), (240, 640)]), stroke([(340, 60), (390, 640)]), stroke([(90, 230), (500, 230)]), stroke([(90, 470), (500, 470)])),
    "&": P(640, stroke(closed(lens(230, 545, 250, 270))), stroke(closed(lens(255, 195, 420, 350))),
           stroke([(560, 20), (130, 440)]), stroke([(380, 330), (590, 330)])),
    "@": P(700, stroke(closed(lens(350, 300, 560, 640))), stroke(arc(350, 300, 110, 130, 0, 360))),
    "<": P(480, stroke([(380, 540), (90, 350), (380, 160)])),
    ">": P(480, stroke([(100, 540), (390, 350), (100, 160)])),
    "←": P(780, stroke([(80, 340), (700, 340)]), stroke([(300, 560), (80, 340), (300, 120)])),
    "→": P(780, stroke([(80, 340), (700, 340)]), stroke([(480, 560), (700, 340), (480, 120)])),
    "↑": P(560, stroke([(280, 20), (280, 650)]), stroke([(80, 440), (280, 650), (480, 440)])),
    "↓": P(560, stroke([(280, 20), (280, 650)]), stroke([(80, 230), (280, 20), (480, 230)])),
    "◀": P(640, Polygon([(500, 640), (500, 60), (120, 350)])),
    "▶": P(640, Polygon([(140, 640), (140, 60), (520, 350)])),
    "×": P(520, stroke([(110, 520), (410, 180)]), stroke([(410, 520), (110, 180)])),
    "−": P(520, stroke([(90, 350), (430, 350)])),
    "°": P(340, stroke(closed(lens(170, 570, 150, 200)))),
    "‘": P(240, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)])),
    "’": P(240, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)])),
    "“": P(340, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)]),
           Polygon([(215, 700), (275, 700), (255, 480), (235, 480)])),
    "”": P(340, Polygon([(95, 700), (155, 700), (135, 480), (115, 480)]),
           Polygon([(215, 700), (275, 700), (255, 480), (235, 480)])),
    "•": P(340, pupil(170, 230, 200, 110)),
    "…": P(780, pupil(130, 0, 150), pupil(390, 0, 150), pupil(650, 0, 150)),
}

# accents: (base letter, mark) -> composed over the glyph; marks are drawn above the glyph top
MARKS = {
    "grave": lambda cx, top: stroke([(cx - 70, top + 140), (cx + 50, top + 40)]),
    "acute": lambda cx, top: stroke([(cx + 70, top + 140), (cx - 50, top + 40)]),
    "circ": lambda cx, top: stroke([(cx - 100, top + 50), (cx, top + 150), (cx + 100, top + 50)]),
    "uml": lambda cx, top: unary_union([pupil(cx - 80, top + 30, 110, 78), pupil(cx + 80, top + 30, 110, 78)]),
}
ACCENTED = {
    "À": ("A", "grave"), "È": ("E", "grave"), "Ì": ("I", "grave"), "Ò": ("O", "grave"), "Ù": ("U", "grave"),
    "Á": ("A", "acute"), "É": ("E", "acute"), "Í": ("I", "acute"), "Ó": ("O", "acute"), "Ú": ("U", "acute"),
    "Â": ("A", "circ"), "Ê": ("E", "circ"), "Î": ("I", "circ"), "Ô": ("O", "circ"), "Û": ("U", "circ"),
    "Ä": ("A", "uml"), "Ë": ("E", "uml"), "Ï": ("I", "uml"), "Ö": ("O", "uml"), "Ü": ("U", "uml"),
}


# ---- glyph construction --------------------------------------------------------------------------
def build_letter(g, top=CAP, xscale=1.0):
    """Returns (shape, advance). `top` = glyph height (cap or small cap)."""
    ys = (top - 2 * 30) / (CAP - 60)

    def tx(x):
        return SB_of(g) + x * xscale

    def ty(y):
        return 30 + (y - 30) * ys if y >= 30 else y * ys

    parts = []
    for pl in g["s"]:
        parts.append(stroke([(tx(x), ty(y)) for x, y in pl]))
    for x in g["feet"]:
        parts.append(foot(tx(x), SERIF * (0.9 if xscale < 1 else 1)))
    for x in g["heads"]:
        parts.append(head(tx(x), top, SERIF * (0.9 if xscale < 1 else 1)))
    shape = unary_union(parts)
    adv = int(round(g["w"] * xscale + 2 * SB_of(g)))
    return shape, adv


def SB_of(g):
    return g["sb"]


def finish(shape):
    if isinstance(shape, MultiPolygon):
        geoms = list(shape.geoms)
    else:
        geoms = [shape]
    return [orient(p.simplify(1.2), sign=-1.0) for p in geoms if not p.is_empty]


def draw(polys, name):
    pen = TTGlyphPen(None)
    for poly in polys:
        for ring in [poly.exterior, *poly.interiors]:
            pts = [(int(round(x)), int(round(y))) for x, y in list(ring.coords)[:-1]]
            dedup = [p for i, p in enumerate(pts) if p != pts[i - 1]]
            if len(dedup) < 3:
                continue
            pen.moveTo(dedup[0])
            for p in dedup[1:]:
                pen.lineTo(p)
            pen.closePath()
    return pen.glyph()


def main(out):
    glyphs = {".notdef": (build_notdef(), 600)}
    cmap = {}
    adv_of = {}

    def add(ch, shape, adv):
        name = "uni%04X" % ord(ch)
        glyphs[name] = (shape, adv)
        cmap[ord(ch)] = name

    for ch, g in GLYPHS.items():
        shape, adv = build_letter(g)
        add(ch, shape, adv)
        if ch.isalpha():
            sshape, sadv = build_letter(g, top=SMALL, xscale=0.88)
            add(ch.lower(), sshape, sadv)
    # accents
    for ch, (base, mark) in ACCENTED.items():
        for case, top, xs in ((ch, CAP, 1.0), (ch.lower(), SMALL, 0.88)):
            g = GLYPHS[base]
            shape, adv = build_letter(g, top=top, xscale=xs)
            cx = SB_of(g) + g["w"] * xs / 2
            shape = unary_union([shape, MARKS[mark](cx, top)])
            add(case, shape, adv)
    for ch, p in PUNCT.items():
        shape = unary_union(p["polys"]) if p["polys"] else Polygon()
        add(ch, shape, p["adv"])

    order = list(glyphs)
    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap(cmap)
    glyf = {}
    metrics = {}
    for name, (shape, adv) in glyphs.items():
        polys = finish(shape) if not shape.is_empty else []
        glyf[name] = draw(polys, name)
        xmin = min((p.bounds[0] for p in polys), default=0)
        metrics[name] = (adv, int(xmin))
    fb.setupGlyf(glyf)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=820, descent=-230)
    fb.setupNameTable({"familyName": "Ghore Serif", "styleName": "Regular",
                       "uniqueFontIdentifier": "GhoreSerif-Regular", "fullName": "Ghore Serif Regular",
                       "psName": "GhoreSerif-Regular", "version": "Version 1.0"})
    fb.setupOS2(sTypoAscender=820, sTypoDescender=-230, sTypoLineGap=60, usWinAscent=900, usWinDescent=300,
                sCapHeight=CAP, sxHeight=SMALL, achVendID="GHOR")
    fb.setupPost()
    fb.save(out)
    print("wrote", out, len(order), "glyphs")


def build_notdef():
    return unary_union([box(60, 0, 540, 700).difference(box(120, 60, 480, 640))])


if __name__ == "__main__":
    dest = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[2] / "assets" / "fonts" / "GhoreSerif.ttf"
    main(str(dest))

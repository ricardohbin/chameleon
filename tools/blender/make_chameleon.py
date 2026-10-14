"""Builds a cute low-detail chameleon (platformer hero) in Blender.

Run:  blender -b -P make_chameleon.py
Outputs chameleon.blend, chameleon.glb and preview.png next to this script.
Character faces -Y (Blender front view), tail curls up at +Y, feet on z=0.
"""
import math
import os

import bpy

OUT = os.path.dirname(os.path.abspath(__file__))

bpy.ops.wm.read_factory_settings(use_empty=True)


def mat(name, rgb, rough=0.55, emit=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1)
    b.inputs["Roughness"].default_value = rough
    return m


GREEN = mat("Skin", (0.30, 0.75, 0.22))
DGREEN = mat("SkinDark", (0.20, 0.58, 0.18))
BELLY = mat("Belly", (0.90, 0.95, 0.55))
ORANGE = mat("Crest", (1.0, 0.50, 0.10))
PINK = mat("Cheek", (1.0, 0.45, 0.50))
BLACK = mat("Pupil", (0.01, 0.01, 0.02), rough=0.1)
WHITE = mat("Highlight", (1, 1, 1), rough=0.2)
SPOT = mat("Spot", (0.98, 0.85, 0.2))

parts = []


def sphere(name, loc, scale, material, seg=32, rings=16, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=seg, ring_count=rings, radius=1, location=loc, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.scale = scale
    bpy.ops.object.transform_apply(scale=True, rotation=True)
    bpy.ops.object.shade_smooth()
    o.data.materials.append(material)
    parts.append(o)
    return o


# --- body & head -----------------------------------------------------------
sphere("Body", (0, 0.05, 0.62), (0.62, 0.85, 0.56), GREEN)
sphere("Belly", (0, -0.12, 0.45), (0.5, 0.72, 0.45), BELLY)
HEAD = (0, -0.82, 0.98)
HR = 0.66
sphere("Head", HEAD, (HR, HR, HR * 0.92), GREEN)

# cheeks
for s in (-1, 1):
    sphere(f"Cheek{s}", (s * 0.40, -1.36, 0.78), (0.13, 0.05, 0.09), PINK, 16, 8, rot=(0, 0, s * 0.3))

# eyes: big turret eyes, characteristic of a chameleon
for s in (-1, 1):
    ex, ey, ez = s * 0.42, -1.0, 1.42
    sphere(f"EyeTurret{s}", (ex, ey, ez), (0.30, 0.30, 0.30), DGREEN)
    sphere(f"EyeWhite{s}", (ex + s * 0.03, ey - 0.13, ez + 0.01), (0.25, 0.22, 0.25), WHITE)
    sphere(f"Pupil{s}", (ex + s * 0.02, ey - 0.27, ez - 0.01), (0.15, 0.09, 0.17), BLACK)
    sphere(f"Shine{s}", (ex + s * 0.0 + 0.05, ey - 0.33, ez + 0.07), (0.045, 0.03, 0.045), WHITE, 12, 8)

# smile (tiny beads along an arc on the face)
for i in range(11):
    t = (i / 10) * 2 - 1  # -1..1
    x = t * 0.26
    z = HEAD[2] - 0.20 + 0.16 * t * t
    y = HEAD[1] - math.sqrt(max(HR ** 2 - x ** 2 - (z - HEAD[2]) ** 2, 0)) * 1.0 - 0.005
    sphere(f"Smile{i}", (x, y, z), (0.022, 0.02, 0.022), BLACK, 8, 6)

# nostrils
for s in (-1, 1):
    sphere(f"Nostril{s}", (s * 0.09, -1.46, 1.02), (0.02, 0.015, 0.02), DGREEN, 8, 6)

# --- crest on head/back ----------------------------------------------------
sphere("Casque", (0, -0.80, 1.60), (0.14, 0.34, 0.20), ORANGE)
for i in range(6):
    y = -0.45 + i * 0.22
    z = 1.22 - 0.02 * i
    r = 0.15 - i * 0.012
    sphere(f"Crest{i}", (0, y, z + 0.0), (r * 0.7, r * 1.1, r), ORANGE, 16, 8)

# spots on the back
for i, (x, y, z, r) in enumerate([(-0.28, 0.0, 1.12, 0.1), (0.3, 0.25, 1.05, 0.12),
                                  (-0.22, 0.42, 0.98, 0.09), (0.12, -0.2, 1.17, 0.08)]):
    sphere(f"Spot{i}", (x, y, z), (r, r, r * 0.35), SPOT, 16, 8)

# --- legs & feet -----------------------------------------------------------
for sx in (-1, 1):
    for fy, name in ((-0.42, "F"), (0.5, "B")):
        sphere(f"Leg{name}{sx}", (sx * 0.52, fy, 0.38), (0.19, 0.22, 0.30), GREEN, 20, 10, rot=(0, sx * 0.25, 0))
        sphere(f"Foot{name}{sx}", (sx * 0.58, fy - 0.07, 0.12), (0.22, 0.30, 0.12), DGREEN, 20, 10)
        for t in (-1, 0, 1):  # little toes
            sphere(f"Toe{name}{sx}{t}", (sx * 0.58 + t * 0.09, fy - 0.33, 0.1), (0.07, 0.07, 0.07), DGREEN, 12, 8)

# --- curly tail (chain of shrinking spheres) -----------------------------
cy, cz = 1.50, 1.00
N = 34
for i in range(N):
    s = i / (N - 1)
    a = 1.25 * math.pi + s * 2.35 * math.pi
    r = 0.80 * (1 - 0.72 * s)
    y = cy + r * math.cos(a)
    z = cz + r * math.sin(a)
    rad = 0.24 * (1 - s) + 0.05
    col = ORANGE if (i // 3) % 2 and i > 6 else GREEN
    sphere(f"Tail{i}", (0, y, z), (rad * 0.9, rad, rad), col, 20, 10)

# --- join into one mesh ----------------------------------------------------
bpy.ops.object.select_all(action="DESELECT")
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
ch = bpy.context.active_object
ch.name = "Chameleon"
bpy.ops.object.origin_set(type="ORIGIN_CURSOR")  # origin at world 0 (between feet)
bpy.ops.object.shade_smooth()

# --- export ---------------------------------------------------------------
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "chameleon.blend"))
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "chameleon.glb"), export_format="GLB", use_selection=False)

# --- preview scene -------------------------------------------------------------
w = bpy.data.worlds.new("W")
bpy.context.scene.world = w
w.use_nodes = True
w.node_tree.nodes["Background"].inputs[0].default_value = (0.75, 0.9, 1.0, 1)
w.node_tree.nodes["Background"].inputs[1].default_value = 1.0

bpy.ops.mesh.primitive_plane_add(size=20, location=(0, 0, 0))
bpy.context.active_object.data.materials.append(mat("Ground", (0.55, 0.8, 0.45), 0.9))

bpy.ops.object.light_add(type="SUN", location=(3, -4, 6), rotation=(math.radians(50), 0, math.radians(35)))
bpy.context.active_object.data.energy = 2.5

bpy.ops.object.camera_add(location=(4.3, -5.0, 2.6))
cam = bpy.context.active_object
d = ch.location - cam.location
tgt = (0, 0.3, 0.95)
from mathutils import Vector
cam.rotation_euler = (Vector(tgt) - cam.location).to_track_quat("-Z", "Y").to_euler()
cam.data.lens = 55
bpy.context.scene.camera = cam

sc = bpy.context.scene
sc.render.engine = "BLENDER_EEVEE"
sc.view_settings.view_transform = "Standard"
sc.render.resolution_x, sc.render.resolution_y = 1000, 800
sc.render.filepath = os.path.join(OUT, "preview.png")
bpy.ops.render.render(write_still=True)
print("DONE")

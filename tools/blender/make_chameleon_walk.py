"""Cute chameleon + armature + looping walk cycle.

Run:  blender -b -P make_chameleon_walk.py
Outputs chameleon_rigged.blend, chameleon_rigged.glb (with a "Walk" clip, frames 1-25,
frame 25 == frame 1 so it loops), walk.mp4 and walk_frames.png (side-view check frames).
Character faces -Y. Walk is in place (drive movement from your game code).
"""
import math
import os

import bpy
from mathutils import Vector

OUT = os.path.dirname(os.path.abspath(__file__))
bpy.ops.wm.read_factory_settings(use_empty=True)


def mat(name, rgb, rough=0.55):
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
BONE = "body"  # bone that the next spheres are skinned to


def sphere(name, loc, scale, material, seg=32, rings=16, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=seg, ring_count=rings, radius=1, location=loc, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.scale = scale
    bpy.ops.object.transform_apply(scale=True, rotation=True)
    bpy.ops.object.shade_smooth()
    o.data.materials.append(material)
    vg = o.vertex_groups.new(name=BONE)
    vg.add(list(range(len(o.data.vertices))), 1.0, "REPLACE")
    parts.append(o)
    return o


# --- body ----------------------------------------------------------------------
BONE = "body"
sphere("Body", (0, 0.05, 0.62), (0.62, 0.85, 0.56), GREEN)
sphere("Belly", (0, -0.12, 0.45), (0.5, 0.72, 0.45), BELLY)
for i in range(6):
    y = -0.45 + i * 0.22
    r = 0.15 - i * 0.012
    sphere(f"Crest{i}", (0, y, 1.22 - 0.02 * i), (r * 0.7, r * 1.1, r), ORANGE, 16, 8)
for i, (x, y, z, r) in enumerate([(-0.28, 0.0, 1.12, 0.1), (0.3, 0.25, 1.05, 0.12),
                                  (-0.22, 0.42, 0.98, 0.09), (0.12, -0.2, 1.17, 0.08)]):
    sphere(f"Spot{i}", (x, y, z), (r, r, r * 0.35), SPOT, 16, 8)

# --- head ----------------------------------------------------------------------
BONE = "head"
HEAD = (0, -0.82, 0.98)
HR = 0.66
sphere("Head", HEAD, (HR, HR, HR * 0.92), GREEN)
for s in (-1, 1):
    sphere(f"Cheek{s}", (s * 0.40, -1.36, 0.78), (0.13, 0.05, 0.09), PINK, 16, 8, rot=(0, 0, s * 0.3))
    sphere(f"Nostril{s}", (s * 0.09, -1.46, 1.02), (0.02, 0.015, 0.02), DGREEN, 8, 6)
for i in range(11):
    t = (i / 10) * 2 - 1
    x = t * 0.26
    z = HEAD[2] - 0.20 + 0.16 * t * t
    y = HEAD[1] - math.sqrt(max(HR ** 2 - x ** 2 - (z - HEAD[2]) ** 2, 0)) - 0.005
    sphere(f"Smile{i}", (x, y, z), (0.022, 0.02, 0.022), BLACK, 8, 6)
sphere("Casque", (0, -0.80, 1.60), (0.14, 0.34, 0.20), ORANGE)

# --- eyes (each on its own bone so they can look around) ----------------------
for s in (-1, 1):
    BONE = f"eye{s}"
    ex, ey, ez = s * 0.42, -1.0, 1.42
    sphere(f"EyeTurret{s}", (ex, ey, ez), (0.30, 0.30, 0.30), DGREEN)
    sphere(f"EyeWhite{s}", (ex + s * 0.03, ey - 0.13, ez + 0.01), (0.25, 0.22, 0.25), WHITE)
    sphere(f"Pupil{s}", (ex + s * 0.02, ey - 0.27, ez - 0.01), (0.15, 0.09, 0.17), BLACK)
    sphere(f"Shine{s}", (ex + 0.05, ey - 0.33, ez + 0.07), (0.045, 0.03, 0.045), WHITE, 12, 8)

# --- legs ----------------------------------------------------------------------
LEGS = {}  # bone name -> (hip, foot)
for sx in (-1, 1):
    for fy, name in ((-0.42, "F"), (0.5, "B")):
        BONE = f"leg{name}{sx}"
        LEGS[BONE] = (Vector((sx * 0.52, fy, 0.60)), Vector((sx * 0.58, fy - 0.07, 0.12)))
        sphere(f"Leg{name}{sx}", (sx * 0.52, fy, 0.38), (0.19, 0.22, 0.30), GREEN, 20, 10, rot=(0, sx * 0.25, 0))
        sphere(f"Foot{name}{sx}", (sx * 0.58, fy - 0.07, 0.12), (0.22, 0.30, 0.12), DGREEN, 20, 10)
        for t in (-1, 0, 1):
            sphere(f"Toe{name}{sx}{t}", (sx * 0.58 + t * 0.09, fy - 0.33, 0.1), (0.07, 0.07, 0.07), DGREEN, 12, 8)

# --- tail: sphere chain skinned to 6 bones --------------------------------------
cy, cz = 1.50, 1.00
N, TB = 34, 6
tail_pts = []
for i in range(N):
    s = i / (N - 1)
    a = 1.25 * math.pi + s * 2.35 * math.pi
    r = 0.80 * (1 - 0.72 * s)
    p = Vector((0, cy + r * math.cos(a), cz + r * math.sin(a)))
    tail_pts.append(p)
    rad = 0.24 * (1 - s) + 0.05
    BONE = f"tail{min(i * TB // N, TB - 1)}"
    col = ORANGE if (i // 3) % 2 and i > 6 else GREEN
    sphere(f"Tail{i}", p, (rad * 0.9, rad, rad), col, 20, 10)

# --- join ----------------------------------------------------------------------
bpy.ops.object.select_all(action="DESELECT")
for o in parts:
    o.select_set(True)
bpy.context.view_layer.objects.active = parts[0]
bpy.ops.object.join()
ch = bpy.context.active_object
ch.name = "Chameleon"
bpy.ops.object.origin_set(type="ORIGIN_CURSOR")
bpy.ops.object.shade_smooth()

# --- armature ------------------------------------------------------------------
bpy.ops.object.armature_add(location=(0, 0, 0))
arm = bpy.context.active_object
arm.name = "ChameleonRig"
arm.data.name = "ChameleonRig"
bpy.ops.object.mode_set(mode="EDIT")
eb = arm.data.edit_bones
eb.remove(eb["Bone"])


def add_bone(name, head, tail, parent=None):
    b = eb.new(name)
    b.head, b.tail = Vector(head), Vector(tail)
    if parent:
        b.parent = eb[parent]
    return b


add_bone("root", (0, 0, 0), (0, 0, 0.25))
add_bone("body", (0, 0.05, 0.62), (0, 0.6, 0.62), "root")
add_bone("head", (0, -0.6, 0.9), (0, -1.2, 0.98), "body")
for s in (-1, 1):
    add_bone(f"eye{s}", (s * 0.42, -1.0, 1.42), (s * 0.42, -1.3, 1.42), "head")
for name, (hip, foot) in LEGS.items():
    add_bone(name, hip, foot, "root")
firsts = [min(i for i in range(N) if min(i * TB // N, TB - 1) == k) for k in range(TB)]
prev = "body"
for k in range(TB):
    h = tail_pts[firsts[k]]
    t = tail_pts[firsts[k + 1]] if k + 1 < TB else tail_pts[-1] + (tail_pts[-1] - tail_pts[-2]) * 2
    if k == 0:
        h = Vector((0, 0.65, 0.55))
    add_bone(f"tail{k}", h, t, prev)
    prev = f"tail{k}"
bpy.ops.object.mode_set(mode="OBJECT")

ch.parent = arm
mod = ch.modifiers.new("Armature", "ARMATURE")
mod.object = arm

# --- animation -----------------------------------------------------------------
bpy.context.view_layer.objects.active = arm
bpy.ops.object.mode_set(mode="POSE")
FX, FY, FZ = Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1))
pose = arm.pose.bones
for pb in pose:
    pb.rotation_mode = "QUATERNION"
rest3 = {pb.name: arm.data.bones[pb.name].matrix_local.to_3x3().inverted() for pb in pose}


def rot(name, world_axis, angle):
    """Rotate bone about a world-space axis (expressed in the bone's rest frame)."""
    ax = rest3[name] @ world_axis
    from mathutils import Quaternion
    return Quaternion(ax, angle)


def move(name, world_vec):
    return rest3[name] @ world_vec


NF = 24
action_frames = range(1, NF + 2)  # 1..25
for f in action_frames:
    p = 2 * math.pi * (f - 1) / NF
    for pb in pose:
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.location = (0, 0, 0)

    # diagonal gait: legF-1 + legB1 together, legF1 + legB-1 together
    for name, (hip, foot) in LEGS.items():
        side = int(name[-2:]) if name[-2] == "-" else int(name[-1])
        front = name[3] == "F"
        ph = 0.0 if (front and side == -1) or (not front and side == 1) else math.pi
        q = p + ph
        pose[name].rotation_quaternion = rot(name, FX, 0.42 * math.sin(q))
        pose[name].location = move(name, FZ * (0.09 * max(0.0, -math.cos(q))))
        # foot toes-out twist for charm
        pose[name].rotation_quaternion = pose[name].rotation_quaternion @ rot(name, FY, 0.0)

    pose["body"].location = move("body", FZ * (0.025 * math.cos(2 * p)))
    pose["body"].rotation_quaternion = rot("body", FY, 0.05 * math.sin(p)) @ rot("body", FZ, 0.05 * math.sin(p + 0.5))
    pose["head"].rotation_quaternion = rot("head", FY, -0.04 * math.sin(p)) @ rot("head", FX, 0.05 * math.sin(2 * p + 1.0))

    # eyes look around independently
    pose["eye-1"].rotation_quaternion = rot("eye-1", FZ, 0.35 * math.sin(p + 0.3)) @ rot("eye-1", FX, 0.1 * math.sin(2 * p))
    pose["eye1"].rotation_quaternion = rot("eye1", FZ, 0.35 * math.sin(p + 1.9)) @ rot("eye1", FX, 0.1 * math.sin(2 * p + 1))

    # tail: curl/uncurl wave travelling down the chain + gentle side sway
    for k in range(TB):
        pose[f"tail{k}"].rotation_quaternion = (rot(f"tail{k}", FX, 0.13 * math.sin(p - 0.6 * k))
                                                @ rot(f"tail{k}", FZ, 0.07 * math.sin(p - 0.6 * k + 1.2)))

    for pb in pose:
        pb.keyframe_insert("rotation_quaternion", frame=f)
        pb.keyframe_insert("location", frame=f)

act = arm.animation_data.action
act.name = "Walk"
def all_fcurves(a):
    if hasattr(a, "fcurves"):
        return list(a.fcurves)
    return [fc for l in a.layers for st in l.strips for cb in st.channelbags for fc in cb.fcurves]


for fc in all_fcurves(act):
    fc.modifiers.new("CYCLES")  # video preview loops; exporters bake the 1-25 range
bpy.ops.object.mode_set(mode="OBJECT")
bpy.context.scene.frame_start, bpy.context.scene.frame_end = 1, NF
bpy.context.scene.render.fps = 24

# --- save + export -------------------------------------------------------------
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "chameleon_rigged.blend"))
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "chameleon_rigged.glb"), export_format="GLB",
                          export_animations=True, export_frame_range=False)

# --- preview scene -------------------------------------------------------------
sc = bpy.context.scene
w = bpy.data.worlds.new("W")
sc.world = w
w.use_nodes = True
w.node_tree.nodes["Background"].inputs[0].default_value = (0.75, 0.9, 1.0, 1)
bpy.ops.mesh.primitive_plane_add(size=20, location=(0, 0, 0))
bpy.context.active_object.data.materials.append(mat("Ground", (0.55, 0.8, 0.45), 0.9))
bpy.ops.object.light_add(type="SUN", location=(3, -4, 6), rotation=(math.radians(50), 0, math.radians(35)))
bpy.context.active_object.data.energy = 2.5


def make_cam(loc, tgt, lens=55):
    bpy.ops.object.camera_add(location=loc)
    c = bpy.context.active_object
    c.rotation_euler = (Vector(tgt) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
    c.data.lens = lens
    return c


cam34 = make_cam((4.3, -5.0, 2.6), (0, 0.3, 0.95))
camside = make_cam((-7.5, 0.35, 1.1), (0, 0.35, 1.0), 50)
sc.render.engine = "BLENDER_EEVEE"
sc.view_settings.view_transform = "Standard"

# side-view check frames
sc.camera = camside
sc.render.resolution_x, sc.render.resolution_y = 500, 400
sc.render.image_settings.file_format = "PNG"
for f in (1, 4, 7, 10, 13, 16, 19, 22):
    sc.frame_set(f)
    sc.render.filepath = os.path.join(OUT, f"_side_{f:02d}.png")
    bpy.ops.render.render(write_still=True)

# 3/4 view frames (assembled into walk.gif afterwards)
sc.camera = cam34
sc.render.resolution_x, sc.render.resolution_y = 560, 450
os.makedirs(os.path.join(OUT, "_anim"), exist_ok=True)
for f in range(1, NF + 1):
    sc.frame_set(f)
    sc.render.filepath = os.path.join(OUT, "_anim", f"{f:02d}.png")
    bpy.ops.render.render(write_still=True)
print("DONE")

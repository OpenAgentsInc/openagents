# Aethyr's architecture kit

Buildings made from parts. A part is built once, as triangles, and stood
wherever a realm wants it.

| File | What |
| --- | --- |
| `mesh.coax` | The pen that writes triangles into a buffer you own, and the shapes: `arch_box`, `arch_lathe` (a profile turned round an axis, with fluting), `arch_ring`, `arch_ellipsoid`, `arch_limb`, `arch_gem`, `arch_panel` |
| `classical.coax` | Classical parts built from those shapes: `arch_column` (a fluted Doric column), `arch_plinth`, `arch_baluster`, `arch_rails`, `arch_entablature`, `arch_brazier`, `arch_flame`, `arch_band` |
| `place.coax` | Records that stand a part somewhere, and draws that name a part and a run of records: `arch_stand`, `arch_round` (a ring of them, with a gap for a doorway), `arch_draw` |

The first user is the Golemancers' Workshop
([`everglade/workshop/temple.coax`](../../everglade/workshop/temple.coax)),
drawn by shaders written in Coax
([`everglade/workshop/gpu/temple.coax`](../../everglade/workshop/gpu/temple.coax)).

## Build a part and stand it

```coax
p: [1] ArchPen; arch_pen(p[..], mesh[..]);
s: [1] ArchScene; arch_scene(s[..], records[..], draws[..]);

p[0].material = MARBLE;
column := arch_column(p[..], 0.4, 6.0, ARCH_FLUTES);          // built once
placed := arch_round(s[..], column, 16, 7.75, 0, 0, 0, 0);    // stood sixteen times in a ring
```

- **A vertex** is 12 floats: place, normal, material, two surface coordinates
  and three spare. Triangles are a plain list, counter-clockwise from outside.
- **A record** is 16 floats: place and turn, scale, a motion and its phase, a
  tint and a value of its own.
- **A draw** is four words: first vertex, vertex count, first record, record
  count. A host draws it as one instanced call.
- **A full pen or scene stops whole** and says so (`full`). It never writes
  part of a triangle.

## Checks

```sh
coax/bin/coax test --library aethyr/arch/mesh.coax
coax/bin/coax test --library aethyr/arch/classical.coax
coax/bin/coax test --library aethyr/arch/place.coax
```

They check that every shape faces out, keeps to its size, and that a short
buffer is never overrun.

## What it doesn't do yet

- No textures: a shader colours a surface from its material and position.
- No level of detail: a part has one mesh.
- Shadows are the shader's own. The Workshop's are worked out for its columns,
  not cast from the mesh.

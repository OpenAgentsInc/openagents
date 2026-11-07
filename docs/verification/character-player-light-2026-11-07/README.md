# Character Create player daylight

Before: 5f2028dfda used the legacy figure brightness override and finish, then
decoded that lit display result into HDR. The player did not sample the geometry
shadow map. After: the private character shader shades linear base colour using
the meadow sun and shadows, hemisphere fill, warm camera-side bounce and broad
specular highlights. The same HDR postprocessing handles player and surroundings.
The screenshots compare the same default camera and wardrobe; the female cast
and phone captures exercise other geometry and responsive layout.

The new Coax check requires directional response, warm sun chromaticity,
shadow attenuation with retained sky fill, and zero diffuse light from black.
Desktop and phone GPU checks compile the actual shader on Apple Metal, require
shadow/post and two HDR targets, and collect GPU errors. The browser controls
journey checks two bodies, animation, eye selection, save/reload, pan and zoom.
No world pack, shared renderer, appearance format or WASM change is needed.

-- Fairy Forest is a park laid out like Central Park (verse/fairy-forest/README.md "Size"), with
-- simple trees and bushes and no grass: its "Wind-swept grass" row switched nothing, so it goes,
-- and "The far ridges" are now the city's blocks round the park.
UPDATE verse.realm_rights
   SET rows = '[{"id": "sparkles", "label": "Fairy dust sparkles", "says": "the fairy dust sparkling behind you"}, {"id": "ridges", "label": "The city round the park", "says": "the city blocks and towers round the park"}]'::jsonb
 WHERE slug = 'fairy-forest';
UPDATE verse.realm_edit SET switched = switched - 'grass' WHERE slug = 'fairy-forest' AND switched ? 'grass';

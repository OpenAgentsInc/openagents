/** Shared simulation/query grid; arena padding keeps contact queries inside it. */
export const BATTLE_GRID = Object.freeze({
  dimension: 512,
  cellSize: 4,
  halfExtent: 1024,
  maxRadius: 1000,
  formationSpacing: 2,
  jitterFraction: 0.04,
  largeNeighborOffset: 0.2,
});

export const BATTLE_BODY_RADII = Object.freeze([
  0.63, 0.63, 0.64, 1.33,
] as const);

export const BATTLE_GRID_WGSL = /* wgsl */ `
const battleMaxBodyRadius=${Math.max(...BATTLE_BODY_RADII)};
const battleBodyRadii=array<f32,4>(${BATTLE_BODY_RADII.join(",")});
const battleFormationJitter=${BATTLE_GRID.jitterFraction};
const battleLargeNeighborOffset=${BATTLE_GRID.largeNeighborOffset};
fn bodyRadius(kind:u32)->f32{return battleBodyRadii[min(kind,3u)];}
const battleGridDimension=512;
const battleGridCells=262144u;
const battleCellSize=4.0;
const battleGridHalfExtent=1024.0;
fn battleGridPosition(position:vec2f,center:vec2f)->vec2i {
 return vec2i(clamp(floor((position-center+vec2(battleGridHalfExtent))/battleCellSize),vec2(0),vec2(f32(battleGridDimension-1))));
}
fn battleGridIndex(position:vec2i)->u32{return u32(position.x+position.y*battleGridDimension);}
`;

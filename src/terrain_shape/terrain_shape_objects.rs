use specs;

use crate::game::constants::TILE_SIZE;
use crate::graphics::orientation::Orientation;
use crate::shaders::Position;
use crate::terrain_shape::TerrainShapeDrawable;

fn set_position(x: isize, y: isize) -> Position {
  Position::new(
    TILE_SIZE * x as f32,
    TILE_SIZE * 0.8 * y as f32
  )
}

pub struct TerrainShapeObjects {
  pub objects: Vec<TerrainShapeDrawable>,
}

impl TerrainShapeObjects {
  pub fn new() -> TerrainShapeObjects {
    TerrainShapeObjects {
      objects: vec![
        TerrainShapeDrawable::new(set_position(0, 4), Orientation::DownLeft)
      ]
    }
  }
}

impl specs::prelude::Component for TerrainShapeObjects {
  type Storage = specs::storage::VecStorage<TerrainShapeObjects>;
}
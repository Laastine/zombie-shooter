use bullet::{BulletDrawable, bullets::Bullets};
use cgmath::Point2;
use character::controls::CharacterInputState;
use critter::CritterData;
use data;
use game::constants::{ASPECT_RATIO, NORMAL_DEATH_SPRITE_OFFSET, SPRITE_OFFSET, VIEW_DISTANCE, ZOMBIE_SHEET_TOTAL_WIDTH, ZOMBIE_STILL_SPRITE_OFFSET};
use game::get_random_bool;
use gfx;
use gfx_app::{ColorFormat, DepthFormat};
use graphics::{add_random_offset_to_screen_pos,
               calc_hypotenuse,
               camera::CameraInputState,
               can_move_to_tile, dimensions::{Dimensions, get_projection, get_view_matrix},
               direction,
               direction_movement,
               direction_movement_180,
               GameTime,
               orientation::{Orientation, Stance},
               orientation_to_direction,
               overlaps,
               texture::load_texture};
use shaders::{CharacterSheet, critter_pipeline, Position, Projection, VertexData};
use specs;
use specs::prelude::{Read, ReadStorage, WriteStorage};
use terrain::path_finding::calc_next_movement;
use zombie::zombies::Zombies;

pub mod zombies;

const SHADER_VERT: &[u8] = include_bytes!("../shaders/character.v.glsl");
const SHADER_FRAG: &[u8] = include_bytes!("../shaders/character.f.glsl");

#[derive(Debug, Clone)]
pub struct ZombieDrawable {
  projection: Projection,
  pub position: Position,
  previous_position: Position,
  orientation: Orientation,
  pub stance: Stance,
  direction: Orientation,
  last_decision: u64,
  pub movement_direction: Point2<f32>,
  zombie_idx: usize,
  zombie_death_idx: usize,
  is_colliding: bool,
  movement_speed: f32,
}

impl ZombieDrawable {
  pub fn new(position: Position) -> ZombieDrawable {
    let view = get_view_matrix(VIEW_DISTANCE);
    let projection = get_projection(view, ASPECT_RATIO);
    ZombieDrawable {
      projection,
      position,
      previous_position: Position::new(0.0, 0.0),
      orientation: Orientation::Left,
      stance: Stance::Still,
      direction: Orientation::Left,
      last_decision: 0,
      movement_direction: Point2::new(0.0, 0.0),
      zombie_idx: 0,
      zombie_death_idx: 0,
      is_colliding: false,
      movement_speed: 0.0,
    }
  }

  pub fn update(&mut self, world_to_clip: &Projection, ci: &CharacterInputState, game_time: u64) {
    self.projection = *world_to_clip;

    let offset_delta = ci.movement - self.previous_position;

    self.previous_position = ci.movement;

    let x_y_distance_to_player = self.position - offset_delta;

    let distance_to_player = calc_hypotenuse(x_y_distance_to_player.position[0].abs(), x_y_distance_to_player.position[1].abs());

    let is_alive = self.stance != Stance::NormalDeath && self.stance != Stance::CriticalDeath;

    if is_alive {
      let zombie_pos = ci.movement - self.position;

      if distance_to_player < 300.0 {
        let dir = calc_next_movement(zombie_pos, self.previous_position) as f32;
        self.direction = orientation_to_direction(dir);
        self.movement_direction = direction_movement(dir);
        self.stance = Stance::Walking;
        self.movement_speed = 1.4;
      } else {
        self.idle_direction_movement(zombie_pos, game_time);
        self.movement_speed = 1.0;
      }
    } else {
      self.movement_direction = Point2::new(0.0, 0.0);
    }

    self.position = Position::new(
      self.movement_direction.x * self.movement_speed,
      self.movement_direction.y * self.movement_speed) + self.position + offset_delta;
  }

  fn idle_direction_movement(&mut self, zombie_pos: Position, game_time: u64) {
    if !can_move_to_tile(zombie_pos) {
      let dir = direction(Point2::new(0.0, 0.0), self.movement_direction);
      self.movement_direction = direction_movement_180(self.movement_direction);
      self.orientation = orientation_to_direction(dir);
    }

    if self.last_decision + 2 < game_time || game_time == 0 {
      self.stance = Stance::Walking;
      self.last_decision = game_time;
      let end_point = add_random_offset_to_screen_pos(zombie_pos);
      let dir = calc_next_movement(zombie_pos, end_point) as f32;
      self.movement_direction = direction_movement(dir);
      self.direction = orientation_to_direction(dir);
    }
  }

  fn check_bullet_hits(&mut self, bullets: &[BulletDrawable]) {
    bullets.iter().for_each(|bullet| {
      if overlaps(self.position, bullet.position, 15.0, 15.0) && self.stance != Stance::NormalDeath && self.stance != Stance::CriticalDeath {
        self.stance =
          if get_random_bool() {
            Stance::NormalDeath
          } else {
            Stance::CriticalDeath
          };
      }
    });
  }

  pub fn update_alive_idx(&mut self, max_idx: usize) {
    if self.zombie_idx < max_idx {
      self.zombie_idx += 1;
    } else {
      self.zombie_idx = 0;
    }
  }

  pub fn update_death_idx(&mut self, max_idx: usize) {
    if self.zombie_death_idx < max_idx {
      self.zombie_death_idx += 1;
    }
  }
}

pub struct ZombieDrawSystem<R: gfx::Resources> {
  bundle: gfx::pso::bundle::Bundle<R, critter_pipeline::Data<R>>,
  data: Vec<CritterData>,
}

impl<R: gfx::Resources> ZombieDrawSystem<R> {
  pub fn new<F>(factory: &mut F,
                rtv: gfx::handle::RenderTargetView<R, ColorFormat>,
                dsv: gfx::handle::DepthStencilView<R, DepthFormat>) -> ZombieDrawSystem<R>
                where F: gfx::Factory<R> {
    use gfx::traits::FactoryExt;

    let zombie_bytes = include_bytes!("../../assets/zombie.png");

    let vertex_data: [VertexData; 4] = [
      VertexData::new([-25.0, -35.0], [0.0, 1.0]),
      VertexData::new([25.0, -35.0], [1.0, 1.0]),
      VertexData::new([25.0, 35.0], [1.0, 0.0]),
      VertexData::new([-25.0, 35.0], [0.0, 0.0]),
    ];

    let index_data: [u16; 6] = [0, 1, 2, 2, 3, 0];

    let (vertex_buf, slice) = factory.create_vertex_buffer_with_slice(&vertex_data[..], &index_data[..]);

    let char_texture = load_texture(factory, zombie_bytes);
    let pso = factory
      .create_pipeline_simple(SHADER_VERT,
                              SHADER_FRAG,
                              critter_pipeline::new())
      .unwrap();

    let pipeline_data = critter_pipeline::Data {
      vbuf: vertex_buf,
      projection_cb: factory.create_constant_buffer(1),
      position_cb: factory.create_constant_buffer(1),
      character_sprite_cb: factory.create_constant_buffer(1),
      charactersheet: (char_texture, factory.create_sampler_linear()),
      out_color: rtv,
      out_depth: dsv,
    };

    let data = data::load_zombie();

    ZombieDrawSystem {
      bundle: gfx::Bundle::new(slice, pso, pipeline_data),
      data,
    }
  }

  fn get_next_sprite(&self, drawable: &mut ZombieDrawable) -> CharacterSheet {
    let zombie_sprite = match drawable.stance {
      Stance::Still => {
        let sprite_idx = (drawable.direction as usize * 4 + drawable.zombie_idx) as usize;
        (&self.data[sprite_idx], sprite_idx)
      },
      Stance::Walking if drawable.orientation != Orientation::Still => {
        let sprite_idx = (drawable.direction as usize * 8 + drawable.zombie_idx + ZOMBIE_STILL_SPRITE_OFFSET) as usize;
        (&self.data[sprite_idx], sprite_idx)
      },
      Stance::NormalDeath if drawable.orientation != Orientation::Still => {
        let sprite_idx = (drawable.direction as usize * 6 + drawable.zombie_death_idx + NORMAL_DEATH_SPRITE_OFFSET) as usize;
        (&self.data[sprite_idx], sprite_idx)
      },
      Stance::CriticalDeath if drawable.orientation != Orientation::Still => {
        let sprite_idx = (drawable.direction as usize * 8 + drawable.zombie_death_idx) as usize;
        (&self.data[sprite_idx], sprite_idx)
      },
      _ => {
        drawable.direction = drawable.orientation;
        let sprite_idx = (drawable.orientation as usize * 8 + drawable.zombie_idx + ZOMBIE_STILL_SPRITE_OFFSET) as usize;
        (&self.data[sprite_idx], sprite_idx)
      },
    };

    let (y_div, row_idx) =
      if drawable.stance == Stance::NormalDeath || drawable.stance == Stance::CriticalDeath {
        (0.0, 2)
      } else {
        (1.0, 2)
      };

    let elements_x = ZOMBIE_SHEET_TOTAL_WIDTH / (zombie_sprite.0.data[2] + SPRITE_OFFSET);
    CharacterSheet {
      x_div: elements_x,
      y_div,
      row_idx,
      index: zombie_sprite.1 as f32,
    }
  }

  pub fn draw<C>(&mut self,
                 mut drawable: &mut ZombieDrawable,
                 encoder: &mut gfx::Encoder<R, C>)
                 where C: gfx::CommandBuffer<R> {
    encoder.update_constant_buffer(&self.bundle.data.projection_cb, &drawable.projection);
    encoder.update_constant_buffer(&self.bundle.data.position_cb, &drawable.position);
    encoder.update_constant_buffer(&self.bundle.data.character_sprite_cb,
                                   &self.get_next_sprite(&mut drawable));
    self.bundle.encode(encoder);
  }
}

#[derive(Debug)]
pub struct PreDrawSystem;

impl PreDrawSystem {
  pub fn new() -> PreDrawSystem {
    PreDrawSystem {}
  }
}

impl<'a> specs::prelude::System<'a> for PreDrawSystem {
  #[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
  type SystemData = (WriteStorage<'a, Zombies>,
                     ReadStorage<'a, CameraInputState>,
                     ReadStorage<'a, CharacterInputState>,
                     ReadStorage<'a, Bullets>,
                     Read<'a, Dimensions>,
                     Read<'a, GameTime>);

  fn run(&mut self, (mut zombies, camera_input, character_input, bullets, dim, gt): Self::SystemData) {
    use specs::join::Join;

    for (zs, camera, ci, bs) in (&mut zombies, &camera_input, &character_input, &bullets).join() {
      let world_to_clip = dim.world_to_projection(camera);

      for z in &mut zs.zombies {
        z.update(&world_to_clip, ci, gt.0);
        z.check_bullet_hits(&bs.bullets);
      }
    }
  }
}

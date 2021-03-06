use gfx::format::SurfaceType;
use gfx::handle::{DepthStencilView, RenderTargetView};
use gfx::memory::Typed;
use glutin::{KeyboardInput, MouseButton, PossiblyCurrent, WindowedContext};
use glutin::dpi::LogicalSize;
use glutin::ElementState::{Pressed, Released};
use glutin::VirtualKeyCode::{A, D, Escape, R, S, W, X, Z};
use std::fmt::{Display, Formatter, Result};

use crate::character::controls::CharacterControl;
use crate::game::constants::{GAME_TITLE, RESOLUTION_X, RESOLUTION_Y};
use crate::gfx_app::controls::{Control, TilemapControls};

pub mod init;
pub mod renderer;
pub mod system;
pub mod controls;
pub mod mouse_controls;

pub type ColorFormat = gfx::format::Rgba8;
pub type DepthFormat = gfx::format::DepthStencil;

pub const COLOR_FORMAT_VALUE: SurfaceType = SurfaceType::R8_G8_B8_A8;
pub const DEPTH_FORMAT_VALUE: SurfaceType = SurfaceType::D24_S8;

#[derive(Debug)]
pub struct GameOptions {
  windowed_mode: bool,
}

impl Display for GameOptions {
  fn fmt(&self, f: &mut Formatter) -> Result {
    write!(f, "{}", format!("windowed_mode={}", self.windowed_mode))
  }
}

impl GameOptions {
  pub fn new(windowed_mode: bool) -> GameOptions {
    GameOptions {
      windowed_mode,
    }
  }
}

pub struct WindowContext {
  window_context: WindowedContext<PossiblyCurrent>,
  controls: Option<controls::TilemapControls>,
  events_loop: glutin::EventsLoop,
  device: gfx_device_gl::Device,
  factory: gfx_device_gl::Factory,
  render_target_view: RenderTargetView<gfx_device_gl::Resources, ColorFormat>,
  depth_stencil_view: DepthStencilView<gfx_device_gl::Resources, DepthFormat>,
  mouse_pos: (f64, f64),
  game_options: GameOptions
}

impl WindowContext {
  pub fn new(game_options: GameOptions) -> WindowContext {
    let events_loop = glutin::EventsLoop::new();

    let window_title = glutin::WindowBuilder::new()
      .with_title(GAME_TITLE);

    println!("{}", game_options);

    let builder = if game_options.windowed_mode {
      let logical_size = LogicalSize::new(RESOLUTION_X.into(), RESOLUTION_Y.into());
      window_title
        .with_dimensions(logical_size)
        .with_decorations(false)
    } else {
      let monitor = {
        events_loop.get_available_monitors().next().expect("No monitor found")
      };
      let monitor_resolution = monitor.get_dimensions();

      let resolution = ((monitor_resolution.width as f32 * 16.0 / 9.0) as u32, monitor_resolution.height);

      let logical_size = LogicalSize::new(resolution.0.into(), resolution.1);
      window_title.with_fullscreen(Some(monitor))
        .with_decorations(false)
        .with_dimensions(logical_size)
    };

    let window_context = glutin::ContextBuilder::new()
      .with_vsync(true)
      .with_double_buffer(Some(true))
      .with_pixel_format(24, 8)
      .with_srgb(true)
      .build_windowed(builder, &events_loop)
      .expect("Window context creation failed");

    let window_context = unsafe {
      window_context
        .make_current()
        .expect("Window focus failed")
    };

    let (width, height) = {
      let inner_size = window_context.window().get_inner_size().expect("get_inner_size failed");
      let size = inner_size.to_physical(window_context.window().get_hidpi_factor());
      (size.width as _, size.height as _)
    };

    let aa = window_context
      .get_pixel_format().multisampling
      .unwrap_or(0) as u8;

    let window_dimensions = (width, height, 1, aa.into());

    let (device, factory) = gfx_device_gl::create(|s|
      window_context.get_proc_address(s) as *const std::os::raw::c_void);

    let (rtv, dsv) =
      gfx_device_gl::create_main_targets_raw(window_dimensions,
                                             COLOR_FORMAT_VALUE,
                                             DEPTH_FORMAT_VALUE);

    WindowContext {
      window_context,
      controls: None,
      events_loop,
      device,
      factory,
      render_target_view: RenderTargetView::new(rtv),
      depth_stencil_view: DepthStencilView::new(dsv),
      mouse_pos: (0.0, 0.0),
      game_options,
    }
  }
}

#[derive(PartialEq, Eq)]
pub enum WindowStatus {
  Open,
  Close,
}

pub trait Window<D: gfx::Device, F: gfx::Factory<D::Resources>> {
  fn swap_window(&mut self);
  fn create_buffers(&mut self, count: usize) -> Vec<D::CommandBuffer>;
  fn set_controls(&mut self, controls: controls::TilemapControls);
  fn get_viewport_size(&mut self) -> (f32, f32);
  fn get_device(&mut self) -> &mut D;
  fn get_factory(&mut self) -> &mut F;
  fn get_hidpi_factor(&mut self) -> f32;
  fn get_render_target_view(&mut self) -> RenderTargetView<D::Resources, ColorFormat>;
  fn get_depth_stencil_view(&mut self) -> DepthStencilView<D::Resources, DepthFormat>;
  fn poll_events(&mut self) -> WindowStatus;
  fn is_windowed(&self) -> bool;
}

impl Window<gfx_device_gl::Device, gfx_device_gl::Factory> for WindowContext {
  fn swap_window(&mut self) {
    use gfx::Device;
    self.window_context
      .swap_buffers()
      .expect("Unable to swap buffers");
    self.device.cleanup();
  }

  fn create_buffers(&mut self, count: usize) -> Vec<gfx_device_gl::CommandBuffer> {
    let mut bufs = Vec::new();
    for _ in 0..count {
      bufs.push(self.factory.create_command_buffer());
    }
    bufs
  }

  fn set_controls(&mut self, controls: controls::TilemapControls) {
    self.controls = Some(controls);
  }

  fn get_viewport_size(&mut self) -> (f32, f32) {
    if self.game_options.windowed_mode {
      (RESOLUTION_X as f32, RESOLUTION_Y as f32)
    } else {
      let monitor = self.events_loop.get_available_monitors().next().expect("No monitor found");
      let monitor_resolution = monitor.get_dimensions();
      (monitor_resolution.width as f32, monitor_resolution.height as f32)
    }
  }

  fn get_device(&mut self) -> &mut gfx_device_gl::Device {
    &mut self.device
  }

  fn get_factory(&mut self) -> &mut gfx_device_gl::Factory {
    &mut self.factory
  }

  fn get_hidpi_factor(&mut self) -> f32 {
    if self.game_options.windowed_mode {
      1.0
    } else {
      self.window_context.window().get_hidpi_factor() as f32
    }
  }

  fn get_render_target_view(&mut self) -> RenderTargetView<gfx_device_gl::Resources, ColorFormat> {
    self.render_target_view.clone()
  }

  fn get_depth_stencil_view(&mut self) -> DepthStencilView<gfx_device_gl::Resources, DepthFormat> {
    self.depth_stencil_view.clone()
  }

  fn poll_events(&mut self) -> WindowStatus {
    use glutin::WindowEvent::{CursorMoved, CloseRequested, MouseInput};

    let controls = match self.controls {
      Some(ref mut c) => c,
      None => panic!("Terrain controls have not been initialized"),
    };

    let m_pos = &mut self.mouse_pos;
    let mut game_status = WindowStatus::Open;

    self.events_loop.poll_events(|event| {
      game_status = if let glutin::Event::WindowEvent { event, .. } = event {
        match event {
          glutin::WindowEvent::KeyboardInput { input, .. } => { process_keyboard_input(input, controls) }
          MouseInput { state: Pressed, button: MouseButton::Left, .. } => {
            controls.mouse_left_click(Some(*m_pos));
            WindowStatus::Open
          }
          MouseInput { state: Released, button: MouseButton::Left, .. } => {
            controls.mouse_left_click(None);
            WindowStatus::Open
          }
          CursorMoved { position, .. } => {
            *m_pos = ((position.x as f32).into(), (position.y as f32).into());
            WindowStatus::Open
          }
          CloseRequested => WindowStatus::Close,
          _ => WindowStatus::Open,
        }
      } else {
        WindowStatus::Open
      };
    });
    game_status
  }

  fn is_windowed(&self) -> bool {
    self.game_options.windowed_mode
  }
}

fn process_keyboard_input(input: glutin::KeyboardInput, controls: &mut TilemapControls) -> WindowStatus {
  match input {
    KeyboardInput { state: Pressed, virtual_keycode: Some(Z), .. } => {
      controls.zoom(&Control::Negative);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(X), .. } => {
      controls.zoom(&Control::Plus);
    }
    KeyboardInput { state: Released, virtual_keycode: Some(Z), .. } |
    KeyboardInput { state: Released, virtual_keycode: Some(X), .. } => {
      controls.zoom(&Control::Released);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(W), .. } => {
      controls.move_character(CharacterControl::Up);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(S), .. } => {
      controls.move_character(CharacterControl::Down);
    }
    KeyboardInput { state: Released, virtual_keycode: Some(W), .. } |
    KeyboardInput { state: Released, virtual_keycode: Some(S), .. } => {
      controls.move_character(CharacterControl::YMoveStop);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(A), .. } => {
      controls.move_character(CharacterControl::Left);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(D), .. } => {
      controls.move_character(CharacterControl::Right);
    }
    KeyboardInput { state: Released, virtual_keycode: Some(A), .. } |
    KeyboardInput { state: Released, virtual_keycode: Some(D), .. } => {
      controls.move_character(CharacterControl::XMoveStop);
    }
    KeyboardInput { state: Pressed, virtual_keycode: Some(R), .. } => {
      controls.reload_weapon(true);
    }
    KeyboardInput { state: Released, virtual_keycode: Some(R), .. } => {
      controls.reload_weapon(false);
    }
    KeyboardInput { state: Pressed, modifiers, .. } => {
      if modifiers.ctrl {
        controls.ctrl_pressed(true);
      }
    }
    KeyboardInput { state: Released, modifiers, .. } => {
      if !modifiers.ctrl {
        controls.ctrl_pressed(false);
      }
    }
  }
  if let Some(Escape) = input.virtual_keycode {
    WindowStatus::Close
  } else {
    WindowStatus::Open
  }
}


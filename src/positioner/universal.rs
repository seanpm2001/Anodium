use smithay::utils::{Logical, Point, Rectangle};

use super::{floating::Floating, tiling::Tiling, MoveResponse, Positioner};

use crate::desktop_layout::{Toplevel, Window, WindowList};

#[allow(unused)]
#[derive(Debug)]
pub enum PositionerMode {
    Floating,
    Tiling,
}

#[derive(Debug)]
pub struct Universal {
    floating: Floating,
    tiling: Tiling,

    mode: PositionerMode,
}

impl Universal {
    #[allow(unused)]
    pub fn new(pointer_position: Point<f64, Logical>, geometry: Rectangle<i32, Logical>) -> Self {
        Self {
            floating: Floating::new(pointer_position, geometry),
            tiling: Tiling::new(pointer_position, geometry),
            mode: PositionerMode::Floating,
        }
    }
}

impl Positioner for Universal {
    fn map_toplevel(&mut self, window: Window, reposition: bool) {
        match self.mode {
            PositionerMode::Floating => self.floating.map_toplevel(window, reposition),
            PositionerMode::Tiling => self.tiling.map_toplevel(window, reposition),
        }
    }

    fn unmap_toplevel(&mut self, toplevel: &Toplevel) -> Option<Window> {
        if let Some(win) = self.floating.unmap_toplevel(toplevel) {
            Some(win)
        } else if let Some(win) = self.tiling.unmap_toplevel(toplevel) {
            Some(win)
        } else {
            None
        }
    }

    fn move_request(
        &mut self,
        toplevel: &Toplevel,
        seat: &smithay::wayland::seat::Seat,
        serial: smithay::wayland::Serial,
        start_data: &smithay::wayland::seat::GrabStartData,
    ) -> Option<MoveResponse> {
        if let Some(req) = self.floating.move_request(toplevel, seat, serial, start_data) {
            Some(req)
        } else if let Some(req) = self.tiling.move_request(toplevel, seat, serial, start_data) {
            Some(req)
        } else {
            None
        }
    }

    fn resize_request(
        &mut self,
        toplevel: &Toplevel,
        seat: &smithay::wayland::seat::Seat,
        serial: smithay::wayland::Serial,
        start_data: smithay::wayland::seat::GrabStartData,
        edges: smithay::reexports::wayland_protocols::xdg_shell::server::xdg_toplevel::ResizeEdge,
    ) {
        self.floating
            .resize_request(toplevel, seat, serial, start_data.clone(), edges);
        self.tiling
            .resize_request(toplevel, seat, serial, start_data, edges);
    }

    fn maximize_request(&mut self, toplevel: &Toplevel) {
        self.floating.maximize_request(toplevel);
        self.tiling.maximize_request(toplevel);
    }

    fn unmaximize_request(&mut self, toplevel: &Toplevel) {
        self.floating.unmaximize_request(toplevel);
        self.tiling.unmaximize_request(toplevel);
    }

    fn windows<'a>(&'a self) -> &'a WindowList {
        // self.floating.windows();
        unimplemented!("");
    }

    fn windows_mut<'a>(&'a mut self) -> &'a mut WindowList {
        // self.floating.windows_mut()
        unimplemented!("");
    }

    fn on_pointer_move(&mut self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) {
        self.floating.on_pointer_move(pos);
        self.tiling.on_pointer_move(pos);
    }

    fn on_pointer_button(
        &mut self,
        button: smithay::backend::input::MouseButton,
        state: smithay::backend::input::ButtonState,
    ) {
        self.floating.on_pointer_button(button, state);
        self.tiling.on_pointer_button(button, state);
    }

    fn set_geometry(&mut self, size: smithay::utils::Rectangle<i32, smithay::utils::Logical>) {
        self.floating.set_geometry(size);
        self.tiling.set_geometry(size);
    }

    fn geometry(&self) -> Rectangle<i32, Logical> {
        self.floating.geometry()
    }

    fn send_frames(&self, time: u32) {
        self.floating.send_frames(time);
        self.tiling.send_frames(time);
    }

    fn update(&mut self, delta: f64) {
        self.floating.update(delta);
        self.tiling.update(delta);
    }
}
